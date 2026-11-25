# Platform-Wide Emergency Halt Mechanism

## Overview

The Hermes platform currently has per-contract pause mechanisms but lacks a unified platform-wide emergency halt system. This design document specifies a new **Emergency Halt Control (EHC)** contract that enables atomic freezing of all platform activity in a single transaction.

## Current State

### Existing Pause Mechanisms
- **Reporting Contract**: `pause_reporting()` / `unpause_reporting()`
- **Fees Contract**: `pause_fees()` / `unpause_fees()`
- **Markets Contract**: `pause_market()` / `resume_market()` (per-market)
- **Validators Contract**: `pause_validators()` / `unpause_validators()`

### Limitations
1. **Multiple transactions required**: Admin must call pause on each contract separately
2. **Race conditions**: Pause operations not atomic; malicious activity may occur between calls
3. **Operational burden**: Complex incident response workflow
4. **No unified state**: Difficult to verify all subsystems are actually halted
5. **Recovery uncertainty**: No clear audit trail or recovery orchestration

## Proposed Solution: Emergency Halt Control (EHC) Contract

### Architecture

```
┌─────────────────────────────────────────┐
│  Emergency Halt Control (EHC) Contract  │
│          (New Contract)                 │
└──────────┬──────────────────────────────┘
           │ Atomic multi-contract operations
           ├─────► Markets Contract (pause all markets)
           ├─────► Fees Contract (pause fees)
           ├─────► Reporting Contract (pause reporting)
           ├─────► Validators Contract (pause validators)
           └─────► Disputes Contract (pause disputes)
```

### Key Features

#### 1. Atomic Platform Halt
- Single contract call that atomically pauses all subsystems
- Transaction either succeeds completely or fails with no partial state
- Implemented via cross-contract invocations with authorization delegation

#### 2. Halt States
```rust
pub enum HaltState {
    Operational,      // Normal operation
    HaltInitiated,    // Halt in progress (timeout window)
    HaltActive,       // All systems paused
    RecoveryInitiated, // Recovery process started
}
```

#### 3. Multi-Level Authorization
- **Emergency Halt**: Requires admin-level auth (immediate effect)
- **Coordinated Halt**: Requires multi-sig approval (for planned maintenance)
- **Recovery**: Requires explicit recovery authorization with audit log

#### 4. Timelock & Recovery Window
- Halt remains active indefinitely once initiated
- Recovery requires explicit action plus optional timelock delay
- Audit log tracks all state transitions with timestamps and authorizers

#### 5. Health Checks
- Pre-halt verification: Query pause status of all subsystems
- Post-halt verification: Confirm all subsystems are halted
- Periodic health probes: Ongoing monitoring of halt effectiveness

### State Management

```rust
#[contracttype]
pub struct HaltConfig {
    pub is_halted: bool,
    pub halt_reason: String,      // Description of why halted
    pub halt_initiated_by: Address, // Admin who initiated
    pub halt_timestamp: u64,        // Ledger time of halt
    pub halt_state: HaltState,
}

#[contracttype]
pub struct RecoveryConfig {
    pub recovery_authorized_by: Address,
    pub recovery_authorization_time: u64,
    pub timelock_duration_seconds: u64, // Optional delay before recovery
    pub is_recovery_locked: bool,
}

#[contracttype]
pub struct HaltAuditEntry {
    pub timestamp: u64,
    pub action: HaltAction,           // Halt, Recovery, HealthCheck, etc.
    pub initiated_by: Address,
    pub reason: String,
    pub subsystems_affected: Vec<String>, // ["Markets", "Fees", "Reporting", ...]
}

pub enum HaltAction {
    PlatformHaltInitiated,
    PlatformRecoveryInitiated,
    HealthCheckPerformed,
    SubsystemPauseConfirmed,
    SubsystemResumeConfirmed,
    RecoveryTimeoutReached,
}
```

### Entrypoints

