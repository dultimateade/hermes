# Emergency Halt Control - Quick Reference & Incident Response Guide

## System Overview (1 Minute Read)

The **Emergency Halt Control (EHC)** contract is a single-entry-point safety mechanism that freezes the entire Hermes platform in a single atomic transaction.

### Key Facts

| Aspect | Details |
|--------|---------|
| **Purpose** | Freeze all platform activity during critical incidents |
| **Scope** | Affects: Markets, Bets, Disputes, Fees, Validation |
| **Trigger Time** | <1 second from call to complete halt |
| **Auth Required** | Platform admin only |
| **Atomicity** | All subsystems pause/resume together or transaction rolls back |
| **Recovery** | Optional timelock prevents accidental instant resume |
| **Audit Trail** | Complete history of all halt/resume events with timestamps |

---

## API Quick Reference

### Emergency Halt (IMMEDIATE)

```rust
// Halt the entire platform NOW
result = ehc.emergency_halt(
    admin,           // Must be platform admin
    "Detected price feed corruption attack"  // Reason string
)?;

// Returns: HaltConfig {
//   is_halted: true,
//   halt_timestamp: 1728604513,
//   halt_reason: "Detected price feed corruption attack",
//   subsystems_affected: ["Markets", "Fees", "Reporting", ...]
// }
```

### Check Platform Status (NO AUTH)

```rust
// Query current halt status
health = ehc.health_check()?;

// Returns: HealthCheckResult {
//   markets_paused: true,
//   fees_paused: true,
//   reporting_paused: true,
//   validators_paused: true,
//   disputes_paused: true,
//   all_halted: true,
//   check_timestamp: 1728604513,
//   failed_subsystems: [],
// }
```

### Initiate Recovery (WITH TIMELOCK)

```rust
// Start recovery process with safety timelock
config = ehc.initiate_recovery(
    admin,
    3600  // 1-hour timelock before resume allowed
)?;

// Returns: RecoveryConfig {
//   recovery_authorized_by: admin_address,
//   recovery_authorization_time: 1728604600,
//   timelock_duration_seconds: 3600,
//   timelock_expiry_timestamp: 1728608200,  // 1 hour from now
//   is_recovery_locked: true,
// }
```

### Resume Platform (AFTER TIMELOCK)

```rust
// Resume all subsystems
ehc.resume_all_subsystems(admin)?;

// Checks:
// 1. Is platform halted? (Must be YES)
// 2. Is recovery initiated? (Must be YES)
// 3. Has timelock expired? (Must be YES)
// 4. Is caller admin? (Must be YES)
//
// On success:
// - All subsystems resume
// - Platform returns to normal operation
// - Halt flag cleared
```

### Abort Recovery (KEEP HALT ACTIVE)

```rust
// If threat still active, abort recovery and stay halted
ehc.abort_recovery(admin)?;

// Clears recovery state, keeps halt active
// Re-initiate recovery when ready
```

---

## Incident Response Workflow

### **PHASE 1: DETECTION (Happens outside EHC)**
You detect an anomaly:
- Price spike suspicious pattern
- Unusual transaction volume
- Oracle feed inconsistency
- Smart contract exploit attempt

### **PHASE 2: EMERGENCY HALT (<10 seconds)**

```
┌─ Check what happened ─────────┐
│ • Gather telemetry              │
│ • Confirm it's not false alarm  │
│ • Notify incident commander     │
└────────────────────────────────┘
                │
                ▼
┌─ INITIATE HALT ───────────────┐
│ $ hermes halt --reason \       │
│   "Detected price spike from   │
│    $500 to $50K in 2min"       │
│                                │
│ • Atomic - all systems pause   │
│ • Irreversible (no rollback)   │
│ • Audit logged                 │
│ • Events emitted               │
└────────────────────────────────┘
```

### **PHASE 3: VERIFY HALT (<30 seconds)**

```
┌─ Confirm Platform Halted ─────┐
│ $ hermes health-check          │
│                                │
│ Result: {                      │
│   "all_halted": true,         │
│   "subsystems": {             │
│     "markets": "paused",      │
│     "bets": "blocked",        │
│     "disputes": "paused",     │
│     "fees": "paused"          │
│   }                           │
│ }                             │
│                               │
│ ✓ All green = proceed         │
│ ✗ Any red = INVESTIGATE       │
└────────────────────────────────┘
```

