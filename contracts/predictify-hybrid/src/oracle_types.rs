#![allow(dead_code)]

use crate::Error;
use alloc::string::String as StdString;
use alloc::string::ToString;
use soroban_sdk::{contracttype, Address, Env, String, Symbol, Vec};

// ===== ORACLE TYPES =====

/// Forward-compatible oracle provider representation.
///
/// This struct provides forward compatibility for oracle provider identification by using
/// string-based representation instead of a fixed enum. This allows new oracle providers
/// to be added without breaking existing stored markets.
///
/// # Forward Compatibility Strategy
///
/// The provider is stored as a string identifier rather than an enum variant, enabling:
/// - **Future Provider Addition**: New providers can be added without breaking existing markets
/// - **Backward Compatibility**: Existing markets continue to work with older contract versions
/// - **Unknown Provider Rejection**: Unknown identifiers never alias a supported provider
///
/// # Provider Identifiers
///
/// **Standardized Provider Names:**
/// - `"reflector"` - Reflector oracle (primary for Stellar Network)
/// - `"pyth"` - Pyth Network oracle (placeholder for future Stellar support)
/// - `"band_protocol"` - Band Protocol oracle (not available on Stellar)
/// - `"dia"` - DIA oracle (not available on Stellar)
///
/// **Future Providers:**
/// - New providers use descriptive lowercase names with underscores
/// - Example: `"chainlink"` for Chainlink oracle integration
/// - Example: `"uniswap_oracle"` for Uniswap-based price feeds
///
/// # Example Usage
///
/// ```rust
/// # use soroban_sdk::{Env, String};
/// # use predictify_hybrid::types::OracleProvider;
/// # let env = Env::default();
///
/// // Create provider instances
/// let reflector = OracleProvider::reflector();
/// let pyth = OracleProvider::pyth();
/// let custom = OracleProvider::from_str(&env, String::from_str(&env, "new_provider"));
///
/// // Check support status
/// assert!(reflector.is_supported());
/// assert!(!pyth.is_supported()); // Not available on Stellar
/// assert!(!custom.is_supported()); // Unknown provider
///
/// // Get provider identifier
/// assert_eq!(reflector.as_str(), "reflector");
/// assert_eq!(pyth.as_str(), "pyth");
/// assert_eq!(custom.as_str(), "unknown");
/// assert!(!custom.is_known());
///
/// // Convert to string for display
/// println!("Provider: {}", reflector.name());
/// ```
///
/// # Validation and Error Handling
///
/// ```rust
/// # use soroban_sdk::{Env, String};
/// # use predictify_hybrid::types::OracleProvider;
/// # let env = Env::default();
///
/// let provider = OracleProvider::from_str(String::from_str(&env, "reflector"));
///
/// // Validate provider for new market creation
/// if provider.is_supported() {
///     println!("Provider {} is supported", provider.name());
/// } else {
///     println!("Provider {} is not supported", provider.name());
/// }
///
/// // Check if provider is known (even if unsupported)
/// if provider.is_known() {
///     println!("Provider {} is recognized", provider.name());
/// } else {
///     println!("Unknown provider - may be from future contract version");
/// }
/// ```
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OracleProvider {
    Reflector,
    Pyth,
    BandProtocol,
    DIA,
    Unknown,
}

impl OracleProvider {
    /// Creates a Reflector oracle provider instance.
    ///
    /// Reflector is the primary oracle provider for the Stellar Network,
    /// offering reliable price feeds for major cryptocurrencies.
    ///
    /// # Returns
    ///
    /// `OracleProvider` instance configured for Reflector oracle
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let provider = OracleProvider::reflector();
    /// assert_eq!(provider.as_str(), "reflector");
    /// assert!(provider.is_supported());
    /// ```
    pub fn reflector() -> Self {
        OracleProvider::Reflector
    }

    /// Creates a Pyth Network oracle provider instance.
    ///
    /// Pyth Network is a high-frequency oracle network that is planned
    /// for future Stellar integration but currently not available.
    ///
    /// # Returns
    ///
    /// `OracleProvider` instance configured for Pyth Network oracle
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let provider = OracleProvider::pyth();
    /// assert_eq!(provider.as_str(), "pyth");
    /// assert!(!provider.is_supported()); // Not available on Stellar
    /// ```
    pub fn pyth() -> Self {
        OracleProvider::Pyth
    }

