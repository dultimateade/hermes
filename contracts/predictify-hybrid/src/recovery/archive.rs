//! Historical recovery archival constants and event types.

use soroban_sdk::{contracttype, symbol_short, Symbol};

pub use super::RecoveryHistoryEntry;

pub const RECOVERY_ARCHIVE_TTL_LEDGERS: u32 = 730 * 17_280;
pub const RECOVERY_ARCHIVE_MAX_ENTRIES: u32 = 10;
pub const RECOVERY_ARCHIVE_TOPIC: Symbol = symbol_short!("rcv_arv");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryArchiveEvent {
    EntryArchived,
    EntryPruned,
}
