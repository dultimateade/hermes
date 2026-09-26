# Emergency Halt Architecture - Technical Integration Guide

## Current vs. Proposed Architecture

### Current State: Per-Contract Pause Mechanisms

```
┌─────────────────────────────────────────────────────────────┐
│                        Admin                                 │
└──────────┬──────────────┬──────────────┬──────────────┬──────┘
           │              │              │              │
     ┌─────▼───┐    ┌─────▼───┐   ┌─────▼───┐   ┌─────▼───┐
     │ Markets │    │  Fees   │   │Reporting│   │Validator│
     │ Contract│    │ Contract│   │ Contract│   │ Contract│
     │         │    │         │   │         │   │         │
     │ pause() │    │ pause() │   │ pause() │   │ pause() │
     │resume() │    │resume() │   │resume() │   │resume() │
     └─────────┘    └─────────┘   └─────────┘   └─────────┘
     
Issues:
  • Multiple transactions required
  • No atomicity guarantee
  • Race conditions possible
  • Operational complexity
  • No unified audit trail
```

### Proposed State: Platform-Wide Emergency Halt Control

```
┌─────────────────────────────────────────────────────────────┐
│                   Platform Admin                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
            ┌──────────▼──────────┐
            │ Emergency Halt      │
            │ Control (EHC)       │
            │ Contract (NEW)      │
            │                     │
            │ • halt()            │
            │ • resume()          │
            │ • health_check()    │
            │ • audit_log()       │
            └─┬──┬──┬──┬──┬──────┘
              │  │  │  │  │
        ┌─────┘  │  │  │  └─────┐
        │        │  │  │        │
   ┌────▼───┐ ┌─▼──▼──▼──┐ ┌───▼────┐
   │ Markets│ │ Fees +   │ │Reporting
   │ Halt   │ │ Reporting│ │ + Others
   │ Guard  │ │ + Val +  │ │ Halt
   │        │ │ Disputes │ │ Guards
   │ Blocks:│ │          │ │ Blocks:
   │ • ops  │ │ Blocks:  │ │ • ops
   │ • bets │ │ • record │ │ • submit
   │ • votes│ │ • collect│ │ • verify
   │        │ │ • submit │ │ • vote
   └────────┘ └──────────┘ └────────┘

Benefits:
  ✓ Single atomic transaction
  ✓ Strong consistency guarantee
  ✓ Complete audit trail
  ✓ Deterministic incident response
  ✓ No race conditions
```

---

## Data Flow: Emergency Halt Sequence

### Sequence 1: Platform Halt

```
Admin  →  EHC.emergency_halt(reason)
            ├─ Verify admin auth
            ├─ Create halt entry
            ├─ Call Markets::pause_all_markets()
            │  └─ Set GlobalPause = true
            ├─ Call Fees::pause_all_fees()
            │  └─ Set GlobalPause = true
            ├─ Call Reporting::pause_all_reporting()
            │  └─ Set GlobalPause = true
            ├─ Call Validators::pause_all_validators()
            │  └─ Set GlobalPause = true
            ├─ Call Disputes::pause_all_disputes()
            │  └─ Set GlobalPause = true
            ├─ Record halt in audit log
            ├─ Emit PlatformHalted event
            └─ Return HaltConfig {
                 is_halted: true,
                 halt_timestamp: ledger_time,
                 halt_reason: reason,
                 subsystems_paused: [
                   "Markets", "Fees", "Reporting", 
                   "Validators", "Disputes"
                 ]
               }

If ANY subsystem call fails:
  → Transaction ROLLS BACK
  → No partial state
  → Complete transparency on failure cause
```

### Sequence 2: Platform Resume

```
Admin  →  EHC.initiate_recovery(timelock_secs)
            ├─ Verify halt state
            ├─ Verify admin auth
            ├─ Set recovery_locked = true
            ├─ Store timelock_expiry = now + timelock_secs
            ├─ Record recovery initiation in audit log
            ├─ Emit RecoveryInitiated event
            └─ Return RecoveryConfig

[Wait timelock_secs seconds...]

Admin  →  EHC.resume_all_subsystems()
            ├─ Verify recovery initiated
            ├─ Verify timelock expired
            ├─ Call Markets::resume_all_markets()
            │  └─ Set GlobalPause = false
            ├─ Call Fees::resume_all_fees()
            │  └─ Set GlobalPause = false
            ├─ [Similar for all subsystems...]
            ├─ Clear halt flag
            ├─ Clear recovery state
            ├─ Record resume in audit log
            ├─ Emit PlatformResumed event
            └─ Return Ok(())
```