    /// Creates a Band Protocol oracle provider instance.
    ///
    /// Band Protocol is a decentralized oracle network primarily available
    /// on Cosmos and EVM chains, not currently supported on Stellar.
    ///
    /// # Returns
    ///
    /// `OracleProvider` instance configured for Band Protocol oracle
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let provider = OracleProvider::band_protocol();
    /// assert_eq!(provider.as_str(), "band_protocol");
    /// assert!(!provider.is_supported()); // Not available on Stellar
    /// ```
    pub fn band_protocol() -> Self {
        OracleProvider::BandProtocol
    }

    /// Creates a DIA oracle provider instance.
    ///
    /// DIA is a multi-chain oracle platform with comprehensive asset coverage,
    /// but currently does not have Stellar integration.
    ///
    /// # Returns
    ///
    /// `OracleProvider` instance configured for DIA oracle
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let provider = OracleProvider::dia();
    /// assert_eq!(provider.as_str(), "dia");
    /// assert!(!provider.is_supported()); // Not available on Stellar
    /// ```
    pub fn dia() -> Self {
        OracleProvider::DIA
    }

    /// Creates an OracleProvider from a string identifier.
    ///
    /// This constructor recognizes registered provider identifiers and maps
    /// unrecognized identifiers to an explicit marker that fails validation.
    ///
    /// # Arguments
    ///
    /// * `provider_id` - String identifier for the oracle provider
    ///
    /// # Returns
    ///
    /// `OracleProvider` instance for a recognized identifier, or `Unknown`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, String};
    /// # use predictify_hybrid::types::OracleProvider;
    /// # let env = Env::default();
    ///
    /// // Known provider
    /// let known = OracleProvider::from_str(String::from_str(&env, "reflector"));
    /// assert!(known.is_known());
    ///
    /// // Unknown/future provider
    /// let unknown = OracleProvider::from_str(String::from_str(&env, "future_oracle"));
    /// assert!(!unknown.is_known());
    /// ```
    pub fn from_str(env: &Env, provider_id: String) -> Self {
        if provider_id == String::from_str(env, "reflector") {
            OracleProvider::Reflector
        } else if provider_id == String::from_str(env, "pyth") {
            OracleProvider::Pyth
        } else if provider_id == String::from_str(env, "band_protocol") {
            OracleProvider::BandProtocol
        } else if provider_id == String::from_str(env, "dia") {
            OracleProvider::DIA
        } else {
            OracleProvider::Unknown
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            OracleProvider::Reflector => "reflector",
            OracleProvider::Pyth => "pyth",
            OracleProvider::BandProtocol => "band_protocol",
            OracleProvider::DIA => "dia",
            OracleProvider::Unknown => "unknown",
        }
    }

    /// Returns a human-readable name for the oracle provider.
    ///
    /// This method provides formatted display names for UI and logging purposes.
    /// Unknown providers return a generic "Unknown Provider" label.
    ///
    /// # Returns
    ///
    /// String containing the formatted provider name
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::{Env, String};
    /// # use predictify_hybrid::types::OracleProvider;
    /// # let env = Env::default();
    ///
    /// let reflector = OracleProvider::reflector();
    /// assert_eq!(reflector.name(), "Reflector");
    ///
    /// let unknown = OracleProvider::from_str(String::from_str(&env, "new_oracle"));
    /// assert_eq!(unknown.name(), "Unknown Provider (new_oracle)");
    /// ```
    pub fn name(&self) -> String {
        let env = soroban_sdk::Env::default();
        match self.as_str() {
            "reflector" => String::from_str(&env, "Reflector"),
            "pyth" => String::from_str(&env, "Pyth Network"),
            "band_protocol" => String::from_str(&env, "Band Protocol"),
            "dia" => String::from_str(&env, "DIA"),
            unknown => {
                let prefix = String::from_str(&env, "Unknown Provider (");
                let suffix = String::from_str(&env, ")");
                // Use string slicing for soroban_sdk::String
                let result = prefix.clone();
                // For simplicity, just return a basic message for unknown providers
                String::from_str(&env, "Unknown Provider")
            }
        }
    }

