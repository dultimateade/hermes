//! Voting delegation with per-address power cap.
//!
//! Implements `set_delegate` / `unset_delegate` entrypoints and enforces a
//! maximum delegation cap: no single address may accumulate delegated voting
//! power exceeding **10 % of the total voting power** of the market.
//!
//! # Design
//!
//! * `Delegation` record — stores the delegator→delegatee relationship and
//!   the amount of voting power that was delegated.
//! * `DelegationManager` — all storage operations and cap enforcement.
//! * The cap is enforced at *delegation time* (`set_delegate`). If adding the
//!   delegator's stake would push the delegatee's total delegated power above
//!   the cap, the call is rejected with [`Error::DelegationCapExceeded`].
//! * `unset_delegate` releases the delegated power and removes the record.
//!
//! # Storage layout
//!
//! | Key                                   | Value        |
//! |---------------------------------------|--------------|
//! | `Delegation(delegator)`               | `Delegation` |
//! | `DelegatedPower(delegatee)`           | `i128`       |
//! | `TotalVotingPower`                    | `i128`       |
//!
//! All keys live in **instance** storage so they are bumped automatically by
//! the Soroban runtime.

use soroban_sdk::{contracttype, Address, Env, Map};

use crate::err::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum fraction (in basis points) of total voting power that a single
/// address may hold through delegation. 1 000 bps = 10 %.
pub const MAX_DELEGATION_BPS: i128 = 1_000;

/// Denominator for basis-point calculations (100 %).
pub const BPS_DENOMINATOR: i128 = 10_000;

/// TTL threshold (ledgers) before delegation keys are bumped on read.
const DELEGATION_TTL_THRESHOLD: u32 = 5_356;

/// Extended TTL (ledgers) for delegation storage entries (~31 days).
const DELEGATION_TTL_TO: u32 = 518_400;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Persistent record of an active delegation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delegation {
    /// The address that delegated their voting power.
    pub delegator: Address,
    /// The address receiving the delegated power.
    pub delegatee: Address,
    /// Amount of voting power delegated (in stroops / base units).
    pub amount: i128,
}

/// Storage keys used by the delegation subsystem.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DelegationKey {
    /// Active delegation record keyed by delegator address.
    Delegation(Address),
    /// Cumulative delegated power received by a delegatee.
    DelegatedPower(Address),
    /// Global running total of all voting power in the system.
    TotalVotingPower,
}

// ---------------------------------------------------------------------------
// Manager
// ---------------------------------------------------------------------------

/// All delegation storage operations and cap enforcement.
pub struct DelegationManager;

impl DelegationManager {
    // -----------------------------------------------------------------------
    //  Public API
    // -----------------------------------------------------------------------

    /// Activate a delegation: the `delegator` transfers their `stake` amount
    /// of voting power to `delegatee`.
    ///
    /// # Cap enforcement
    /// The delegatee's new total delegated power must not exceed
    /// `MAX_DELEGATION_BPS / BPS_DENOMINATOR` of [`TotalVotingPower`].
    ///
    /// # Errors
    /// * [`Error::InvalidInput`] — `stake` is not positive.
    /// * [`Error::InvalidInput`] — delegator and delegatee are the same.
    /// * [`Error::DelegationCapExceeded`] — cap would be exceeded.
    pub fn set_delegate(
        env: &Env,
        delegator: &Address,
        delegatee: &Address,
        stake: i128,
    ) -> Result<(), Error> {
        delegator.require_auth();

        if stake <= 0 {
            return Err(Error::InvalidInput);
        }
        if delegator == delegatee {
            return Err(Error::InvalidInput);
        }

        Self::validate_no_cycle(env, delegator, delegatee)?;

        // If the delegator already has an active delegation, remove the old
        // one first so we don't double-count.
        if let Some(existing) = Self::get_delegation(env, delegator) {
            if existing.delegatee == *delegatee && existing.amount == stake {
                // Idempotent: same delegation already active.
                return Ok(());
            }
            Self::remove_delegation_power(env, &existing.delegatee, existing.amount)?;
        }

        // Check cap BEFORE writing.
        let total_power = Self::get_total_voting_power(env);
        let current_delegated = Self::get_delegated_power(env, delegatee);
        let new_delegated = current_delegated
            .checked_add(stake)
            .ok_or(Error::Overflow)?;

        let cap = Self::delegation_cap(total_power);
        if new_delegated > cap {
            return Err(Error::DelegationCapExceeded);
        }

        // Write the new delegation record.
        let record = Delegation {
            delegator: delegator.clone(),
            delegatee: delegatee.clone(),
            amount: stake,
        };
        Self::store_delegation(env, &record);
        Self::add_delegated_power(env, delegatee, stake)?;

        Ok(())
    }