**If health check shows partial pause:**
- One or more subsystems failed to halt
- Do NOT attempt recovery
- Review logs to identify which subsystem failed
- Contact engineering team for manual intervention

### **PHASE 4: INVESTIGATION (Duration varies)**

```
Now you have time to investigate without active threats:

• Review blockchain transactions (no new bets/markets)
• Check oracle feeds for tampering
• Audit contract state for inconsistencies
• Analyze root cause of incident
• Develop remediation plan

Typical duration: 15 min - 1 hour
```

### **PHASE 5: RECOVERY DECISION**

```
Decision Point:

  [Root Cause IDENTIFIED & FIXED]
        │
        ▼
  Is fix deployed and tested?
        │
        ├─ NO  ──→ Extend halt, continue investigation
        │           Re-check periodically
        │
        └─ YES ──→ Proceed to recovery
```

### **PHASE 6: INITIATE RECOVERY (With Timelock)**

```
┌─ Initiate Recovery ────────────┐
│ $ hermes recovery --lock 3600  │
│   (1-hour timelock)            │
│                                │
│ • Sets recovery lock           │
│ • Prevents accidental resume   │
│ • Allows incident review       │
│ • Provides operator confirmation
│   moment                        │
└────────────────────────────────┘
```

**Why timelock?**
- Safety margin to detect if threat still active
- Prevents accidental premature resume
- Allows final verification before going live
- Gives incident responder time to communicate

### **PHASE 7: FINAL VERIFICATION (Before Resume)**

```
After timelock duration expired:

┌─ Pre-Resume Checks ────────────┐
│ 1. Health check still shows:   │
│    all_halted = true           │
│ 2. Fix deployed to mainnet?    │
│    YES                         │
│ 3. No new threat indicators?   │
│    YES                         │
│ 4. Incident commander approve? │
│    YES                         │
│ 5. Monitoring team ready?      │
│    YES                         │
└────────────────────────────────┘
        │ All YES?
        ▼ YES
    ┌─ Resume Platform ──────────┐
    │ $ hermes resume            │
    │                            │
    │ • All subsystems active    │
    │ • Bets, markets can proceed│
    │ • Platform live again      │
    │ • Audit logged             │
    └────────────────────────────┘
```

### **PHASE 8: POST-RECOVERY MONITORING**

```
First hour after resume:

• Monitor health_check() every 30 seconds
• Watch transaction volume (ramping up normally?)
• Check for any new anomalies
• Verify oracle feeds consistent
• Ensure fee collection working

If issues detected → HALT AGAIN immediately
If all good → Return to normal monitoring
```

---

## Common Incident Scenarios

### Scenario 1: Exploiter Found Vulnerability

```
Timeline:
  T+0min  : Anomaly detected (unusual transaction pattern)
  T+2min  : Operator initiates halt
  T+2min  : Health check confirms halt
  T+3min  : Engineering begins investigation
  T+45min : Root cause found (integer overflow in bet calc)
  T+50min : Fix deployed and tested
  T+55min : Recovery initiated with 1-hour timelock
  T+1h55min: Timelock expires
  T+1h56min: Final verification passed
  T+1h57min: Resume platform
  T+1h58min: Platform live, normal operation resumes

Total downtime: ~2 hours
Potential loss prevented: Millions (due to early detection)
```

### Scenario 2: False Alarm

```
Timeline:
  T+0min  : Volume spike detected (looks malicious)
  T+1min  : Operator halts platform (conservative approach)
  T+2min  : Investigation begins
  T+5min  : Root cause identified (legitimate bot trading)
  T+6min  : Recovery initiated with 30-min timelock
  T+36min : Timelock expires
  T+37min : Resume platform

Total downtime: ~37 minutes
Benefit: Avoided cascading issue from unvetted bot
```

### Scenario 3: Oracle Feed Malfunction

