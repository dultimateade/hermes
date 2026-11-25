//! Validation Tests for Oracle Configuration
//!
//! This module contains tests that verify the strict validation logic for
//! oracle configurations, including provider support and provider-specific
//! feed ID constraints.

use super::*;
use crate::markets::{MarketPauseManager, MarketStateManager, MarketValidator};
use crate::oracles::OracleValidationConfigManager;
use crate::types::MarketPauseInfo;
use soroban_sdk::{Env, String, Address, Symbol, Vec, Map, IntoVal, vec};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_oracle_provider_validation() {
    let env = Env::default();

    // Reflector is supported on Stellar
    let reflector = OracleProvider::reflector();
    assert!(reflector.validate_for_market(&env).is_ok());

    // Pyth is known but not yet supported on Stellar
    let pyth = OracleProvider::pyth();
    let pyth_result = pyth.validate_for_market(&env);
    assert!(pyth_result.is_err());
    assert_eq!(pyth_result.unwrap_err(), Error::InvalidOracleProvider);

    // Band Protocol is not supported on Stellar
    let band = OracleProvider::band_protocol();
    let band_result = band.validate_for_market(&env);
    assert!(band_result.is_err());
    assert_eq!(band_result.unwrap_err(), Error::InvalidOracleProvider);
}

#[test]
fn test_oracle_factory_rejects_unregistered_provider() {
    let env = Env::default();
    let provider = OracleProvider::from_str(&env, String::from_str(&env, "unregistered"));

    assert!(!provider.is_known());
    assert_eq!(
        crate::oracles::OracleFactory::create_oracle(provider, Address::generate(&env)).err(),
        Some(Error::InvalidOracleProvider)
    );
}