### Sequence 3: Health Check

```
Query  →  EHC.health_check()
            ├─ No auth required
            ├─ Query Markets::is_platform_halted()
            ├─ Query Fees::is_platform_halted()
            ├─ Query Reporting::is_platform_halted()
            ├─ Query Validators::is_platform_halted()
            ├─ Query Disputes::is_platform_halted()
            └─ Return HealthCheckResult {
                 markets_paused: bool,
                 fees_paused: bool,
                 reporting_paused: bool,
                 validators_paused: bool,
                 disputes_paused: bool,
                 all_halted: bool,
                 check_timestamp: u64,
                 failed_subsystems: Vec<String>
               }
```

---

## Storage Schema

### EHC Contract Storage

```rust
// Instance storage (not subject to TTL expiry)
Admin              → Address           // Platform admin

// Persistent storage with extended TTL
HaltConfig         → {
  is_halted: bool,
  halt_reason: String,
  halt_initiated_by: Address,
  halt_timestamp: u64,
  halt_state: HaltState,
  subsystems_affected: Vec<String>,
}

RecoveryConfig     → {
  recovery_authorized_by: Address,
  recovery_authorization_time: u64,
  timelock_duration_seconds: u64,
  timelock_expiry_timestamp: u64,
  is_recovery_locked: bool,
}

AuditLogIndex      → u32              // Number of entries
AuditLogEntry_{n}  → {                // Per-entry storage
  timestamp: u64,
  action: HaltAction,
  initiated_by: Address,
  reason: String,
  subsystems_affected: Vec<String>,
  success: bool,
  error_details: Option<String>,
}

SubsystemRegistry  → Vec<{            // Contract addresses
  name: String,
  contract_address: Address,
  is_active: bool,
}>
```

### Subsystem Contract Changes

Each affected contract adds to its DataKey enum:

```rust
pub enum DataKey {
    // Existing keys...
    Admin,
    
    // NEW: Global platform halt flag
    GlobalPause,  // bool: true = platform halted
    
    // Existing local pause mechanisms remain:
    // e.g. ReportingPaused, ValidatorsPaused, etc.
}
```

---

## Cross-Contract Interface

### Markets Contract

```rust
impl MarketsContract {
    /// Pause all markets immediately (called by EHC)
    /// 
    /// # Authorization
    /// Only the configured EHC contract address can call this.
    /// The caller must authenticate via require_auth().
    pub fn pause_all_markets(env: Env, ehc_contract: Address) 
        -> Result<(), Error> {
        ehc_contract.require_auth();
        verify_ehc_contract(&env, &ehc_contract)?;
        
        env.storage()
            .persistent()
            .set(&DataKey::GlobalPause, &true);
        
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::GlobalPause, 1_000_000, 1_000_000);
            
        events::emit_markets_paused(&env, "platform_emergency_halt");
        Ok(())
    }
    
    /// Resume all markets (called by EHC)
    pub fn resume_all_markets(env: Env, ehc_contract: Address) 
        -> Result<(), Error> {
        ehc_contract.require_auth();
        verify_ehc_contract(&env, &ehc_contract)?;
        
        env.storage()
            .persistent()
            .set(&DataKey::GlobalPause, &false);
        
        events::emit_markets_resumed(&env);
        Ok(())
    }
    
    /// Check if platform-wide halt is active (read-only, no auth)
    pub fn is_platform_halted(env: Env) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::GlobalPause)
            .unwrap_or(false)
    }
}

// In every state-changing entrypoint:
pub fn place_bet(env: Env, market_id: Symbol, ...) -> Result<(), Error> {
    // NEW: Check platform halt first
    if MarketsContract::is_platform_halted(&env) {
        return Err(Error::PlatformHalted);
    }
    
    // ... existing validation and logic
}
```

### Fees Contract (Similar Pattern)

