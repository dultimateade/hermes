#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

/// Minimum byte length required for a commit-reveal preimage.
///
/// Requiring at least 32 bytes of entropy makes an offline brute-force search
/// against the commitment infeasible before the apply window opens.
pub const MIN_PREIMAGE_LEN: u32 = 32;
pub const DEFAULT_COUNCIL_SIZE: u32 = 3;
pub const DEFAULT_COUNCIL_THRESHOLD: u32 = 2;
pub const DEFAULT_QUORUM: u32 = 10;
pub const ESCALATION_STAKE_THRESHOLD: u32 = 1_000_000;

/// A typed council member vote.
///
/// Replaces the former bare `u32` constants (`COUNCIL_VOTE_ABSTAIN`,
/// `COUNCIL_VOTE_UPHOLD`, `COUNCIL_VOTE_OVERTURN`).  Using an enum makes
/// out-of-range values impossible to construct and removes the need for the
/// runtime range-check that was previously missing from `council_vote`.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CouncilVote {
    /// The member abstains; the vote is recorded but does not affect tallies.
    Abstain = 0,
    /// The member votes to uphold the community resolution.
    Uphold = 1,
    /// The member votes to overturn the community resolution.
    Overturn = 2,
}

/// Outcome of a community vote or council resolution.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisputeOutcome {
    /// No binding resolution yet.
    Pending = 0,
    /// Community vote resolved in favor of the challenger.
    ChallengerWins = 1,
    /// Community vote resolved in favor of the defender.
    DefenderWins = 2,
    /// Community vote was split below quorum; escalated to council.
    Escalated = 3,
    /// Arbitration council upheld the community resolution.
    CouncilUpheld = 4,
    /// Arbitration council overturned the community resolution.
    CouncilOverturned = 5,
}

/// Lifecycle status of a dispute.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    /// Dispute opened; voting has not yet begun.
    Open = 0,
    /// Community voting is in progress.
    Voting = 1,
    /// A binding resolution has been recorded.
    Resolved = 2,
    /// The dispute was withdrawn by its creator before voting began.
    Withdrawn = 3,
}

/// Recorded state for an open dispute.
///
/// `creator` is an `Address` so that `withdraw_dispute` can authenticate the
/// caller with `require_auth()` — a `Symbol` string provides no cryptographic
/// binding and would allow anyone to impersonate the creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub id: u32,
    pub market_id: Symbol,
    pub creator: Address,
    pub status: DisputeStatus,
    pub outcome: DisputeOutcome,
}

/// Per-market configuration governing when a dispute may be escalated to the
/// arbitration council.
///
/// Construct with [`EscalationPolicy::new`] to enforce invariants, or use
/// [`DisputesContract::default_escalation_policy`] for the standard defaults.
/// Direct struct construction is still possible for tests and trusted internal
/// callers, but `new` validates inputs at the boundary.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    /// Minimum market stake (in stroops) required for escalation.
    /// Must be greater than zero; set via `EscalationPolicy::new` or
    /// `DisputesContract::default_escalation_policy`.
    pub min_stake: u32,
    /// Minimum community vote turnout required for a binding community vote.
    pub quorum: u32,
    /// Number of council members that must agree for a resolution to bind.
    pub council_threshold: u32,
}

impl EscalationPolicy {
    /// Construct a validated `EscalationPolicy`.
    ///
    /// # Panics
    /// - `min_stake` is zero (every stake would trivially qualify).
    /// - `council_threshold` is zero (a threshold of zero allows resolution
    ///   with no votes, which is meaningless).
    pub fn new(min_stake: u32, quorum: u32, council_threshold: u32) -> Self {
        if min_stake == 0 {
            panic!("EscalationPolicy: min_stake must be greater than zero");
        }
        if council_threshold == 0 {
            panic!("EscalationPolicy: council_threshold must be greater than zero");
        }
        Self {
            min_stake,
            quorum,
            council_threshold,
        }
    }
}