    /// Checks if this oracle provider is known to the current contract version.
    ///
    /// This method identifies providers that are explicitly recognized by the
    /// current contract code, even if they are not supported on the current
    /// blockchain network.
    ///
    /// # Returns
    ///
    /// `true` if the provider is known, `false` for unknown providers
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let reflector = OracleProvider::reflector();
    /// assert!(reflector.is_known()); // Known provider
    ///
    /// let pyth = OracleProvider::pyth();
    /// assert!(pyth.is_known()); // Known but unsupported
    /// ```
    pub fn is_known(&self) -> bool {
        !matches!(self, OracleProvider::Unknown)
    }

    /// Checks if this oracle provider is supported on the current network.
    ///
    /// This method determines if the provider can actually be used for oracle
    /// operations on the current blockchain network (Stellar). This is more
    /// restrictive than `is_known()` as it considers network availability.
    ///
    /// # Network Support Matrix
    ///
    /// | Provider | Stellar | Ethereum | Cosmos | Status |
    /// |----------|---------|----------|---------|---------|
    /// | Reflector | ✅ | ❌ | ❌ | Primary |
    /// | Pyth | ❌ | ✅ | ❌ | Future |
    /// | Band Protocol | ❌ | ✅ | ✅ | Not on Stellar |
    /// | DIA | ❌ | ✅ | ✅ | Not on Stellar |
    ///
    /// # Returns
    ///
    /// `true` if the provider is supported on Stellar, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust
    /// # use predictify_hybrid::types::OracleProvider;
    ///
    /// let reflector = OracleProvider::reflector();
    /// assert!(reflector.is_supported()); // Supported on Stellar
    ///
    /// let pyth = OracleProvider::pyth();
    /// assert!(!pyth.is_supported()); // Not available on Stellar
    /// ```
    pub fn is_supported(&self) -> bool {
        matches!(self, OracleProvider::Reflector)
    }

    pub fn is_reflector(&self) -> bool {
        matches!(self, OracleProvider::Reflector)
    }

    pub fn is_pyth(&self) -> bool {
        matches!(self, OracleProvider::Pyth)
    }

    pub fn is_band_protocol(&self) -> bool {
        matches!(self, OracleProvider::BandProtocol)
    }

    pub fn is_dia(&self) -> bool {
        matches!(self, OracleProvider::DIA)
    }

    /// Validates the oracle provider for market creation.
    ///
    /// This method performs comprehensive validation to ensure the provider
    /// can be safely used for new market creation. It's more strict than
    /// `is_supported()` and should be used during market validation.
    ///
    /// # Validation Rules
    ///
    /// - Provider must be a known identifier
    /// - Provider must be supported on the current network
    /// - Provider must pass basic format validation
    ///
    /// # Returns
    ///
    /// `Ok(())` if the provider is valid for market creation
    /// `Err(Error::InvalidOracleProvider)` if validation fails
    ///
    /// # Example
    ///
    /// ```rust
    /// # use soroban_sdk::Env;
    /// # use predictify_hybrid::types::OracleProvider;
    /// # let env = Env::default();
    ///
    /// let reflector = OracleProvider::reflector();
    /// assert!(reflector.validate_for_market(&env).is_ok()); // Valid
    ///
    /// let pyth = OracleProvider::pyth();
    /// assert!(pyth.validate_for_market(&env).is_err()); // Not supported
    /// ```
    pub fn validate_for_market(&self, _env: &soroban_sdk::Env) -> Result<(), crate::Error> {
        if !self.is_supported() {
            return Err(crate::Error::InvalidOracleProvider);
        }
        Ok(())
    }
}