    /// Rejects self-delegation and chains that return to the delegator.
    fn validate_no_cycle(
        env: &Env,
        delegator: &Address,
        delegatee: &Address,
    ) -> Result<(), Error> {
        if delegator == delegatee {
            return Err(Error::DelegationSelfDelegation);
        }

        let mut current = delegatee.clone();
        for _ in 0..64 {
            if current == *delegator {
                return Err(Error::DelegationCircular);
            }

            let next = match Self::get_delegation(env, &current) {
                Some(record) => record.delegatee,
                None => return Ok(()),
            };
            if next == current {
                return Err(Error::DelegationCircular);
            }
            current = next;
        }

        Err(Error::DelegationCircular)
    }

    /// Remove an active delegation, releasing the delegated voting power.
    ///
    /// # Errors
    /// * [`Error::InvalidInput`] — no active delegation for this delegator.
    pub fn unset_delegate(env: &Env, delegator: &Address) -> Result<(), Error> {
        delegator.require_auth();

        let record = Self::get_delegation(env, delegator)
            .ok_or(Error::InvalidInput)?;

        Self::remove_delegation_power(env, &record.delegatee, record.amount)?;
        Self::delete_delegation(env, delegator);

        Ok(())
    }

    /// Read the active delegation for a delegator (read-only).
    pub fn get_delegation(env: &Env, delegator: &Address) -> Option<Delegation> {
        let key = DelegationKey::Delegation(delegator.clone());
        let val: Option<Delegation> = env.storage().instance().get(&key);
        if val.is_some() {
            Self::bump_ttl(env, &key);
        }
        val
    }

    /// Total delegated power received by a delegatee (read-only).
    pub fn get_delegated_power(env: &Env, delegatee: &Address) -> i128 {
        let key = DelegationKey::DelegatedPower(delegatee.clone());
        let val: i128 = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or(0);
        if val > 0 {
            Self::bump_ttl(env, &key);
        }
        val
    }

    /// Global total voting power (read-only).  Defaults to 0 when unset.
    pub fn get_total_voting_power(env: &Env) -> i128 {
        let key = DelegationKey::TotalVotingPower;
        let val: i128 = env.storage().instance().get(&key).unwrap_or(0);
        if val > 0 {
            Self::bump_ttl(env, &key);
        }
        val
    }

    /// Update the global total voting power (e.g. when new votes are cast
    /// outside the delegation system).  Called by the voting module.
    pub fn set_total_voting_power(env: &Env, total: i128) -> Result<(), Error> {
        let key = DelegationKey::TotalVotingPower;
        env.storage().instance().set(&key, &total);
        Self::bump_ttl(env, &key);
        Ok(())
    }

    /// Compute the per-address delegation cap given the total voting power.
    ///
    /// `cap = total_power * MAX_DELEGATION_BPS / BPS_DENOMINATOR`
    pub fn delegation_cap(total_power: i128) -> i128 {
        if total_power <= 0 {
            return 0;
        }
        total_power
            .checked_mul(MAX_DELEGATION_BPS)
            .unwrap_or(i128::MAX)
            .checked_div(BPS_DENOMINATOR)
            .unwrap_or(0)
    }

    // -----------------------------------------------------------------------
    //  Internal helpers
    // -----------------------------------------------------------------------

    fn store_delegation(env: &Env, record: &Delegation) {
        let key = DelegationKey::Delegation(record.delegator.clone());
        env.storage().instance().set(&key, record);
        Self::bump_ttl(env, &key);
    }

    fn delete_delegation(env: &Env, delegator: &Address) {
        let key = DelegationKey::Delegation(delegator.clone());
        env.storage().instance().remove(&key);
    }

    fn add_delegated_power(
        env: &Env,
        delegatee: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        let key = DelegationKey::DelegatedPower(delegatee.clone());
        let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
        let new_val = current.checked_add(amount).ok_or(Error::Overflow)?;
        env.storage().instance().set(&key, &new_val);
        Self::bump_ttl(env, &key);
        Ok(())
    }

    fn remove_delegation_power(
        env: &Env,
        delegatee: &Address,
        amount: i128,
    ) -> Result<(), Error> {
        let key = DelegationKey::DelegatedPower(delegatee.clone());
        let current: i128 = env.storage().instance().get(&key).unwrap_or(0);
        let new_val = current.checked_sub(amount).unwrap_or(0);
        env.storage().instance().set(&key, &new_val);
        Self::bump_ttl(env, &key);
        Ok(())
    }