#### State-Changing Operations (Require Auth)

```rust
/// Initiates platform-wide emergency halt
/// - Pauses all subsystems atomically
/// - Requires admin authentication
/// - Returns immediately with success/failure
/// - No timelock on halt itself
pub fn emergency_halt(
    env: Env,
    admin: Address,
    reason: String,
) -> Result<HaltConfig, HaltError>

/// Initiates recovery from halt
/// - Requires admin authentication
/// - Sets recovery_locked = true if timelock_duration > 0
/// - Emits recovery initiated event
/// - Admin can trigger resume after timelock expires
pub fn initiate_recovery(
    env: Env,
    admin: Address,
    timelock_duration_seconds: u64,
) -> Result<RecoveryConfig, HaltError>

/// Completes recovery and resumes all subsystems
/// - Checks if timelock has expired (if set)
/// - Resumes all paused subsystems
/// - Clears halt flag and recovery state
/// - Requires admin authentication
pub fn resume_all_subsystems(
    env: Env,
    admin: Address,
) -> Result<(), HaltError>

/// Force abort recovery (keep halt active)
/// - Reverts recovery_locked state
/// - Halt remains active
/// - Useful for abort scenario (e.g., detected ongoing attack)
pub fn abort_recovery(
    env: Env,
    admin: Address,
) -> Result<(), HaltError>
```

#### Read-Only Operations (No Auth Required)

```rust
/// Get current halt status
pub fn get_halt_config(env: Env) -> HaltConfig

/// Get recovery configuration
pub fn get_recovery_config(env: Env) -> RecoveryConfig

/// Check if platform is halted
pub fn is_halted(env: Env) -> bool

/// Get health status of all subsystems
pub fn health_check(env: Env) -> HealthCheckResult {
    pub markets_paused: bool,
    pub fees_paused: bool,
    pub reporting_paused: bool,
    pub validators_paused: bool,
    pub disputes_paused: bool,
    pub all_halted: bool,
    pub check_timestamp: u64,
}

/// Get audit log (paginated)
pub fn get_audit_log(
    env: Env,
    offset: u32,
    limit: u32,
) -> Vec<HaltAuditEntry>

/// Get halt history
pub fn get_halt_history(env: Env) -> Vec<HaltHistoryEntry> {
    pub halt_id: u64,
    pub initiated_timestamp: u64,
    pub recovery_timestamp: Option<u64>,
    pub duration_seconds: Option<u64>,
    pub reason: String,
    pub outcome: HaltOutcome,
}
```

### Integration Points

#### Cross-Contract Calls

Each subsystem contract must support being called by EHC:

```rust
// In Markets Contract
#[contractimpl]
impl MarketsContract {
    pub fn pause_all_markets(env: Env, initiator: Address) -> Result<(), Error> {
        // Only allow EHC contract as initiator
        verify_ehc_caller(&env, &initiator)?;
        
        // Pause flag that blocks all operations
        env.storage().persistent().set(&DataKey::GlobalPause, &true);
        events::emit_global_pause(&env, "emergency_halt");
        Ok(())
    }

    pub fn resume_all_markets(env: Env, initiator: Address) -> Result<(), Error> {
        // Only allow EHC contract as initiator
        verify_ehc_caller(&env, &initiator)?;
        
        env.storage().persistent().set(&DataKey::GlobalPause, &false);
        events::emit_global_resume(&env);
        Ok(())
    }
}
```

#### Operation Guards

All state-changing operations in subsystem contracts must check global pause:

```rust
pub fn require_platform_operational(env: &Env) -> Result<(), Error> {
    if is_platform_halted(env)? {
        Err(Error::PlatformHalted)
    } else {
        Ok(())
    }
}

// In any state-changing entrypoint:
pub fn place_bet(env: Env, market_id: Symbol, ...) -> Result<(), Error> {
    require_platform_operational(&env)?;
    // ... rest of logic
}
```

