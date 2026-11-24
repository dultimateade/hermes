/// Per-market configuration for admin timelocks.
///
/// All durations are measured in **ledger-time seconds**
/// (`env.ledger().timestamp()`).  Under normal Stellar network conditions
/// ledger time tracks wall-clock time to within a few minutes.  For windows
/// of one hour or more the effective real-world duration is
/// indistinguishable from the configured value.  For windows below
/// ~5 minutes, consider using ledger sequence numbers instead (see
/// `docs/TIMESTAMP_ASSUMPTIONS.md`).
use soroban_sdk::contracttype;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketTimelockConfig {
    /// Minimum seconds of ledger time between admin mutations on this market.
    ///
    /// Zero disables the per-market cooldown.  Meaningful values should be
    /// at least 300 seconds (5 minutes) to stay above the typical ledger
    /// timestamp drift.
    pub admin_cooldown_seconds: u64,
}

impl Default for MarketTimelockConfig {
    fn default() -> Self {
        Self {
            admin_cooldown_seconds: 0,
        }
    }
}
