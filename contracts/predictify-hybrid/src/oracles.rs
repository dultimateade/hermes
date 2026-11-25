#![no_std]

use soroban_sdk::{
    contracttype, symbol_short, Address, Env, IntoVal, String, Symbol, Vec,
};
use crate::err::Error;
use crate::types::{OracleConfig, OracleProvider, ReflectorPriceData};

// ============================================================
// Storage Keys
// ============================================================

const SYM_ALLOWED_ORACLES: &str = "AllowedOracles";

// ============================================================
// OracleRegistry
// ============================================================

/// Registry of approved / allowlisted oracle contract addresses.
///
/// Prevents malicious market creators from pointing resolution at
/// spoofed or unauthorized contracts returning predetermined outcomes.
pub struct OracleRegistry;

impl OracleRegistry {
    /// Loads the list of allowlisted oracle contract addresses.
    pub fn list_allowed_oracles(env: &Env) -> Vec<Address> {
        let key = Symbol::new(env, SYM_ALLOWED_ORACLES);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Stores the list of allowlisted oracle addresses.
    fn save_allowed_oracles(env: &Env, list: &Vec<Address>) {
        let key = Symbol::new(env, SYM_ALLOWED_ORACLES);
        env.storage().persistent().set(&key, list);
    }

    /// Adds an oracle address to the authorized allowlist (admin only).
    pub fn add_allowed_oracle(env: &Env, admin: &Address, oracle: &Address) -> Result<(), Error> {
        admin.require_auth();
        let mut list = Self::list_allowed_oracles(env);
        if !list.contains(oracle) {
            list.push_back(oracle.clone());
            Self::save_allowed_oracles(env, &list);
        }
        Ok(())
    }

    /// Removes an oracle address from the authorized allowlist (admin only).
    pub fn remove_allowed_oracle(env: &Env, admin: &Address, oracle: &Address) -> Result<(), Error> {
        admin.require_auth();
        let list = Self::list_allowed_oracles(env);
        let mut filtered: Vec<Address> = Vec::new(env);
        for a in list.iter() {
            if &a != oracle {
                filtered.push_back(a);
            }
        }
        Self::save_allowed_oracles(env, &filtered);
        Ok(())
    }

    /// Verifies whether an oracle address is approved.
    ///
    /// When the allowlist is configured with at least one oracle, only explicitly
    /// registered oracles are permitted.
    pub fn is_oracle_allowed(env: &Env, oracle: &Address) -> bool {
        let list = Self::list_allowed_oracles(env);
        if list.is_empty() {
            return true;
        }
        list.contains(oracle)
    }

    /// Asserts that the oracle address is authorized; returns `Error::InvalidOracleConfig` if not.
    pub fn require_oracle_allowed(env: &Env, oracle: &Address) -> Result<(), Error> {
        if !Self::is_oracle_allowed(env, oracle) {
            return Err(Error::InvalidOracleConfig);
        }
        Ok(())
    }
}

// ============================================================
// OracleClient & OracleFactory
// ============================================================

pub struct OracleClient {
    pub provider: OracleProvider,
    pub oracle_address: Address,
}

impl OracleClient {
    pub fn get_price_data(&self, env: &Env, feed_id: &String) -> Result<ReflectorPriceData, Error> {
        // Enforce allowlist check before external invocation
        OracleRegistry::require_oracle_allowed(env, &self.oracle_address)?;

        let full: Option<ReflectorPriceData> = env.invoke_contract(
            &self.oracle_address,
            &symbol_short!("get_pdata"),
            soroban_sdk::vec![env, feed_id.clone().into_val(env)],
        );
        if let Some(data) = full {
            return Ok(data);
        }

        let price: Option<i128> = env.invoke_contract(
            &self.oracle_address,
            &symbol_short!("get_price"),
            soroban_sdk::vec![env, feed_id.clone().into_val(env)],
        );
        if let Some(p) = price {
            return Ok(ReflectorPriceData {
                price: p,
                timestamp: env.ledger().timestamp(),
            });
        }

        Err(Error::OracleUnavailable)
    }
}

pub struct OracleFactory;

impl OracleFactory {
    pub fn create_oracle(
        provider: OracleProvider,
        oracle_address: Address,
    ) -> Result<OracleClient, Error> {
        if !provider.is_supported() {
            return Err(Error::InvalidOracleProvider);
        }
        Ok(OracleClient {
            provider,
            oracle_address,
        })
    }