    fn bump_ttl(env: &Env, key: &DelegationKey) {
        env.storage()
            .instance()
            .extend_ttl(key, DELEGATION_TTL_THRESHOLD, DELEGATION_TTL_TO);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    #[test]
    fn test_set_delegate_basic() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        // Set total voting power so the cap is non-zero.
        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        DelegationManager::set_delegate(&env, &delegator, &delegatee, 50_000).unwrap();

        let record = DelegationManager::get_delegation(&env, &delegator);
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.delegator, delegator);
        assert_eq!(record.delegatee, delegatee);
        assert_eq!(record.amount, 50_000);

        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 50_000);
    }

    #[test]
    fn test_unset_delegate_releases_power() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();
        DelegationManager::set_delegate(&env, &delegator, &delegatee, 50_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 50_000);

        DelegationManager::unset_delegate(&env, &delegator).unwrap();

        assert!(DelegationManager::get_delegation(&env, &delegator).is_none());
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 0);
    }

    #[test]
    fn test_cap_exactly_10_percent_allowed() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        // Total voting power = 1 000 000 → cap = 100 000 (10 %).
        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        // Delegating exactly the cap should succeed.
        DelegationManager::set_delegate(&env, &delegator, &delegatee, 100_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 100_000);
    }

    #[test]
    fn test_cap_exceeded_rejected() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        // Total voting power = 1 000 000 → cap = 100 000 (10 %).
        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        // Delegating 100 001 (just over the cap) should fail.
        let result =
            DelegationManager::set_delegate(&env, &delegator, &delegatee, 100_001);
        assert_eq!(result, Err(Error::DelegationCapExceeded));
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 0);
    }

    #[test]
    fn test_multiple_delegators_cap_enforced() {
        let env = setup_env();
        let delegatee = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        // First delegator: 60 000 (under cap).
        let d1 = Address::generate(&env);
        DelegationManager::set_delegate(&env, &d1, &delegatee, 60_000).unwrap();

        // Second delegator: 40 000 → total 100 000 (exactly at cap, OK).
        let d2 = Address::generate(&env);
        DelegationManager::set_delegate(&env, &d2, &delegatee, 40_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 100_000);

        // Third delegator: 1 more → over cap, rejected.
        let d3 = Address::generate(&env);
        let result = DelegationManager::set_delegate(&env, &d3, &delegatee, 1);
        assert_eq!(result, Err(Error::DelegationCapExceeded));
    }

    #[test]
    fn test_redelegate_updates_power() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee_a = Address::generate(&env);
        let delegatee_b = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        DelegationManager::set_delegate(&env, &delegator, &delegatee_a, 50_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee_a), 50_000);

        // Re-delegate to a different delegatee.
        DelegationManager::set_delegate(&env, &delegator, &delegatee_b, 30_000).unwrap();

        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee_a), 0);
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee_b), 30_000);
        let record = DelegationManager::get_delegation(&env, &delegator).unwrap();
        assert_eq!(record.delegatee, delegatee_b);
        assert_eq!(record.amount, 30_000);
    }

    #[test]
    fn test_set_delegate_rejects_self_delegation() {
        let env = setup_env();
        let delegator = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        let result = DelegationManager::set_delegate(&env, &delegator, &delegator, 50_000);
        assert_eq!(result, Err(Error::InvalidInput));
    }

    #[test]
    fn test_set_delegate_rejects_circular_delegation() {
        let env = setup_env();
        let addr_a = Address::generate(&env);
        let addr_b = Address::generate(&env);
        let addr_c = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        DelegationManager::set_delegate(&env, &addr_a, &addr_b, 10_000).unwrap();
        DelegationManager::set_delegate(&env, &addr_b, &addr_c, 10_000).unwrap();

        let result = DelegationManager::set_delegate(&env, &addr_c, &addr_a, 10_000);
        assert_eq!(result, Err(Error::DelegationCircular));
    }

    #[test]
    fn test_set_delegate_rejects_zero_stake() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        let result = DelegationManager::set_delegate(&env, &delegator, &delegatee, 0);
        assert_eq!(result, Err(Error::InvalidInput));
    }

    #[test]
    fn test_unset_delegate_no_active_delegation() {
        let env = setup_env();
        let delegator = Address::generate(&env);

        let result = DelegationManager::unset_delegate(&env, &delegator);
        assert_eq!(result, Err(Error::InvalidInput));
    }

    #[test]
    fn test_delegation_cap_zero_total_power() {
        // When total voting power is 0, the cap is 0 — no delegation allowed.
        assert_eq!(DelegationManager::delegation_cap(0), 0);
    }

    #[test]
    fn test_delegation_cap_calculation() {
        // 10 % of 1 000 000 = 100 000
        assert_eq!(DelegationManager::delegation_cap(1_000_000), 100_000);
        // 10 % of 500 = 50
        assert_eq!(DelegationManager::delegation_cap(500), 50);
        // 10 % of 10 = 1
        assert_eq!(DelegationManager::delegation_cap(10), 1);
    }

    #[test]
    fn test_idempotent_same_delegation() {
        let env = setup_env();
        let delegator = Address::generate(&env);
        let delegatee = Address::generate(&env);

        DelegationManager::set_total_voting_power(&env, 1_000_000).unwrap();

        // First call stores the delegation.
        DelegationManager::set_delegate(&env, &delegator, &delegatee, 50_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 50_000);

        // Second call with same params is idempotent.
        DelegationManager::set_delegate(&env, &delegator, &delegatee, 50_000).unwrap();
        assert_eq!(DelegationManager::get_delegated_power(&env, &delegatee), 50_000);
    }
}