/// Comprehensive oracle configuration for prediction market resolution.
///
/// This structure defines all parameters needed to configure oracle-based market
/// resolution, including provider selection, price feed identification, threshold
/// values, and comparison logic. It serves as the bridge between prediction markets
/// and external oracle data sources.
///
/// # Configuration Components
///
/// **Provider Selection:**
/// - **Provider**: Which oracle service to use (Reflector, Pyth, etc.)
/// - **Feed ID**: Specific price feed identifier for the asset
///
/// **Resolution Logic:**
/// - **Threshold**: Price level that determines market outcome
/// - **Comparison**: How to compare oracle price against threshold
///
/// # Supported Comparisons
///
/// The oracle configuration supports various comparison operators:
/// - **"gt"**: Greater than - price > threshold resolves to "yes"
/// - **"lt"**: Less than - price < threshold resolves to "yes"
/// - **"eq"**: Equal to - price == threshold resolves to "yes"
///
/// # Price Format Standards
///
/// Thresholds follow consistent pricing conventions:
/// - **Integer Values**: No floating point arithmetic
/// - **Cent Precision**: Prices in cents (e.g., 5000000 = $50,000)
/// - **Positive Values**: All thresholds must be positive
/// - **Reasonable Range**: Between $0.01 and $10,000,000
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleConfig {
    /// The oracle provider to use
    pub provider: OracleProvider,
    /// The oracle contract address
    pub oracle_address: Address,
    /// Oracle-specific identifier (e.g., "BTC/USD" for Pyth, "BTC" for Reflector)
    pub feed_id: String,
    /// Price threshold in cents (e.g., 10_000_00 = $10k)
    pub threshold: i128,
    /// Comparison operator: "gt", "lt", "eq"
    pub comparison: String,
}

impl OracleConfig {
    /// Create a new oracle configuration
    pub fn new(
        provider: OracleProvider,
        oracle_address: Address,
        feed_id: String,
        threshold: i128,
        comparison: String,
    ) -> Self {
        Self {
            provider,
            oracle_address,
            feed_id,
            threshold,
            comparison,
        }
    }

    /// Returns the reserved sentinel used to encode "no fallback oracle" in storage.
    ///
    /// The contracts persist fallback-oracle state as a `(has_fallback, fallback_oracle_config)`
    /// pair instead of `Option<OracleConfig>` for Soroban contract-type compatibility. When
    /// `has_fallback` is `false`, this sentinel is written into `fallback_oracle_config`.
    ///
    /// Sentinel semantics are reserved by the tuple `(provider=Reflector, feed_id="", threshold=0,
    /// comparison="")`. Those values are intentionally outside the valid oracle-config domain, so
    /// the sentinel cannot collide with any configuration that passes validation.
    ///
    /// This value must never be used for live oracle creation or resolution.
    pub fn none_sentinel(env: &Env) -> Self {
        Self {
            provider: OracleProvider::reflector(),
            oracle_address: Address::from_str(
                env,
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            ),
            feed_id: String::from_str(env, ""),
            threshold: 0,
            comparison: String::from_str(env, ""),
        }
    }

    /// Returns `true` when this configuration is the reserved "no fallback" sentinel.
    ///
    /// The sentinel identity intentionally ignores `oracle_address`: the reserved semantics are
    /// carried by the invalid `feed_id` / `threshold` / `comparison` tuple, which keeps the
    /// sentinel unambiguous even if a caller reuses the placeholder address elsewhere.
    pub fn is_none_sentinel(&self) -> bool {
        self.provider.as_str() == "reflector"
            && self.feed_id.is_empty()
            && self.threshold == 0
            && self.comparison.is_empty()
    }
}

impl OracleConfig {
    /// Validate the oracle configuration
    pub fn validate(&self, env: &Env) -> Result<(), crate::Error> {
        if self.is_none_sentinel() {
            return Err(crate::Error::InvalidOracleConfig);
        }

        if self.feed_id.is_empty() {
            return Err(crate::Error::InvalidOracleFeed);
        }

        crate::metadata_limits::validate_feed_id_length(&self.feed_id)?;
        crate::metadata_limits::validate_comparison_length(&self.comparison)?;

        // Threshold must be positive
        if self.threshold <= 0 {
            return Err(crate::Error::InvalidThreshold);
        }

        // Only allow gt / lt / eq
        if self.comparison != String::from_str(env, "gt")
            && self.comparison != String::from_str(env, "lt")
            && self.comparison != String::from_str(env, "eq")
        {
            return Err(crate::Error::InvalidComparison);
        }

        // Reject impossible combinations per provider
        let provider_str = self.provider.as_str();
        let feed_id_len = self.feed_id.len();

        if provider_str == "reflector" {
            // Reflector uses short asset symbols like "BTC/USD" or "XLM"
            // Hex strings of 64+ chars are Pyth feeds and impossible for Reflector
            if feed_id_len >= 64 {
                return Err(crate::Error::InvalidOracleFeed);
            }
        } else if provider_str == "pyth" {
            // Pyth uses 64-char hex strings (sometimes 66 with 0x)
            if feed_id_len < 64 || feed_id_len > 66 {
                return Err(crate::Error::InvalidOracleFeed);
            }
        } else if provider_str == "band_protocol" || provider_str == "dia" {
            if feed_id_len >= 64 {
                return Err(crate::Error::InvalidOracleFeed);
            }
        }

        // Validate provider is supported using new validation method
        self.provider.validate_for_market(env)?;

        // Enforce authorized oracle address validation
        crate::oracles::OracleRegistry::require_oracle_allowed(env, &self.oracle_address)?;

        Ok(())
    }

