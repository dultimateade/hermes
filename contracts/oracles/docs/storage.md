# Oracles Contract Storage

## Instance Storage

The contract does not currently store any keys in instance storage.

## Persistent Storage

| Key | Type | Description |
|-----|------|-------------|
| `Symbol("OracleList")` (`DataKey::OracleList`) | `Vec<Address>` | Ordered list of registered oracle addresses |
| `DataKey::MarketResult(market_id, oracle_address)` | `String` | Outcome submitted by an oracle for a market; each market/oracle pair has its own entry |

`market_id` is a `Symbol`; `oracle_address` is the submitting oracle's `Address`.

## Temporary Storage

The contract does not currently store any keys in temporary storage.

## State Diagram

```
EmptyRegistry
   │
   ├── add_oracle(admin, oracle)
   │      ├── requires `capabilities()` to include `CapabilityFlag::GET_PRICE`
   │      └── push_back → NonEmptyRegistry
   │
   ├── remove_oracle(admin, oracle)
   │      └── ItemCount─1 → EmptyRegistry / NonEmptyRegistry
   │
   └── list_oracles()
          └── Reads registry (with TTL bump)

submit_market_result(market_id, oracle_address, result)
   └── Requires oracle_address authorization
          └── Writes DataKey::MarketResult(market_id, oracle_address)
                 └── Sets a one-year TTL

get_result(market_id, oracle_address)
   └── Reads the matching market result (with TTL renewal)
```

## TTL Management

The oracle list key is bumped on every read via `bump_registry_ttl()`:

- **Threshold**: `REGISTRY_TTL_BUMP_THRESHOLD = 120_960` ledgers
- **Extend to**: `REGISTRY_TTL_BUMP_TO = 518_400` ledgers

Each `MarketResult` persistent entry is assigned a one-year TTL on submission:

- **TTL**: `MARKET_RESULT_TTL_LEDGERS = 6_307_200` ledgers (approximately 365 days at five seconds per ledger)
- **Read renewal threshold**: `MARKET_RESULT_TTL_BUMP_THRESHOLD = 120_960` ledgers (seven days)
- **Read renewal target**: `MARKET_RESULT_TTL_LEDGERS`

`get_result` renews an existing entry to one year when fewer than seven days
remain. An entry that is not submitted again or read near expiry is therefore
removed after its TTL elapses.