#[test]
fn test_oracle_config_impossible_combinations() {
    let env = Env::default();
    let oracle_address = Address::generate(&env);

    // 1. Reflector with Pyth-like hex feed ID (impossible)
    let reflector_invalid = OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef12345678"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    let result = reflector_invalid.validate(&env);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidOracleFeed);

    // 2. Pyth with short feed ID (impossible)
    let pyth_invalid = OracleConfig {
        provider: OracleProvider::pyth(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "BTC/USD"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    let result = pyth_invalid.validate(&env);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidOracleFeed);

    // 3. Band with long feed ID (impossible)
    let band_invalid = OracleConfig {
        provider: OracleProvider::band_protocol(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef12345678"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    let result = band_invalid.validate(&env);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidOracleFeed);
}

fn oracle_config_with(
    env: &Env,
    provider: OracleProvider,
    feed_id: &str,
    threshold: i128,
    comparison: &str,
) -> OracleConfig {
    OracleConfig::new(
        provider,
        Address::generate(env),
        String::from_str(env, feed_id),
        threshold,
        String::from_str(env, comparison),
    )
}

#[test]
fn test_oracle_config_rejects_sentinel_with_invalid_oracle_config() {
    let env = Env::default();
    let sentinel = OracleConfig::none_sentinel(&env);
    assert_eq!(sentinel.validate(&env), Err(Error::InvalidOracleConfig));
}

#[test]
fn test_oracle_config_rejects_empty_feed_with_invalid_oracle_feed() {
    let env = Env::default();
    let config = oracle_config_with(&env, OracleProvider::reflector(), "", 100, "gt");
    assert_eq!(config.validate(&env), Err(Error::InvalidOracleFeed));
}

#[test]
fn test_oracle_config_rejects_non_positive_threshold_with_invalid_threshold() {
    let env = Env::default();
    for threshold in [0, -1] {
        let config = oracle_config_with(
            &env,
            OracleProvider::reflector(),
            "BTC/USD",
            threshold,
            "gt",
        );
        assert_eq!(config.validate(&env), Err(Error::InvalidThreshold));
    }
}

#[test]
fn test_oracle_config_rejects_unknown_comparison_with_invalid_comparison() {
    let env = Env::default();
    let config = oracle_config_with(&env, OracleProvider::reflector(), "BTC/USD", 100, "gte");
    assert_eq!(config.validate(&env), Err(Error::InvalidComparison));
}

#[test]
fn test_oracle_config_rejects_unsupported_provider_with_invalid_oracle_provider() {
    let env = Env::default();
    let config = oracle_config_with(&env, OracleProvider::dia(), "BTC/USD", 100, "gt");
    assert_eq!(config.validate(&env), Err(Error::InvalidOracleProvider));
}

#[test]
fn test_market_validator_reports_granular_oracle_errors() {
    let env = Env::default();

    let zero_threshold = oracle_config_with(&env, OracleProvider::reflector(), "BTC/USD", 0, "gt");
    assert_eq!(
        MarketValidator::validate_oracle_config(&env, &zero_threshold),
        Err(Error::InvalidThreshold)
    );

    let empty_feed = oracle_config_with(&env, OracleProvider::reflector(), "", 100, "gt");
    assert_eq!(
        MarketValidator::validate_oracle_config(&env, &empty_feed),
        Err(Error::InvalidOracleFeed)
    );

    let empty_comparison =
        oracle_config_with(&env, OracleProvider::reflector(), "BTC/USD", 100, "");
    assert_eq!(
        MarketValidator::validate_oracle_config(&env, &empty_comparison),
        Err(Error::InvalidComparison)
    );

    let valid = oracle_config_with(&env, OracleProvider::reflector(), "BTC/USD", 100, "gt");
    assert!(MarketValidator::validate_oracle_config(&env, &valid).is_ok());
}

#[test]
fn test_oracle_config_errors_expose_distinct_codes() {
    let errors = [
        Error::InvalidOracleConfig,
        Error::InvalidOracleProvider,
        Error::InvalidOracleFeed,
        Error::InvalidThreshold,
        Error::InvalidComparison,
    ];

    for (index, error) in errors.iter().enumerate() {
        for other in errors.iter().skip(index + 1) {
            assert_ne!(*error as u32, *other as u32);
            assert_ne!(error.code(), other.code());
            assert_ne!(error.description(), other.description());
        }
    }

    assert_eq!(Error::InvalidOracleProvider as u32, 215);
    assert_eq!(
        Error::InvalidOracleProvider.code(),
        "INVALID_ORACLE_PROVIDER"
    );
}

#[test]
fn test_oracle_factory_stellar_compatibility() {
    let env = Env::default();
    let oracle_address = Address::generate(&env);

    // Valid Reflector config
    let reflector_valid = OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "BTC/USD"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    assert!(crate::oracles::OracleFactory::validate_stellar_compatibility(&reflector_valid).is_ok());

    // Invalid Reflector (long ID)
    let reflector_invalid = OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef12345678"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    assert!(crate::oracles::OracleFactory::validate_stellar_compatibility(&reflector_invalid).is_err());

    // Pyth with valid ID (passes compatibility check but would fail validate_for_market)
    let pyth_valid_id = OracleConfig {
        provider: OracleProvider::pyth(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef12345678"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    assert!(crate::oracles::OracleFactory::validate_stellar_compatibility(&pyth_valid_id).is_ok());

    // Band Protocol (rejected by compatibility check)
    let band_config = OracleConfig {
        provider: OracleProvider::band_protocol(),
        oracle_address: oracle_address.clone(),
        feed_id: String::from_str(&env, "BTC/USD"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    assert!(crate::oracles::OracleFactory::validate_stellar_compatibility(&band_config).is_err());
}

// ============================================================================
// Auto-pause on oracle validation failure tests
// ============================================================================

/// Helper: set up contract environment with a stored market and admin.
fn setup_auto_pause_env(env: &Env, contract_id: &Address) -> Symbol {
    let admin = Address::generate(env);
    let market_id = Symbol::new(env, "auto_pause_market");

    env.as_contract(contract_id, || {
        env.storage().persistent().set(&Symbol::new(env, "Admin"), &admin);

        let market = Market {
            admin: admin.clone(),
            question: String::from_str(env, "Test?"),
            outcomes: Vec::from_array(env, [String::from_str(env, "yes"), String::from_str(env, "no")]),
            end_time: env.ledger().timestamp() + 3600,
            oracle_config: OracleConfig::new(
                OracleProvider::reflector(),
                Address::generate(env),
                String::from_str(env, "BTC/USD"),
                50_000_00,
                String::from_str(env, "gt"),
            ),
            metadata_commitment: BytesN::from_array(env, &[0u8; 32]),
            has_fallback: false,
            fallback_oracle_config: OracleConfig::none_sentinel(env),
            resolution_timeout: 86400,
            oracle_result: None,
            votes: Map::new(env),
            stakes: Map::new(env),
            claimed: Map::new(env),
            total_staked: 0,
            dispute_stakes: Map::new(env),
            winning_outcomes: None,
            fee_collected: false,
            state: MarketState::Active,
            total_extension_days: 0,
            max_extension_days: 30,
            extension_history: Vec::new(env),
            category: None,
            tags: Vec::new(env),
            min_pool_size: None,
            bet_deadline: 0,
            dispute_window_seconds: 86400,
            winnings_swept: false,
            timelock_config: crate::timelock::MarketTimelockConfig::default(),
            dispute_stake_floor: None,
            max_participants: None,
        };
        env.storage().persistent().set(&market_id, &market);
    });

    market_id
}

#[test]
fn test_auto_pause_config_validation_accepts_valid() {
    let env = Env::default();
    let config = GlobalOracleValidationConfig {
        max_staleness_secs: 60,
        max_confidence_bps: 500,
        max_deviation_bps: None,
        max_deviation_z_multiple: None,
        history_size: None,
        auto_pause_duration_secs: Some(3600),
    };
    assert!(OracleValidationConfigManager::set_global_config(&env, &config).is_ok());
}

#[test]
fn test_auto_pause_config_validation_rejects_zero() {
    let env = Env::default();
    let config = GlobalOracleValidationConfig {
        max_staleness_secs: 60,
        max_confidence_bps: 500,
        max_deviation_bps: None,
        max_deviation_z_multiple: None,
        history_size: None,
        auto_pause_duration_secs: Some(0),
    };
    let result = OracleValidationConfigManager::set_global_config(&env, &config);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidInput);
}

#[test]
fn test_auto_pause_config_validation_rejects_too_large() {
    let env = Env::default();
    let config = GlobalOracleValidationConfig {
        max_staleness_secs: 60,
        max_confidence_bps: 500,
        max_deviation_bps: None,
        max_deviation_z_multiple: None,
        history_size: None,
        auto_pause_duration_secs: Some(604_801),
    };
    let result = OracleValidationConfigManager::set_global_config(&env, &config);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), Error::InvalidInput);
}

#[test]
fn test_auto_pause_staleness_triggers_pause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        env.ledger().with_mut(|li| li.timestamp = 100);

        let config = GlobalOracleValidationConfig {
            max_staleness_secs: 10,
            max_confidence_bps: 500,
            max_deviation_bps: None,
            max_deviation_z_multiple: None,
            history_size: None,
            auto_pause_duration_secs: Some(3600),
        };
        OracleValidationConfigManager::set_global_config(&env, &config).unwrap();

        let data = OraclePriceData {
            price: 100_00,
            publish_time: env.ledger().timestamp().saturating_sub(20),
            confidence: None,
            exponent: 0,
        };

        let result = OracleValidationConfigManager::validate_oracle_data(
            &env, &market_id, &OracleProvider::reflector(),
            &String::from_str(&env, "BTC/USD"), &data,
        );
        assert_eq!(result.unwrap_err(), Error::OracleStale);
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

#[test]
fn test_auto_pause_none_does_not_pause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        env.ledger().with_mut(|li| li.timestamp = 100);

        let config = GlobalOracleValidationConfig {
            max_staleness_secs: 10,
            max_confidence_bps: 500,
            max_deviation_bps: None,
            max_deviation_z_multiple: None,
            history_size: None,
            auto_pause_duration_secs: None,
        };
        OracleValidationConfigManager::set_global_config(&env, &config).unwrap();

        let data = OraclePriceData {
            price: 100_00,
            publish_time: env.ledger().timestamp().saturating_sub(20),
            confidence: None,
            exponent: 0,
        };

        let result = OracleValidationConfigManager::validate_oracle_data(
            &env, &market_id, &OracleProvider::reflector(),
            &String::from_str(&env, "BTC/USD"), &data,
        );
        assert_eq!(result.unwrap_err(), Error::OracleStale);
        assert!(!MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

#[test]
fn test_auto_pause_already_paused_is_noop() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        let admin: Address = env.storage().persistent().get(&Symbol::new(&env, "Admin")).unwrap();
        MarketPauseManager::pause_market(&env, admin, &market_id, 1).unwrap();
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());

        // Second auto-pause should be a no-op
        MarketPauseManager::auto_pause_market(&env, &market_id, 3600).unwrap();
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

#[test]
fn test_market_unpause_cool_off() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        let admin: Address = env.storage().persistent().get(&Symbol::new(&env, "Admin")).unwrap();
        
        // Initial state: not paused
        assert!(!MarketPauseManager::is_market_paused(&env, &market_id).unwrap());

        // Set initial ledger timestamp
        env.ledger().with_mut(|li| li.timestamp = 10_000);

        // Pause the market for 24 hours
        MarketPauseManager::pause_market(&env, admin.clone(), &market_id, 24).unwrap();
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());

        // Attempting to resume immediately (0 seconds passed) should fail with CoolOffPeriodActive
        let err = MarketPauseManager::resume_market(&env, admin.clone(), &market_id).unwrap_err();
        assert_eq!(err, Error::CoolOffPeriodActive);

        // Fast forward 6 days (518,400 seconds passed)
        env.ledger().with_mut(|li| li.timestamp = 10_000 + 6 * 24 * 3600);
        let err = MarketPauseManager::resume_market(&env, admin.clone(), &market_id).unwrap_err();
        assert_eq!(err, Error::CoolOffPeriodActive);

        // Fast forward to exactly 7 days (604,800 seconds passed)
        env.ledger().with_mut(|li| li.timestamp = 10_000 + 7 * 24 * 3600);
        MarketPauseManager::resume_market(&env, admin.clone(), &market_id).unwrap();

        // Resumed successfully
        assert!(!MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

#[test]
fn test_auto_pause_config_per_event_override() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        env.ledger().with_mut(|li| li.timestamp = 100);

        let global = GlobalOracleValidationConfig {
            max_staleness_secs: 60,
            max_confidence_bps: 500,
            max_deviation_bps: None,
            max_deviation_z_multiple: None,
            history_size: None,
            auto_pause_duration_secs: None,
        };
        OracleValidationConfigManager::set_global_config(&env, &global).unwrap();

        let event_cfg = EventOracleValidationConfig {
            max_staleness_secs: 5,
            max_confidence_bps: 500,
            max_deviation_bps: None,
            max_deviation_z_multiple: None,
            history_size: None,
            auto_pause_duration_secs: Some(7200),
        };
        OracleValidationConfigManager::set_event_config(&env, &market_id, &event_cfg).unwrap();

        let data = OraclePriceData {
            price: 100_00,
            publish_time: env.ledger().timestamp().saturating_sub(10),
            confidence: None,
            exponent: 0,
        };

        let result = OracleValidationConfigManager::validate_oracle_data(
            &env, &market_id, &OracleProvider::reflector(),
            &String::from_str(&env, "BTC/USD"), &data,
        );
        assert_eq!(result.unwrap_err(), Error::OracleStale);
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

#[test]
fn test_auto_pause_deviation_spike_triggers_pause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, crate::PredictifyHybrid);
    let market_id = setup_auto_pause_env(&env, &contract_id);

    env.as_contract(&contract_id, || {
        let config = GlobalOracleValidationConfig {
            max_staleness_secs: 60,
            max_confidence_bps: 500,
            max_deviation_bps: Some(500),
            max_deviation_z_multiple: None,
            history_size: None,
            auto_pause_duration_secs: Some(3600),
        };
        OracleValidationConfigManager::set_global_config(&env, &config).unwrap();

        let first = OraclePriceData {
            price: 100_000_00,
            publish_time: env.ledger().timestamp(),
            confidence: None,
            exponent: 0,
        };
        OracleValidationConfigManager::validate_oracle_data(
            &env, &market_id, &OracleProvider::reflector(),
            &String::from_str(&env, "BTC"), &first,
        ).unwrap();

        let spike = OraclePriceData {
            price: 200_000_00,
            publish_time: env.ledger().timestamp(),
            confidence: None,
            exponent: 0,
        };
        let result = OracleValidationConfigManager::validate_oracle_data(
            &env, &market_id, &OracleProvider::reflector(),
            &String::from_str(&env, "BTC"), &spike,
        );
        assert_eq!(result.unwrap_err(), Error::OracleNoConsensus);
        assert!(MarketPauseManager::is_market_paused(&env, &market_id).unwrap());
    });
}

// ============================================================================
// Oracle Address Allowlist & Anti-Spoofing Tests (Issue #033)
// ============================================================================

#[test]
fn test_spoofed_oracle_rejected_when_allowlist_configured() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let trusted_oracle = Address::generate(&env);
    let spoofed_oracle = Address::generate(&env);

    // Register trusted oracle in allowlist
    crate::oracles::OracleRegistry::add_allowed_oracle(&env, &admin, &trusted_oracle).unwrap();

    // Valid config with trusted oracle
    let valid_cfg = OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: trusted_oracle.clone(),
        feed_id: String::from_str(&env, "BTC/USD"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    assert!(valid_cfg.validate(&env).is_ok());
    assert!(MarketValidator::validate_oracle_config(&env, &valid_cfg).is_ok());

    // Spoofed oracle address must be rejected
    let spoofed_cfg = OracleConfig {
        provider: OracleProvider::reflector(),
        oracle_address: spoofed_oracle.clone(),
        feed_id: String::from_str(&env, "BTC/USD"),
        threshold: 100,
        comparison: String::from_str(&env, "gt"),
    };
    let validation_res = spoofed_cfg.validate(&env);
    assert_eq!(validation_res, Err(Error::InvalidOracleConfig));
    assert_eq!(MarketValidator::validate_oracle_config(&env, &spoofed_cfg), Err(Error::InvalidOracleConfig));
}

#[test]
fn test_oracle_allowlist_management() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let oracle1 = Address::generate(&env);
    let oracle2 = Address::generate(&env);

    // Initially unconfigured
    assert!(crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle1));

    // Add oracle1
    crate::oracles::OracleRegistry::add_allowed_oracle(&env, &admin, &oracle1).unwrap();
    assert!(crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle1));
    assert!(!crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle2));

    // Add oracle2
    crate::oracles::OracleRegistry::add_allowed_oracle(&env, &admin, &oracle2).unwrap();
    assert!(crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle2));

    let list = crate::oracles::OracleRegistry::list_allowed_oracles(&env);
    assert_eq!(list.len(), 2);

    // Remove oracle1
    crate::oracles::OracleRegistry::remove_allowed_oracle(&env, &admin, &oracle1).unwrap();
    assert!(!crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle1));
    assert!(crate::oracles::OracleRegistry::is_oracle_allowed(&env, &oracle2));
}