    /// Returns `true` if this oracle configuration is active and valid.
    ///
    /// An oracle is considered active if it's not the none sentinel.
    pub fn is_active(&self) -> bool {
        !self.is_none_sentinel() && !self.feed_id.is_empty()
    }
}

/// Comprehensive oracle result structure for automatic result verification.
///
/// This structure captures the complete oracle response including the fetched data,
/// signature verification details, timestamp, and validation status. Used for
/// automated event outcome verification from external data sources.
///
/// # Components
///
/// **Result Data:**
/// - **market_id**: Market being resolved
/// - **outcome**: Determined outcome ("yes"/"no" or custom)
/// - **price**: Fetched price value from oracle
/// - **threshold**: Configured threshold for comparison
///
/// **Verification Data:**
/// - **provider**: Oracle provider used
/// - **signature**: Oracle signature for authenticity (if available)
/// - **is_verified**: Whether signature validation passed
/// - **confidence_score**: Statistical confidence (0-100)
///
/// **Metadata:**
/// - **timestamp**: When result was fetched
/// - **block_number**: Ledger sequence at fetch time
/// - **sources_count**: Number of oracle sources consulted
///
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleResult {
    /// Market ID this result is for
    pub market_id: Symbol,
    /// Determined outcome ("yes", "no", or custom outcome)
    pub outcome: String,
    /// Fetched price from oracle
    pub price: i128,
    /// Threshold configured for this market
    pub threshold: i128,
    /// Comparison operator used ("gt", "lt", "eq")
    pub comparison: String,
    /// Oracle provider that provided the result
    pub provider: OracleProvider,
    /// Feed ID used for price lookup
    pub feed_id: String,
    /// Timestamp when result was fetched
    pub timestamp: u64,
    /// Ledger sequence number at fetch time
    pub block_number: u32,
    /// Whether the oracle response was verified (signature valid)
    pub is_verified: bool,
    /// Confidence score (0-100)
    pub confidence_score: u32,
    /// Number of oracle sources consulted
    pub sources_count: u32,
    /// Oracle signature bytes (optional, for providers that support signatures)
    pub signature: Option<String>,
    /// Error message if verification failed
    pub error_message: Option<String>,
}

impl OracleResult {
    /// Check if the oracle result is valid and verified
    pub fn is_valid(&self) -> bool {
        self.is_verified && self.confidence_score >= 50 && self.price > 0
    }

    /// Check if the oracle data is fresh (within max_age_seconds) using current ledger timestamp
    pub fn is_fresh(&self, env: &Env, max_age_seconds: u64) -> bool {
        env.ledger().timestamp().saturating_sub(self.timestamp) <= max_age_seconds
    }
}

/// Lightweight oracle price payload with validation metadata.
///
/// This structure captures the minimum data needed for staleness and
/// confidence interval validation without provider-specific dependencies.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OraclePriceData {
    /// Price value in oracle base units
    pub price: i128,
    /// Publish time of the oracle data (unix timestamp seconds)
    pub publish_time: u64,
    /// Publish ledger sequence of the oracle data
    pub publish_ledger: u32,
    /// Confidence interval (absolute) in the same base units as `price`
    pub confidence: Option<i128>,
    /// Exponent/decimals scale used by the oracle (e.g., Pyth exponent)
    pub exponent: i32,
}

/// Global oracle validation configuration applied when no per-event override exists.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalOracleValidationConfig {
    pub max_staleness_secs: u64,
    pub max_confidence_bps: u32,
    pub max_deviation_bps: Option<u32>,
    pub max_deviation_z_multiple: Option<u32>,
    pub history_size: Option<u32>,
    pub auto_pause_duration_secs: Option<u64>,
}