## Implementation Phases

### Phase 1: EHC Contract Core
- [ ] Create Emergency Halt Control contract scaffold
- [ ] Implement halt/resume entrypoints
- [ ] Add audit log storage and retrieval
- [ ] Add comprehensive error handling

### Phase 2: Subsystem Integration
- [ ] Add `pause_all_*` / `resume_all_*` entrypoints to:
  - Markets Contract
  - Fees Contract
  - Reporting Contract
  - Validators Contract
  - Disputes Contract (if separate)
- [ ] Add global pause flag to each contract
- [ ] Add `is_platform_halted()` helper to each contract
- [ ] Integrate guard checks into all state-changing operations

### Phase 3: Health Checks & Monitoring
- [ ] Implement health check system
- [ ] Add periodic verification mechanism
- [ ] Create monitoring alerts for halt status

### Phase 4: Testing & Documentation
- [ ] Auth boundary tests for all new entrypoints
- [ ] Integration tests for atomic halt sequence
- [ ] Recovery scenario testing
- [ ] Incident response playbooks

## Error Handling

```rust
pub enum HaltError {
    NotInitialized,
    AlreadyHalted,
    NotHalted,
    NotRecoveryInitiated,
    RecoveryLocked,              // Timelock still active
    RecoveryTimeoutNotReached,
    InvalidTimelock,             // Duration out of bounds
    SubsystemNotResponding,      // Health check failure
    HealthCheckFailed,           // Not all subsystems are paused
    Unauthorized,
    InvalidReason,
}
```

## Security Considerations

1. **Atomic Consistency**: All subsystems must reach halt state or entire transaction rolls back
2. **Authorization**: Only platform admin can initiate halt
3. **Audit Trail**: Every action logged with timestamp and initiator
4. **Recovery Guards**: Timelock prevents accidental instant recovery during active incidents
5. **State Verification**: Health checks confirm all systems are actually paused
6. **TTL Management**: Halt flags must have extended TTLs to survive storage pruning

## Example Usage: Incident Response

```rust
// 1. Detect issue and halt platform immediately
ehc.emergency_halt(admin, "Detected price manipulation attack")?;

// 2. Verify all systems are halted
let health = ehc.health_check()?;
assert!(health.all_halted);

// 3. Investigate the issue (offchain)
// ...investigation occurs...

// 4. Initiate recovery with timelock for safety
ehc.initiate_recovery(admin, 3600)?;  // 1 hour timelock

// 5. After 1 hour, resume operations
ehc.resume_all_subsystems(admin)?;
```

## Deployment Considerations

1. **Contract Address Registry**: All subsystems must know EHC contract address
2. **Configuration Update**: Platform config needs EHC contract address
3. **Gradual Rollout**: Enable EHC features progressively to test integration
4. **Backwards Compatibility**: Existing per-contract pause mechanisms remain functional
5. **Event Emission**: All state transitions emit observable events for monitoring

## Testing Strategy

### Auth Boundary Tests
- ✓ Non-admin cannot initiate halt
- ✓ Non-admin cannot initiate recovery
- ✓ Non-admin cannot resume
- ✓ Read-only operations require no auth

### Functional Tests
- ✓ Halt pauses all subsystems atomically
- ✓ Health check correctly reports halt status
- ✓ Recovery can be initiated when halted
- ✓ Timelock prevents immediate resume
- ✓ Recovery abort keeps halt active
- ✓ Audit log tracks all transitions
- ✓ Partial halt failure rolls back completely

### Integration Tests
- ✓ Cross-contract calls succeed with correct auth
- ✓ State changes propagate to all subsystems
- ✓ Guard checks block operations during halt
- ✓ Events emitted for all state transitions

### Edge Cases
- ✓ Double halt is idempotent
- ✓ Resume without halt fails gracefully
- ✓ Timelock boundaries validated
- ✓ Halt reason string limits enforced