```rust
impl FeesContract {
    pub fn pause_all_fees(env: Env, ehc_contract: Address) 
        -> Result<(), Error> { ... }
    
    pub fn resume_all_fees(env: Env, ehc_contract: Address) 
        -> Result<(), Error> { ... }
    
    pub fn is_platform_halted(env: Env) -> bool { ... }
}

// Note: Local pause remains independent
pub fn record_fee(env: Env, ...) -> Result<(), Error> {
    // Check BOTH local pause AND platform halt
    require_fees_not_paused(&env)?;  // Existing local check
    if is_platform_halted(&env) {
        return Err(Error::PlatformHalted);
    }
    
    // ... rest of logic
}
```

### Reporting, Validators, Disputes: Same Pattern

---

## Error Handling Strategy

### EHC Error Types

```rust
pub enum HaltError {
    // Initialization & State
    NotInitialized,
    AlreadyHalted,
    NotHalted,
    
    // Recovery State
    NotRecoveryInitiated,
    RecoveryLocked,              // Timelock still active
    RecoveryTimeoutNotReached,
    InvalidTimelock,             // Out of bounds
    
    // Subsystem Coordination
    SubsystemPauseFailed {
        subsystem: String,
        details: String,
    },
    SubsystemResumeFailed {
        subsystem: String,
        details: String,
    },
    HealthCheckFailed {
        failed_subsystems: Vec<String>,
    },
    
    // Authorization & Validation
    Unauthorized,
    InvalidReason,              // Too long or invalid chars
    InvalidAddress,
}
```

### Subsystem Error Extension

```rust
// Each subsystem adds to its Error enum:
pub enum Error {
    // Existing errors...
    
    // NEW: Platform halt
    PlatformHalted,             // All state-changing ops return this
}
```

### Error Propagation

```
EHC calls Markets::pause_all_markets()
  ├─ If succeeds: Continue to next subsystem
  ├─ If fails: Capture error and bubble up
  └─ Transaction rolls back completely
  
Caller receives HaltError::SubsystemPauseFailed {
    subsystem: "Markets",
    details: "Insufficient budget for TTL extension"
}
```

---

## Event Emission Strategy

### EHC Events

```rust
// In events.rs
#[derive(Clone)]
pub struct PlatformHaltedEvent {
    pub timestamp: u64,
    pub initiated_by: Address,
    pub reason: String,
    pub subsystems_affected: Vec<String>,
}

#[derive(Clone)]
pub struct RecoveryInitiatedEvent {
    pub timestamp: u64,
    pub initiated_by: Address,
    pub timelock_duration_seconds: u64,
    pub timelock_expiry_timestamp: u64,
}

#[derive(Clone)]
pub struct PlatformResumedEvent {
    pub timestamp: u64,
    pub resume_initiated_by: Address,
    pub halt_duration_seconds: u64,
    pub subsystems_affected: Vec<String>,
}

#[derive(Clone)]
pub struct HealthCheckEvent {
    pub timestamp: u64,
    pub all_halted: bool,
    pub subsystems_status: Vec<(String, bool)>,
    pub failed_subsystems: Vec<String>,
}

// Emission
pub fn emit_platform_halted(env: &Env, event: &PlatformHaltedEvent) {
    env.events().publish(("PlatformHalted",), event);
}
```

### Subsystem Events

```rust
// In each subsystem (e.g., markets.rs)
pub fn emit_markets_paused(env: &Env, reason: &str) {
    env.events().publish(("MarketsGlobalPause",), reason);
}

pub fn emit_markets_resumed(env: &Env) {
    env.events().publish(("MarketsGlobalResume",), ());
}
```

---

## Integration Checklist for Each Subsystem

### For Markets, Fees, Reporting, Validators, Disputes:

- [ ] Add `GlobalPause` to `DataKey` enum
- [ ] Add `pause_all_*()` entrypoint with EHC auth check
- [ ] Add `resume_all_*()` entrypoint with EHC auth check
- [ ] Add `is_platform_halted()` query function
- [ ] Add `PlatformHalted` error variant
- [ ] Integrate guard into ALL state-changing entrypoints:
  - [ ] Check `is_platform_halted()` at function start
  - [ ] Return `Err(Error::PlatformHalted)` if true
  - [ ] Update error documentation
- [ ] Add pause/resume events for global halt
- [ ] Update auth boundary tests:
  - [ ] Test `pause_all_*` requires EHC auth
  - [ ] Test `is_platform_halted()` requires no auth
  - [ ] Test all state-changing ops fail with correct error
- [ ] Add functional tests:
  - [ ] Verify pause blocks specific operations
  - [ ] Verify resume allows operations
  - [ ] Verify idempotency of pause/resume
