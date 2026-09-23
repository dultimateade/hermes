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