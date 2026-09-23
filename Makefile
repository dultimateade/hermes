.PHONY: test deploy-testnet verify-contract-sync

test:
	cargo test --workspace

deploy-testnet:
	@test -n "$(CONTRACT)" || (echo "CONTRACT is required, e.g. make deploy-testnet CONTRACT=markets SOURCE_ACCOUNT=deployer" >&2; exit 2)
	@test -n "$(SOURCE_ACCOUNT)" || (echo "SOURCE_ACCOUNT is required, e.g. make deploy-testnet CONTRACT=markets SOURCE_ACCOUNT=deployer" >&2; exit 2)
	CONTRACT="$(CONTRACT)" SOURCE_ACCOUNT="$(SOURCE_ACCOUNT)" NETWORK="$(or $(NETWORK),testnet)" ./scripts/deploy_testnet.sh

verify-contract-sync:
	@test -n "$(CONTRACT)" || (echo "CONTRACT is required, e.g. make verify-contract-sync CONTRACT=markets STAGING_CONTRACT_ID=C... PRODUCTION_CONTRACT_ID=C..." >&2; exit 2)
	@test -n "$(STAGING_CONTRACT_ID)" || (echo "STAGING_CONTRACT_ID is required" >&2; exit 2)
	@test -n "$(PRODUCTION_CONTRACT_ID)" || (echo "PRODUCTION_CONTRACT_ID is required" >&2; exit 2)
	CONTRACT="$(CONTRACT)" STAGING_CONTRACT_ID="$(STAGING_CONTRACT_ID)" PRODUCTION_CONTRACT_ID="$(PRODUCTION_CONTRACT_ID)" STAGING_NETWORK="$(or $(STAGING_NETWORK),testnet)" PRODUCTION_NETWORK="$(or $(PRODUCTION_NETWORK),mainnet)" ./scripts/verify_contract_sync.sh