// ============================================================================
// Oracle Price Validation Tests (Issue #034)
// ============================================================================

#[test]
fn test_oracle_data_rejects_negative_or_zero_price() {
    let env = Env::default();
    let market_id = Symbol::new(&env, "test_mkt");
    let provider = OracleProvider::reflector();
    let feed_id = String::from_str(&env, "BTC/USD");

    // Negative price (e.g., from an integer overflow/sign-wrap) must be rejected
    let negative_price_data = ReflectorPriceData {
        price: -1,
        timestamp: env.ledger().timestamp(),
        source: String::from_str(&env, "reflector"),
    };
    let result = OracleValidationConfigManager::validate_oracle_data(
        &env,
        &market_id,
        &provider,
        &feed_id,
        &negative_price_data,
    );
    assert_eq!(result, Err(Error::InvalidInput));

    // Zero price must be rejected
    let zero_price_data = ReflectorPriceData {
        price: 0,
        timestamp: env.ledger().timestamp(),
        source: String::from_str(&env, "reflector"),
    };
    let result = OracleValidationConfigManager::validate_oracle_data(
        &env,
        &market_id,
        &provider,
        &feed_id,
        &zero_price_data,
    );
    assert_eq!(result, Err(Error::InvalidInput));

    // Valid positive price must succeed
    let valid_price_data = ReflectorPriceData {
        price: 50_000_00,
        timestamp: env.ledger().timestamp(),
        source: String::from_str(&env, "reflector"),
    };
    let result = OracleValidationConfigManager::validate_oracle_data(
        &env,
        &market_id,
        &provider,
        &feed_id,
        &valid_price_data,
    );
    assert!(result.is_ok());
}