/// Recorded state for an escalated dispute.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationRecord {
    pub market_id: Symbol,
    pub stake: u32,
    pub turnout: u32,
    pub community_outcome: DisputeOutcome,
    pub council_voters: soroban_sdk::Vec<soroban_sdk::Address>,
    pub council_votes: u32,
    pub uphold_votes: u32,
    pub overturn_votes: u32,
    pub resolved: bool,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    DisputeCounter,
    Dispute(u32),
    Escalation(Symbol),
}

fn load_dispute(env: &Env, id: u32) -> Dispute {
    env.storage()
        .persistent()
        .get(&DataKey::Dispute(id))
        .unwrap_or_else(|| panic!("dispute not found"))
}

fn store_dispute(env: &Env, dispute: &Dispute) {
    env.storage()
        .persistent()
        .set(&DataKey::Dispute(dispute.id), dispute);
}

#[contract]
pub struct DisputesContract;

#[contractimpl]
impl DisputesContract {
    pub fn version(_env: Env) -> u32 {
        8
    }

    pub fn validate_preimage(_env: Env, preimage: soroban_sdk::Bytes) -> bool {
        preimage.len() >= MIN_PREIMAGE_LEN
    }

    pub fn default_escalation_policy(_env: Env) -> EscalationPolicy {
        EscalationPolicy::new(
            ESCALATION_STAKE_THRESHOLD,
            DEFAULT_QUORUM,
            DEFAULT_COUNCIL_THRESHOLD,
        )
    }

    /// Construct a validated `EscalationPolicy` with a caller-supplied
    /// `min_stake`.
    ///
    /// This is the runtime-configurable alternative to
    /// `default_escalation_policy`. Use it when different markets require
    /// different escalation thresholds.
    ///
    /// # Panics
    /// - `min_stake` is zero.
    /// - `council_threshold` is zero.
    pub fn new_escalation_policy(
        _env: Env,
        min_stake: u32,
        quorum: u32,
        council_threshold: u32,
    ) -> EscalationPolicy {
        EscalationPolicy::new(min_stake, quorum, council_threshold)
    }

    pub fn is_escalation_eligible(
        _env: Env,
        policy: EscalationPolicy,
        stake: u32,
        turnout: u32,
    ) -> bool {
        stake >= policy.min_stake && turnout < policy.quorum
    }

    pub fn escalate_dispute(
        env: Env,
        policy: EscalationPolicy,
        market_id: Symbol,
        stake: u32,
        turnout: u32,
        community_outcome: DisputeOutcome,
    ) -> EscalationRecord {
        if !Self::is_escalation_eligible(env.clone(), policy, stake, turnout) {
            panic!("market not eligible for escalation");
        }
        let record = EscalationRecord {
            market_id,
            stake,
            turnout,
            community_outcome,
            council_voters: soroban_sdk::Vec::new(&env),
            council_votes: 0,
            uphold_votes: 0,
            overturn_votes: 0,
            resolved: false,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Escalation(record.market_id.clone()), &record);
        record
    }