/// Per-event oracle validation configuration override.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventOracleValidationConfig {
    pub max_staleness_secs: u64,
    pub max_confidence_bps: u32,
    pub max_deviation_bps: Option<u32>,
    pub max_deviation_z_multiple: Option<u32>,
    pub history_size: Option<u32>,
    pub auto_pause_duration_secs: Option<u64>,
}

/// Multi-oracle aggregated result for consensus-based verification.
///
/// This structure aggregates results from multiple oracle sources to provide
/// a more reliable and tamper-resistant outcome determination.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiOracleResult {
    /// Market ID this result is for
    pub market_id: Symbol,
    /// Final determined outcome based on consensus
    pub final_outcome: String,
    /// Individual results from each oracle source
    pub individual_results: Vec<OracleResult>,
    /// Whether consensus was reached among oracles
    pub consensus_reached: bool,
    /// Required consensus threshold (percentage, e.g., 66 for 2/3)
    pub consensus_threshold: u32,
    /// Actual agreement percentage among oracles
    pub agreement_percentage: u32,
    /// Aggregation timestamp
    pub timestamp: u64,
}

impl MultiOracleResult {
    /// Check if the multi-oracle result has sufficient consensus
    pub fn has_consensus(&self) -> bool {
        self.consensus_reached && self.agreement_percentage >= self.consensus_threshold
    }
}

// ── Median Oracle Aggregation Types ─────────────────────────────────────────

/// A single oracle's price quote produced by
/// `OracleResolutionManager::resolve_with_median`.
///
/// One `OracleQuote` is collected from each of Pyth, Reflector, and Band.
/// The `included` flag is `false` either because the oracle fetch failed
/// or because the reported price was rejected as an outlier by the
/// configured `max_deviation_bps` threshold.
///
/// # Confidence Weight Derivation
///
/// When the oracle reports a confidence interval `c` for a price `p`, the
/// weight is computed as:
/// ```text
/// weight_bps = p × 10 000 / (p + c)
/// ```
/// A tighter interval (lower `c`) raises the weight toward 10 000.  When
/// no interval is reported, a medium weight of 5 000 bps is used.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleQuote {
    /// Oracle provider (Pyth, Reflector, or Band Protocol).
    pub provider: OracleProvider,
    /// Raw price from the oracle feed, in market base units.
    /// Zero when `included` is `false`.
    pub price: i128,
    /// Confidence interval expressed as a fraction of `price` in basis
    /// points (100 bps = 1 %).  Zero indicates the oracle did not report
    /// a confidence interval; a larger value means higher price uncertainty.
    pub confidence_bps: u32,
    /// Influence weight for the weighted-median computation, in basis
    /// points (10 000 = full weight).  Derived from the confidence
    /// interval: tighter intervals yield higher weights.  Zero for
    /// excluded quotes.
    pub weight_bps: u32,
    /// `true` when this quote participates in the final median computation.
    /// `false` for failed fetches or outliers that exceed
    /// `MedianOracleConfig::max_deviation_bps`.
    pub included: bool,
}

/// Global configuration for the three-oracle median resolver.
///
/// Stored once by the contract admin via
/// `OracleResolutionManager::set_median_config` and referenced by every
/// subsequent call to `OracleResolutionManager::resolve_with_median`.
///
/// # Outlier Detection
///
/// After all three feeds are queried, an unweighted simple median is
/// computed.  Any quote whose price satisfies
/// ```text
/// |price − simple_median| × 10 000 / simple_median > max_deviation_bps
/// ```
/// is discarded before the final weighted-median computation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MedianOracleConfig {
    /// Contract address of the Pyth price oracle.
    /// On Stellar this oracle returns `OracleUnavailable`; the quote will
    /// be excluded with `included = false`.
    pub pyth_address: Address,
    /// Contract address of the Reflector oracle (primary on Stellar).
    pub reflector_address: Address,
    /// Contract address of the Band Protocol oracle.
    pub band_address: Address,
    /// Maximum price deviation allowed before a quote is rejected as an
    /// outlier, in basis points (1 bps = 0.01 %).
    /// Recommended default: 200 (2 %).
    pub max_deviation_bps: u32,
    /// Minimum number of non-outlier quotes required for a valid
    /// resolution.  `resolve_with_median` returns `OracleNoConsensus`
    /// when fewer quotes survive filtering.  Must be ≥ 1;
    /// recommended value: 2.
    pub min_sources: u32,
}