```
Timeline:
  T+0min  : Reported price $1.5M (was $1,500 seconds ago)
  T+0.5min: Halt triggered
  T+5min  : Health check confirms halt
  T+15min : Oracle provider confirms feed error
  T+20min : Oracle provider fixes feed
  T+25min : Fix verified with live price $1,500
  T+26min : Recovery initiated with 10-min timelock
  T+36min : Timelock expires
  T+37min : Resume platform

Total downtime: ~37 minutes
Damage prevented: Massive liquidations from false price
```

---

## Admin CLI Commands (Pseudocode)

```bash
# View current status
hermes status

# View halt configuration
hermes halt show-config

# Initiate emergency halt
hermes halt emergency --reason "detected price feed corruption"

# Check platform health
hermes halt health-check

# Get audit log
hermes halt audit-log --limit 50 --offset 0

# Get halt history
hermes halt history

# Initiate recovery with timelock
hermes halt initiate-recovery --timelock-seconds 3600

# Check recovery status
hermes halt recovery-status

# Complete recovery (after timelock expires)
hermes halt resume

# Abort recovery (keep halted)
hermes halt abort-recovery

# View real-time events
hermes events tail --topic PlatformHalted
hermes events tail --topic PlatformResumed
```

---

## Metrics & Monitoring Dashboard

### Key Metrics to Track

| Metric | Target | Alert Threshold |
|--------|--------|-----------------|
| Halt Latency | <1s | >2s |
| Resume Latency | <1s | >2s |
| Health Check Accuracy | 99%+ | <95% |
| Subsystem Response Time | <500ms | >1s |
| Failed Pause Attempts | 0/week | ≥1 |
| Halt->Resume Time | Incident dependent | - |
| Recovery Timelock Average | 30-60 min | >4 hours |

### Dashboard Panels

1. **Halt Status Panel**
   - Current state: Operational / HaltInitiated / HaltActive / RecoveryInitiated
   - Time since last halt
   - Last halt reason
   - Last halt duration

2. **Subsystem Health Panel**
   - Markets: Operational / Paused / Unhealthy
   - Fees: Operational / Paused / Unhealthy
   - Reporting: Operational / Paused / Unhealthy
   - Validators: Operational / Paused / Unhealthy
   - Disputes: Operational / Paused / Unhealthy

3. **Recovery Panel**
   - Recovery locked: Yes / No
   - Timelock remaining: HH:MM:SS
   - Can resume now: Yes / No

4. **Incident History Panel**
   - Last 10 halt events
   - Duration each
   - Reason each
   - Outcome each

---

## Troubleshooting Guide

### Problem: Health Check Shows Partial Halt

**Symptom:**
```
health_check() returns:
{
  "all_halted": false,
  "markets_paused": true,
  "fees_paused": false,  ← NOT PAUSED
  "reporting_paused": true,
  ...
}
```

**Diagnosis:**
- Fees contract may be uninitialized
- Fees contract address not registered in EHC
- Fees contract pause call timed out
- Network issue with fees contract call

**Action:**
1. Check if Fees contract is deployed
2. Verify Fees contract address in EHC registry
3. Manually pause Fees contract
4. Review logs for timeout errors
5. Retry health check

---

### Problem: Cannot Resume After Timelock Expires

**Symptom:**
```
resume() returns error: "RecoveryLocked"
```

**Diagnosis:**
- Timelock hasn't actually expired yet (clock skew?)
- Recovery was never initiated
- Recovery was aborted

**Action:**
1. Check timelock expiry timestamp
2. Compare with current ledger time
3. If timelock not expired yet: wait
4. If timelock expired: check recovery status
5. If recovery aborted: re-initiate recovery

---

### Problem: Health Check Hangs / Slow

**Symptom:**
```
health_check() takes >5 seconds
```

**Diagnosis:**
- One or more subsystem calls slow
- Network latency
- Subsystem contract busy

**Action:**
1. Run health check with timeout
2. Identify which subsystem is slow
3. Check subsystem contract logs
4. Consider manual verification of that subsystem
5. Try health check again

---

### Problem: Resume Partially Succeeds

**Symptom:**
```
resume_all_subsystems() returns success, but
health_check() shows some still paused
```

**Diagnosis:**
- Event emission delay
- One subsystem failed mid-transaction
- Clock skew between contracts