- [ ] Update README with new pause behavior

---

## Testing Pyramid

### Unit Tests (Per Contract)
- [ ] Halt flag storage/retrieval
- [ ] Pause/resume entrypoint logic
- [ ] Auth checks for pause/resume
- [ ] Error handling and propagation
- [ ] Event emission

### Integration Tests (Multi-Contract)
- [ ] EHC calls pause on each subsystem
- [ ] All subsystems report halted via health check
- [ ] EHC calls resume on each subsystem
- [ ] Operations proceed after resume
- [ ] Timelock prevents early resume
- [ ] Audit log captures all transitions

### End-to-End Tests (Full Platform)
- [ ] Concurrent operations before halt
- [ ] Halt blocks all operations atomically
- [ ] Health check confirms halt
- [ ] Recovery initiated with timelock
- [ ] Resume restores operations
- [ ] Concurrent operations after resume

### Edge Case Tests
- [ ] Double halt (idempotent)
- [ ] Resume without halt (error)
- [ ] Abort recovery keeps halt active
- [ ] Timelock boundary at exact expiry
- [ ] Platform halt vs. local pause (both work)
- [ ] Halt during mid-operation transaction (rolls back)

---

## Monitoring & Observability

### Health Check Frequency
- **Normal**: Every 5 minutes (via scheduler)
- **After Halt**: Every 30 seconds (verify all subsystems)
- **During Recovery**: Every 10 seconds (confirm timelock)

### Key Metrics
- **Halt Latency**: Time from call to all subsystems paused (target: <1s)
- **Resume Latency**: Time from call to all subsystems active (target: <1s)
- **Health Check Accuracy**: % of checks that match actual state (target: 99%+)
- **Incident Response Time**: Detection to halt (target: <10s)

### Alerts
- [ ] Alert if health check fails for any subsystem
- [ ] Alert if halt initiated (operational logging)
- [ ] Alert if recovery initiated
- [ ] Alert if resume completes
- [ ] Alert if timelock expires but resume not called

---

## Deployment Validation

Before going live:

1. **Contract Compilation**
   - [ ] No warnings or errors
   - [ ] Wasm size within budget
   - [ ] All tests pass locally

2. **Integration Testing**
   - [ ] Halt pauses all subsystems
   - [ ] Resume restores all subsystems
   - [ ] Health check accurate
   - [ ] Audit log captures all transitions

3. **Security Review**
   - [ ] Auth checks on cross-contract calls
   - [ ] No privilege escalation vectors
   - [ ] Timelock prevents bypass
   - [ ] Audit trail tamper-proof

4. **Performance**
   - [ ] Halt completes in <1s
   - [ ] Guard checks don't impact normal throughput
   - [ ] Health check query fast (<100ms)

5. **Operational**
   - [ ] Admin training complete
   - [ ] Incident response playbook reviewed
   - [ ] Monitoring configured
   - [ ] Alert recipients notified

---

## Configuration Management

### EHC Configuration

```json
{
  "emergency_halt_enabled": true,
  "subsystems": [
    {
      "name": "Markets",
      "contract_address": "CAAAA...",
      "is_critical": true,
      "pause_timeout_ms": 5000
    },
    {
      "name": "Fees",
      "contract_address": "CAAAB...",
      "is_critical": true,
      "pause_timeout_ms": 3000
    },
    {
      "name": "Reporting",
      "contract_address": "CAAAC...",
      "is_critical": false,
      "pause_timeout_ms": 3000
    },
    {
      "name": "Validators",
      "contract_address": "CAAAD...",
      "is_critical": false,
      "pause_timeout_ms": 2000
    },
    {
      "name": "Disputes",
      "contract_address": "CAAAE...",
      "is_critical": true,
      "pause_timeout_ms": 5000
    }
  ],
  "default_recovery_timelock_seconds": 3600,
  "max_recovery_timelock_seconds": 86400,
  "audit_log_retention_days": 365,
  "health_check_interval_seconds": 300
}
```

---

## Next Steps

1. **Design Review**: Validate architecture with team
2. **Create EHC Contract**: Implement Phase 1
3. **Integrate Subsystems**: Update each contract per Phase 2
4. **Cross-Contract Testing**: Verify atomic behavior
5. **Deployment**: Rollout to production
6. **Monitoring**: Enable health checks and alerts
