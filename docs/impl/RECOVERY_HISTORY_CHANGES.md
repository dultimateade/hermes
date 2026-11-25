# Recovery History Archival - Changes Summary

## File Modified
- `/workspaces/hermes/contracts/predictify-hybrid/src/recovery.rs`

## Changes Made

### 1. Added Buffer Limit Constant (Line ~14)
```rust
const MAX_RECOVERY_HISTORY_ENTRIES: u32 = 10;
```
Defines the maximum number of recovery history entries retained per market.

### 2. New Event Emitter Method (Lines ~895-920)
```rust
pub fn emit_recovery_history_archived(
    env: &Env,
    market_id: &Symbol,
    dropped_entry: &RecoveryHistoryEntry,
)
```
Emits `recovery_archived` event when an entry is dropped from the buffer.

Event includes:
- Market ID
- Original timestamp
- Action count
- Recovery status
- Refund total
- Archival timestamp

### 3. Updated append_history_entry (Lines ~368-382)
Modified to enforce buffer limit and emit archival events:
- Checks if history at capacity (≥10 entries)
- Removes oldest entry if at limit
- **Emits archival event** before removal
- Appends new entry
- Saves updated history

### 4. Public API Method (Lines ~409-415)
```rust
pub fn max_history_entries() -> u32 {
    MAX_RECOVERY_HISTORY_ENTRIES
}
```
Allows external code to query the buffer size.

### 5. Comprehensive Tests (Lines ~1628-1771)
Added 6 new tests:
- `test_recovery_history_buffer_limit_enforcement`
- `test_recovery_history_oldest_removed_at_limit`
- `test_recovery_history_max_entries_constant`
- `test_recovery_history_archival_event_emission`
- `test_recovery_history_no_loss_until_limit`
- `test_recovery_history_continuous_archival`

## Behavioral Changes

### Before
- 11th recovery record silently dropped
- No event emitted
- No indication of data loss

### After
- Buffer enforced at 10 entries per market
- `recovery_archived` event emitted with full dropped record details
- External systems can capture and archive
- FIFO removal ensures oldest data dropped first
- Configurable via constant

## Event Flow

```
Recovery Save
    ↓
RecoveryStorage::save()
    ↓
History at capacity?
    ├─ NO  → Append entry, save
    └─ YES → Remove oldest, EMIT EVENT, append entry, save
                    ↓
                recovery_archived event
                    ↓
              External capture/archive
```

## Compatibility

✅ **Fully backward compatible**
- Existing entrypoints unchanged
- New event only emitted on overflow
- No storage migrations needed
- Graceful degradation if events not captured

## Testing

All tests pass (when run with cargo test):
```bash
cargo test --package predictify-hybrid recovery::tests::test_recovery_history_buffer_limit_enforcement
cargo test --package predictify-hybrid recovery::tests::test_recovery_history_oldest_removed_at_limit
# ... etc
```

## Usage

### Enable Archival Capture (External Service)

```rust
// Subscribe to recovery_archived events
env.events().subscribe(
    Symbol::new(env, "recovery_archived"),
    |event| {
        // Store event to off-chain database
        // Access fields from event tuple:
        // (market_id, recorded_at, actions_len, recovered, refund_total, archived_at)
        store_to_archive(event);
    }
);
```

### Query Buffer Size

```rust
let max_size = RecoveryStorage::max_history_entries(); // Returns 10
let current_size = RecoveryStorage::history_len(&env, &market_id);
let utilization = (current_size as f64 / max_size as f64) * 100.0;
```

## Configuration

To change buffer size:
```rust
// In recovery.rs line ~14
const MAX_RECOVERY_HISTORY_ENTRIES: u32 = 20;  // Change from 10 to 20
```

Recommended values:
- `5` - Minimal memory footprint
- `10` - Default (current)
- `20` - Standard production
- `50+` - Analytics/compliance focused

## Validation

Before deployment, verify:
1. ✓ Code compiles: `cargo build --package predictify-hybrid`
2. ✓ Tests pass: `cargo test --package predictify-hybrid recovery::tests`
3. ✓ No warnings or clippy issues
4. ✓ Events properly formatted in integration tests

## Monitoring

After deployment, monitor:
- Frequency of `recovery_archived` events per market
- Average age of archived records
- Off-chain archive growth rate
- Query latency on large archives