    pub fn council_vote(
        env: Env,
        policy: EscalationPolicy,
        record: EscalationRecord,
        voter: soroban_sdk::Address,
        vote: CouncilVote,
    ) -> EscalationRecord {
        let mut record: EscalationRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Escalation(record.market_id.clone()))
            .unwrap_or_else(|| panic!("escalation not found"));
        if record.resolved {
            panic!("escalation already resolved");
        }
        if record
            .council_voters
            .iter()
            .any(|member| member == voter.clone())
        {
            panic!("council member has already voted");
        }
        match vote {
            CouncilVote::Abstain => {}
            CouncilVote::Uphold => {
                record.uphold_votes += 1;
            }
            CouncilVote::Overturn => {
                record.overturn_votes += 1;
            }
        }
        record.council_voters.push_back(voter);
        record.council_votes += 1;
        if record.uphold_votes >= policy.council_threshold {
            record.resolved = true;
        } else if record.overturn_votes >= policy.council_threshold {
            record.resolved = true;
        }
        env.storage()
            .persistent()
            .set(&DataKey::Escalation(record.market_id.clone()), &record);
        record
    }

    /// Returns the persisted escalation record for a market, if one exists.
    pub fn get_escalation(env: Env, market_id: Symbol) -> Option<EscalationRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Escalation(market_id))
    }

    pub fn council_outcome(_env: Env, record: EscalationRecord) -> DisputeOutcome {
        if !record.resolved {
            panic!("council has not resolved the dispute");
        }
        if record.uphold_votes >= record.overturn_votes {
            DisputeOutcome::CouncilUpheld
        } else {
            DisputeOutcome::CouncilOverturned
        }
    }

    /// Returns a dispute by its persistent ID, if it exists.
    pub fn get_dispute(env: Env, id: u32) -> Option<Dispute> {
        env.storage().persistent().get(&DataKey::Dispute(id))
    }

    /// Open a new dispute for a market.
    ///
    /// The dispute begins in the `Open` status; community voting has not yet
    /// started. Its allocated ID can be used to retrieve the persisted dispute
    /// and identify it in later lifecycle transitions.
    pub fn open_dispute(env: Env, market_id: Symbol, creator: Address) -> Dispute {
        let previous_id: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::DisputeCounter)
            .unwrap_or(0);
        let id = previous_id
            .checked_add(1)
            .unwrap_or_else(|| panic!("dispute ID overflow"));
        let dispute = Dispute {
            id,
            market_id,
            creator,
            status: DisputeStatus::Open,
            outcome: DisputeOutcome::Pending,
        };
        env.storage()
            .persistent()
            .set(&DataKey::DisputeCounter, &id);
        store_dispute(&env, &dispute);
        dispute
    }

    /// Transition a dispute from `Open` to `Voting`.
    ///
    /// Once voting has begun the dispute can no longer be withdrawn by its
    /// creator. The persisted record is authoritative; only the supplied ID is
    /// used.
    pub fn start_voting(env: Env, dispute: Dispute) -> Dispute {
        let mut dispute = load_dispute(&env, dispute.id);
        if dispute.status != DisputeStatus::Open {
            panic!("dispute is not open");
        }
        dispute.status = DisputeStatus::Voting;
        store_dispute(&env, &dispute);
        dispute
    }

    /// Withdraw an open dispute.
    ///
    /// Only the dispute creator may withdraw, and only while the dispute is
    /// still in the `Open` status (i.e. before community voting has begun).
    /// This prevents a creator from opening a frivolous dispute to delay
    /// resolution and then cancelling it mid-vote to avoid an unfavourable
    /// ruling.
    ///
    /// # Authorization
    /// `caller` must be the address that created the dispute and must
    /// authenticate via `require_auth()`.
    pub fn withdraw_dispute(env: Env, dispute: Dispute, caller: Address) -> Dispute {
        // Cryptographically authenticate the caller before any state check.
        caller.require_auth();

        let mut dispute = load_dispute(&env, dispute.id);
        if caller != dispute.creator {
            panic!("only the dispute creator may withdraw");
        }
        if dispute.status != DisputeStatus::Open {
            panic!("cannot withdraw a dispute once voting has begun");
        }
        dispute.status = DisputeStatus::Withdrawn;
        store_dispute(&env, &dispute);
        dispute
    }

    /// Record a binding resolution for a dispute.
    ///
    /// Only permitted while the dispute is in the `Voting` status.
    pub fn resolve_dispute(
        env: Env,
        dispute: Dispute,
        outcome: DisputeOutcome,
    ) -> Dispute {
        let mut dispute = load_dispute(&env, dispute.id);
        if dispute.status != DisputeStatus::Voting {
            panic!("dispute is not in voting");
        }
        if outcome == DisputeOutcome::Pending {
            panic!("cannot resolve to pending");
        }
        dispute.status = DisputeStatus::Resolved;
        dispute.outcome = outcome;
        store_dispute(&env, &dispute);
        dispute
    }
}