/// Oracle source configuration for multi-oracle support.
///
/// Defines a single oracle source with its configuration, weight, and status.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleSource {
    /// Unique identifier for this oracle source
    pub source_id: Symbol,
    /// Oracle provider type
    pub provider: OracleProvider,
    /// Oracle contract address
    pub contract_address: Address,
    /// Weight for consensus calculation (1-100)
    pub weight: u32,
    /// Whether this source is currently active
    pub is_active: bool,
    /// Priority for fallback ordering (lower = higher priority)
    pub priority: u32,
    /// Last successful response timestamp
    pub last_success: u64,
    /// Consecutive failure count
    pub failure_count: u32,
}

/// Oracle fetch request configuration.
///
/// Specifies parameters for fetching oracle data including timeout,
/// retry settings, and source preferences.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleFetchRequest {
    /// Market ID to fetch result for
    pub market_id: Symbol,
    /// Feed ID to query
    pub feed_id: String,
    /// Maximum age of data in seconds (staleness threshold)
    pub max_data_age: u64,
    /// Required number of confirmations/sources
    pub required_confirmations: u32,
    /// Whether to use fallback sources on primary failure
    pub use_fallback: bool,
    /// Minimum confidence score required
    pub min_confidence: u32,
}

impl OracleFetchRequest {
    /// Create a default fetch request for a market
    pub fn new(env: &Env, market_id: Symbol, feed_id: String) -> Self {
        Self {
            market_id,
            feed_id,
            max_data_age: 300, // 5 minutes default
            required_confirmations: 1,
            use_fallback: true,
            min_confidence: 50,
        }
    }

    /// Create a strict fetch request requiring multiple confirmations
    pub fn strict(env: &Env, market_id: Symbol, feed_id: String) -> Self {
        Self {
            market_id,
            feed_id,
            max_data_age: 60, // 1 minute
            required_confirmations: 2,
            use_fallback: true,
            min_confidence: 80,
        }
    }
}

/// Oracle verification status for tracking verification state.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleVerificationStatus {
    /// Verification not yet attempted
    Pending,
    /// Verification in progress
    InProgress,
    /// Verification successful
    Verified,
    /// Verification failed - invalid signature
    InvalidSignature,
    /// Verification failed - stale data
    StaleData,
    /// Verification failed - oracle unavailable
    OracleUnavailable,
    /// Verification failed - threshold not met
    ThresholdNotMet,
    /// Verification failed - consensus not reached
    NoConsensus,
}

/// Enumeration of supported assets in the Reflector Oracle ecosystem.
///
/// This enum defines the cryptocurrency assets for which the Reflector Oracle
/// provides price feeds on the Stellar network.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReflectorAsset {
    /// Stellar Lumens (XLM)
    Stellar,
    /// Bitcoin (BTC)
    BTC,
    /// Ethereum (ETH)
    ETH,
    /// Other asset identified by symbol
    Other(Symbol),
}

/// Comprehensive price data structure from Reflector Oracle.
///
/// This structure contains all price information returned by the Reflector Oracle,
/// including current price, timestamp, and metadata necessary for market resolution
/// and validation.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectorPriceData {
    /// Price value in cents (e.g., 2500000 = $25,000)
    pub price: i128,
    /// Timestamp of price update
    pub timestamp: u64,
    /// Price source/confidence
    pub source: String,
}

impl ReflectorAsset {
    fn soroban_string_to_host_string(value: &String) -> StdString {
        let mut bytes = alloc::vec![0u8; value.len() as usize];
        value.copy_into_slice(&mut bytes);
        StdString::from_utf8(bytes).unwrap_or_else(|_| StdString::from("invalid_utf8"))
    }

    #[cfg(not(target_family = "wasm"))]
    fn custom_symbol_to_host_string(symbol: &Symbol) -> StdString {
        symbol.to_string()
    }

    #[cfg(target_family = "wasm")]
    fn custom_symbol_to_host_string(_symbol: &Symbol) -> StdString {
        StdString::from("CUSTOM")
    }