**Action:**
1. Wait 2 seconds
2. Run health_check() again
3. If issue persists: review transaction logs
4. Manually resume remaining subsystems
5. Create incident ticket

---

## Emergency Contacts & Escalation

### Escalation Path

```
⏱️ 0-5 min    : Operator confirms incident
                ↓
              Initiate halt (if clear threat)
                ↓
⏱️ 5-10 min   : Notify incident commander
                ↓
              Notify engineering lead
                ↓
⏱️ 10-30 min  : Investigation phase
                ↓
              If needed: Notify C-level
                ↓
⏱️ 30-60+ min : Resolution phase
                ↓
              Timelock -> Recovery
                ↓
⏱️ 60+ min    : Post-incident review
```

### Contact List
- **On-call Operator**: [Number/Slack]
- **Incident Commander**: [Number/Slack]
- **Engineering Lead**: [Number/Slack]
- **Security Team**: [Number/Slack]
- **Executive Escalation**: [Number/Slack]

---

## Post-Incident Review Checklist

After every halt event, complete:

- [ ] Document incident timeline
- [ ] Identify root cause
- [ ] Review audit log for anomalies
- [ ] Verify no missed halt signatures
- [ ] Check for data consistency issues
- [ ] Validate recovery completed cleanly
- [ ] Update runbooks with findings
- [ ] Assign preventive action items
- [ ] Schedule incident retrospective
- [ ] Brief incident team on lessons learned

---

## Training & Certification

### For Platform Admins: Required Training
- [ ] Understanding EHC architecture
- [ ] When to trigger emergency halt
- [ ] Health check interpretation
- [ ] Recovery procedure with timelock
- [ ] Incident response workflow
- [ ] Escalation procedures
- [ ] Post-incident review process

### Practice Drills: Monthly
- [ ] Trigger halt on testnet
- [ ] Verify health check
- [ ] Initiate recovery
- [ ] Resume platform
- [ ] Audit log review
- [ ] Time the entire sequence

### Certification Requirement
- [ ] Pass written exam (75%+ score)
- [ ] Complete 2 successful practice drills
- [ ] Signed acknowledgment of procedures
- [ ] Annual recertification

---

## FAQ

**Q: Can I stop a halt once initiated?**  
A: No, halt is irreversible. Recovery requires explicit action with timelock.

**Q: How long can the platform stay halted?**  
A: Indefinitely. There's no automatic timeout. Halt remains until admin initiates recovery.

**Q: What happens to in-flight transactions?**  
A: Those already submitted continue to completion. New operations after halt are blocked.

**Q: Can I adjust the timelock duration?**  
A: Yes, during `initiate_recovery()` call. Range: 0 - 24 hours (configurable).

**Q: What if an admin halts maliciously?**  
A: Requires admin auth only. Use your access control procedures to prevent compromised admins. Audit log shows who initiated halt and when.

**Q: Does halt affect oracle feeds?**  
A: No, oracles continue running. Only market operations, fee collection, and dispute resolution are paused.

**Q: Can I partially resume subsystems?**  
A: No, resume is all-or-nothing. Requires all subsystems respond successfully.

**Q: Is there a maximum halt duration?**  
A: No, but extended halts should trigger escalation alerts.

**Q: Can I test halt on mainnet?**  
A: Not recommended. Use testnet for all drills. Emergency halt should only be used for actual incidents.

---

## Additional Resources

- **Full Design Doc**: `/PLATFORM_EMERGENCY_HALT_DESIGN.md`
- **Implementation Plan**: `/EMERGENCY_HALT_IMPLEMENTATION_PLAN.md`
- **Integration Guide**: `/EMERGENCY_HALT_INTEGRATION_GUIDE.md`
- **Contract Code**: `/contracts/emergency-halt/src/`
- **Test Suite**: `/contracts/emergency-halt/tests/`
- **Monitoring**: Configured in platform dashboard

---

## Version History

| Date | Version | Changes |
|------|---------|---------|
| 2024-09-26 | 1.0 | Initial design |
| - | - | - |

---

**Last Updated**: 2024-09-26  
**Status**: APPROVED FOR IMPLEMENTATION  
**Next Review**: Post-deployment (1 week)