    pub fn validate_stellar_compatibility(config: &OracleConfig) -> Result<(), Error> {
        let provider_str = config.provider.as_str();
        let feed_id_len = config.feed_id.len();

        if provider_str == "reflector" {
            if feed_id_len >= 64 {
                return Err(Error::InvalidOracleFeed);
            }
        } else if provider_str == "pyth" {
            if feed_id_len < 64 || feed_id_len > 66 {
                return Err(Error::InvalidOracleFeed);
            }
        } else if provider_str == "band_protocol" || provider_str == "dia" {
            return Err(Error::InvalidOracleProvider);
        }
        Ok(())
    }
}

// ============================================================
// OracleValidationConfigManager
// ============================================================

pub struct OracleValidationConfigManager;

impl OracleValidationConfigManager {
    pub fn validate_oracle_data(
        env: &Env,
        _market_id: &Symbol,
        _provider: &OracleProvider,
        _feed_id: &String,
        price_data: &ReflectorPriceData,
    ) -> Result<(), Error> {
        if price_data.price <= 0 {
            return Err(Error::InvalidInput);
        }
        let now = env.ledger().timestamp();
        // Allow up to 1 hour staleness
        if now > price_data.timestamp && now.saturating_sub(price_data.timestamp) > 3600 {
            return Err(Error::OracleStale);
        }
        Ok(())
    }
}

// ============================================================
// OracleUtils
// ============================================================

pub struct OracleUtils;

impl OracleUtils {
    pub fn determine_outcome(
        price: i128,
        threshold: i128,
        comparison: &String,
        env: &Env,
    ) -> Result<String, Error> {
        let gt = String::from_str(env, "gt");
        let lt = String::from_str(env, "lt");
        let eq = String::from_str(env, "eq");

        let condition_met = if comparison == &gt {
            price > threshold
        } else if comparison == &lt {
            price < threshold
        } else if comparison == &eq {
            price == threshold
        } else {
            return Err(Error::InvalidComparison);
        };

        if condition_met {
            Ok(String::from_str(env, "yes"))
        } else {
            Ok(String::from_str(env, "no"))
        }
    }

    pub fn resolve_outcome_with_fallback(
        primary_outcome: &String,
        fallback_outcome: &String,
        _env: &Env,
    ) -> Result<String, Error> {
        if primary_outcome == fallback_outcome {
            Ok(primary_outcome.clone())
        } else {
            // Fallback outcome takes precedence when primary conflicts
            Ok(fallback_outcome.clone())
        }
    }
}

// ============================================================
// OracleIntegrationManager
// ============================================================

pub struct OracleIntegrationManager;

impl OracleIntegrationManager {
    pub fn set_oracle_weight(
        env: &Env,
        admin: Address,
        oracle: Address,
        weight: u32,
    ) -> Result<(), Error> {
        admin.require_auth();
        let key = Symbol::new(env, "oracle_weight");
        let mut weights: soroban_sdk::Map<Address, u32> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(env));
        weights.set(oracle, weight);
        env.storage().persistent().set(&key, &weights);
        Ok(())
    }

    pub fn get_oracle_weight(env: &Env, oracle: &Address) -> u32 {
        let key = Symbol::new(env, "oracle_weight");
        let weights: soroban_sdk::Map<Address, u32> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| soroban_sdk::Map::new(env));
        weights.get(oracle.clone()).unwrap_or(1u32)
    }

    pub fn add_allowed_oracle(env: &Env, admin: &Address, oracle: &Address) -> Result<(), Error> {
        OracleRegistry::add_allowed_oracle(env, admin, oracle)
    }

    pub fn remove_allowed_oracle(env: &Env, admin: &Address, oracle: &Address) -> Result<(), Error> {
        OracleRegistry::remove_allowed_oracle(env, admin, oracle)
    }

    pub fn list_allowed_oracles(env: &Env) -> Vec<Address> {
        OracleRegistry::list_allowed_oracles(env)
    }

    pub fn is_oracle_allowed(env: &Env, oracle: &Address) -> bool {
        OracleRegistry::is_oracle_allowed(env, oracle)
    }
}
