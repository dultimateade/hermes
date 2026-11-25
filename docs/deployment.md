# Testnet Deployment

Deploy a contract from the repository root with the Stellar CLI and a configured
CLI identity:

```bash
stellar keys generate deployer
make deploy-testnet CONTRACT=markets SOURCE_ACCOUNT=deployer
```

The command builds the selected Cargo package for `wasm32v1-none`, verifies that
the expected WASM artifact exists, and deploys it to Stellar testnet. Use a
different configured network with `NETWORK` when needed:

```bash
make deploy-testnet CONTRACT=fees SOURCE_ACCOUNT=deployer NETWORK=testnet
```

The `CONTRACT` value is the package name from the contract's `Cargo.toml`.
Package names containing dashes are supported. The deployer identity must be
funded on the selected network before deployment.

This workflow deploys the WASM only. Contract-specific initialization, if
required by a future contract, should be run as an explicit follow-up command
using the returned contract ID.

## Full deployment sequence

Deploying a contract with `make deploy-testnet` only uploads the WASM. Stateful
contracts must then be initialized with `stellar contract invoke` before they
accept traffic, and the oracle registry must be populated before it can return
prices.

### What each contract needs at init

Every `initialize` entrypoint takes the deployer's Stellar **account** address
as `admin` (the same identity used for `SOURCE_ACCOUNT`), calls
`admin.require_auth()`, and rejects re-initialization, so it must be invoked
exactly once per contract. None of the current contracts take a sibling
contract's ID as an `initialize` argument — there is no init-time
cross-contract dependency to satisfy. Contract addresses are bound at runtime
instead (see [Runtime wiring](#runtime-wiring)).

Contracts with an `initialize` entrypoint:

| Contract | `initialize` signature | Required parameters |
|---|---|---|
| `admin` | `initialize(admin: Address)` | `admin` account |
| `allowlist` | `initialize(admin: Address)` | `admin` account |
| `monitor` | `initialize(admin: Address)` | `admin` account |
| `reporting` | `initialize(admin: Address)` | `admin` account |
| `fees` | `initialize(admin: Address)` | `admin` account (also seeds the default fee config, 2.00% platform fee) |
| `validators` | `initialize(admin: Address, min_stake: i128, max_stake: i128)` | `admin` account, stake window in stroops (`0 ≤ min_stake ≤ max_stake`) |
| `migrate` | `initialize(admin: Address, version: u32)` | `admin` account, target version (`1 ≤ version ≤ 2`) |
| `predictify-hybrid` | `initialize(admin: Address, platform_fee_percentage: Option<i128>, allowed_assets: Option<Vec<Address>>)` | `admin` account, optional platform fee in basis points (must fall within the contract's enforced min/max), optional allowlist of asset contract addresses |

Contracts with **no `initialize`** — deploy only, then use their runtime
entrypoints:

| Contract | Post-deploy setup |
|---|---|
| `oracles` | Register each price-feed oracle with `add_oracle(admin: Address, oracle: Address)` before any consumer reads prices |
| `markets` | Stub — only `version` and `get_market` are implemented; no init, no cross-contract wiring |
| `resolution` | Stub — only `version`; no init |
| `disputes` | Stub — only `version`; no init |
| `auction` | Stateless `capabilities`; ready immediately |
| `analytics` | Stateless `record_event` / `aggregate` / `percentile`; ready immediately |

`betting` is a library/event-surface crate, not a deployable contract, so it is
not deployed on its own.

### Recommended order

1. **Fund the deployer identity** on the target network. This account is the
   `admin` passed to every `initialize`, so reuse a single identity for the whole
   sequence.
2. **Admin-bearing contracts** (`admin`, `validators`, `fees`, `allowlist`,
   `monitor`, `reporting`, `migrate`) — deploy and `initialize` each, passing the
   same admin account. They are independent of one another; deploy `admin` first
   so its record is the canonical admin reference. `validators` and `migrate`
   additionally require `min_stake` / `max_stake` and `version`.
3. **No-init contracts** (`oracles`, `markets`, `resolution`, `disputes`,
   `auction`, `analytics`) — deploy. Then populate the `oracles` registry with
   `add_oracle` before any consumer reads prices.
4. **Monolith** (`predictify-hybrid`) — if you deploy the full-featured single
   contract instead of the modular stubs, `initialize` it with the admin account
   and the optional fee/allowlist. Oracle addresses are bound per market at
   creation time, not here.

### Example: deploy and initialize one contract

```bash
# Admin account used for every initialize call
ADMIN_ADDRESS=$(stellar keys address deployer)

# 1. Deploy
make deploy-testnet CONTRACT=admin SOURCE_ACCOUNT=deployer
# → prints: Deployed admin to testnet: <ADMIN_CONTRACT_ID>

# 2. Initialize (call exactly once; requires the admin's signature)
stellar contract invoke \
  --id <ADMIN_CONTRACT_ID> \
  --source-account deployer \
  --network testnet \
  -- initialize --admin "$ADMIN_ADDRESS"
```

For contracts with extra parameters, pass them after `--admin`:

```bash
# validators: also set the stake window in stroops
stellar contract invoke --id <VALIDATORS_CONTRACT_ID> --source-account deployer --network testnet \
  -- initialize --admin "$ADMIN_ADDRESS" --min_stake 100000000 --max_stake 1000000000000

# migrate: also set the target version (1 or 2)
stellar contract invoke --id <MIGRATE_CONTRACT_ID> --source-account deployer --network testnet \
  -- initialize --admin "$ADMIN_ADDRESS" --version 2

# predictify-hybrid: Option args take JSON (null means None)
stellar contract invoke --id <PREDICTIFY_CONTRACT_ID> --source-account deployer --network testnet \
  -- initialize --admin "$ADMIN_ADDRESS" --platform-fee-percentage null --allowed-assets null
```

### Runtime wiring

Contract addresses are not passed into `initialize`; they are bound at runtime
in two places:

- **Oracle registry** — after deploying `oracles`, register each external
  price-feed contract so `get_price` / `get_price_data` can call it:
  ```bash
  stellar contract invoke --id <ORACLES_CONTRACT_ID> --source-account deployer --network testnet \
    -- add_oracle --admin "$ADMIN_ADDRESS" --oracle <ORACLE_PRICE_FEED_CONTRACT_ID>
  ```
- **Per-market oracle** — in `predictify-hybrid`, each market is created with its
  own `oracle_config.oracle_address` (and optional `fallback_oracle_config`) via
  `create_market`. The oracle address is therefore supplied when a market is
  created, not when the contract is initialized.

> The flags in the `invoke` examples mirror the on-chain parameter names. If
> your Stellar CLI version rejects underscores, use the kebab-case equivalent
> (for example `--min-stake`) or confirm the exact spelling with
> `stellar contract invoke --help`.

## Verify staging and production

After deploying the same commit to both environments, compare the deployed
WASM binaries with:

```bash
make verify-contract-sync \
	CONTRACT=markets \
	STAGING_CONTRACT_ID=C... \
	PRODUCTION_CONTRACT_ID=C... \
	STAGING_NETWORK=testnet \
	PRODUCTION_NETWORK=mainnet
```

The command builds the selected package once, fetches both deployed WASMs with
the Stellar CLI, and fails if either SHA-256 hash differs from the local build.
It requires configured CLI network and identity settings for both networks.