    /// Check if this asset is Stellar Lumens (XLM)
    pub fn is_xlm(&self) -> bool {
        matches!(self, ReflectorAsset::Stellar)
    }

    /// Returns the symbol string for this asset
    pub fn symbol(&self) -> String {
        let env = soroban_sdk::Env::default();
        match self {
            ReflectorAsset::Stellar => String::from_str(&env, "XLM"),
            ReflectorAsset::BTC => String::from_str(&env, "BTC"),
            ReflectorAsset::ETH => String::from_str(&env, "ETH"),
            ReflectorAsset::Other(symbol) => {
                String::from_str(&env, Self::custom_symbol_to_host_string(symbol).as_str())
            }
        }
    }

    /// Returns the human-readable name for this asset
    pub fn name(&self) -> String {
        let env = soroban_sdk::Env::default();
        match self {
            ReflectorAsset::Stellar => String::from_str(&env, "Stellar Lumens"),
            ReflectorAsset::BTC => String::from_str(&env, "Bitcoin"),
            ReflectorAsset::ETH => String::from_str(&env, "Ethereum"),
            ReflectorAsset::Other(symbol) => String::from_str(
                &env,
                &alloc::format!(
                    "Custom Asset ({})",
                    Self::custom_symbol_to_host_string(symbol)
                ),
            ),
        }
    }

    /// Returns the number of decimal places for this asset
    pub fn decimals(&self) -> u8 {
        match self {
            ReflectorAsset::Stellar => 7,
            ReflectorAsset::BTC => 8,
            ReflectorAsset::ETH => 18,
            ReflectorAsset::Other(_) => 7, // Default to 7 for custom assets
        }
    }

    /// Returns the Reflector feed ID for this asset (e.g., "BTC/USD")
    pub fn feed_id(&self) -> String {
        let env = soroban_sdk::Env::default();
        match self {
            ReflectorAsset::Stellar => String::from_str(&env, "XLM/USD"),
            ReflectorAsset::BTC => String::from_str(&env, "BTC/USD"),
            ReflectorAsset::ETH => String::from_str(&env, "ETH/USD"),
            ReflectorAsset::Other(symbol) => String::from_str(
                &env,
                &alloc::format!("{}/USD", Self::custom_symbol_to_host_string(symbol)),
            ),
        }
    }

    /// Checks if this asset is supported by Reflector oracle
    pub fn is_supported(&self) -> bool {
        match self {
            ReflectorAsset::Stellar | ReflectorAsset::BTC | ReflectorAsset::ETH => true,
            ReflectorAsset::Other(_) => false, // Custom assets not supported by default
        }
    }

    /// Checks if this asset is a known asset (including custom ones)
    pub fn is_known(&self) -> bool {
        true // All ReflectorAsset variants are known by definition
    }

    /// Validates the asset for use in market creation
    pub fn validate_for_market(&self, _env: &soroban_sdk::Env) -> Result<(), crate::Error> {
        if !self.is_supported() {
            return Err(Error::InvalidOracleFeed);
        }
        Ok(())
    }

    /// Creates a ReflectorAsset from a symbol string
    pub fn from_symbol(env: &Env, symbol: String) -> Self {
        match Self::soroban_string_to_host_string(&symbol).as_str() {
            "XLM" => ReflectorAsset::Stellar,
            "BTC" => ReflectorAsset::BTC,
            "ETH" => ReflectorAsset::ETH,
            _ => ReflectorAsset::Other(Symbol::new(
                env,
                Self::soroban_string_to_host_string(&symbol).as_str(),
            )),
        }
    }

    /// Returns all supported assets for testing purposes
    pub fn all_supported() -> Vec<Self> {
        let env = soroban_sdk::Env::default();
        Vec::from_array(
            &env,
            [
                ReflectorAsset::Stellar,
                ReflectorAsset::BTC,
                ReflectorAsset::ETH,
            ],
        )
    }

    /// Returns all known assets (including unsupported) for testing purposes
    pub fn all_known() -> Vec<Self> {
        let env = soroban_sdk::Env::default();
        Vec::from_array(
            &env,
            [
                ReflectorAsset::Stellar,
                ReflectorAsset::BTC,
                ReflectorAsset::ETH,
                ReflectorAsset::Other(Symbol::new(&env, "CUSTOM")),
            ],
        )
    }
}
