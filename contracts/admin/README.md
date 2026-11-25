# Admin Contract

Admin contract with per-entrypoint gas regression testing for the Predictify platform.

## Overview

This contract provides core admin functionality with comprehensive gas snapshot testing to ensure CPU and memory usage remains within acceptable bounds. Each entrypoint is tested individually with a 5% regression limit enforced by CI.

## Entrypoints

Every entrypoint below is a `#[contractimpl]` method on `AdminContract` in
`src/lib.rs`. The **required role** column is the authorization the contract
itself enforces — there is no other gate in front of these methods, so this
table is the audit surface for admin access.

| Entrypoint | Required role / authorization | Mutates storage | Returns |
| --- | --- | --- | --- |
| `initialize(env, admin)` | The `admin` address must authorize the call (`admin.require_auth()`). Succeeds at most once. | Yes — `DataKey::Admin` (instance) | `Result<(), ContractError>` |
| `admin(env)` | None — read-only view. | No | `Result<Address, ContractError>` |
| `set_admin_cooldown(env, admin, seconds)` | `admin.require_auth()` **and** `admin` must equal the stored admin. `seconds` must be at least 60. | Yes — `DataKey::AdminCooldownSeconds` (persistent) | `Result<(), ContractError>` |
| `disable_admin_cooldown(env, admin)` | `admin.require_auth()` **and** `admin` must equal the stored admin. | Yes — `DataKey::AdminCooldownSeconds` (persistent, set to 0) | `Result<(), ContractError>` |
| `get_admin_cooldown(env)` | None — read-only view. | No | `u64` (`0` when unset or explicitly disabled) |
| `check_admin_cooldown(env, admin, function_name)` | `admin.require_auth()` **and** `admin` must equal the stored admin. | Yes — `DataKey::AdminLastAction(function_name)` on success only | `Result<(), ContractError>` |
| `propose_ownership(env, current_admin, new_admin)` | `current_admin.require_auth()` **and** `current_admin` must equal the stored admin. | Yes — `DataKey::PendingOwnership` (persistent) | `Result<(), ContractError>` |
| `accept_ownership(env, new_admin)` | `new_admin.require_auth()` **and** it must match the pending address. | Yes — updates `DataKey::Admin` and removes the pending proposal | `Result<(), ContractError>` |
| `pending_ownership(env)` | None — read-only view. | No | `Option<PendingOwnership>` |
| `cancel_ownership_transfer(env, current_admin)` | `current_admin.require_auth()` **and** `current_admin` must equal the stored admin. | Yes — removes `DataKey::PendingOwnership` | `Result<(), ContractError>` |

### Authorization notes

- **`initialize`** is the only way to set the admin, and it is deliberately not
  idempotent: a second call returns `AlreadyInitialized` (`1`).
- **`set_admin_cooldown`**, **`disable_admin_cooldown`**, and
  **`check_admin_cooldown`** return `AdminNotSet` (`2`) before initialization,
  and `Unauthorized` (`3`) unless the caller is the stored admin. Authorization
  is checked before any storage write. Use `disable_admin_cooldown` to explicitly
  disable enforcement; `set_admin_cooldown` rejects zero.
- **`check_admin_cooldown`** is the throttle other contracts call. While the
  cooldown is active it returns `AdminActionTimelocked` (`4`) and does **not**
  update the last-action timestamp, so a blocked call does not extend the window.
- **Views** (`admin`, `get_admin_cooldown`) require no authorization and expose
  only non-secret configuration.
- **Ownership transfer** is two-step. `propose_ownership` stores the new admin
  and a 86,400-second acceptance timelock. The role does not change until
  `accept_ownership` is called by the proposed address after the timelock.
  `transfer_ownership` is retained as a compatibility alias for
  `propose_ownership`; it does not immediately change ownership. The current
  admin may replace or cancel a pending proposal.

### Signatures

```rust
pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError>
pub fn admin(env: Env) -> Result<Address, ContractError>
pub fn set_admin_cooldown(env: Env, admin: Address, seconds: u64) -> Result<(), ContractError>
pub fn disable_admin_cooldown(env: Env, admin: Address) -> Result<(), ContractError>
pub fn get_admin_cooldown(env: Env) -> u64
pub fn check_admin_cooldown(env: Env, admin: Address, function_name: Symbol) -> Result<(), ContractError>
pub fn propose_ownership(env: Env, current_admin: Address, new_admin: Address) -> Result<(), ContractError>
pub fn accept_ownership(env: Env, new_admin: Address) -> Result<(), ContractError>
pub fn pending_ownership(env: Env) -> Option<PendingOwnership>
pub fn cancel_ownership_transfer(env: Env, current_admin: Address) -> Result<(), ContractError>
```

## Gas Snapshots

Per-entrypoint gas snapshots are maintained in `tests/gas_snap.rs` with the following baselines:

- `initialize`: CPU 55,643, Memory 20,337
- `admin`: CPU 31,614, Memory 12,495
- `set_admin_cooldown`: CPU 45,000, Memory 18,000
- `get_admin_cooldown`: CPU 28,000, Memory 10,000
- `check_admin_cooldown`: CPU 52,000, Memory 19,000

CI enforces a maximum 5% regression on both CPU and memory metrics.

## Testing

Run gas snapshot tests:
```bash
cargo test -p admin --test gas_snap
```

## Error Codes

- `AlreadyInitialized = 1`: Contract already initialized
- `AdminNotSet = 2`: No admin configured
- `Unauthorized = 3`: Caller is not the admin
- `AdminActionTimelocked = 4`: Cooldown period has not elapsed
- `InvalidCooldown = 5`: Cooldown must be at least 60 seconds
- `InvalidNewOwner = 6`: Proposed owner is the current admin
- `OwnershipTransferNotProposed = 7`: No pending ownership transfer exists
- `OwnershipTransferTimelocked = 8`: The proposal's acceptance timelock has not elapsed
