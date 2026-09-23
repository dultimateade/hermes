.PHONY: test deploy-testnet

test:
	cargo test --workspace

deploy-testnet:
	@test -n "$(CONTRACT)" || (echo "CONTRACT is required, e.g. make deploy-testnet CONTRACT=markets SOURCE_ACCOUNT=deployer" >&2; exit 2)
	@test -n "$(SOURCE_ACCOUNT)" || (echo "SOURCE_ACCOUNT is required, e.g. make deploy-testnet CONTRACT=markets SOURCE_ACCOUNT=deployer" >&2; exit 2)
	CONTRACT="$(CONTRACT)" SOURCE_ACCOUNT="$(SOURCE_ACCOUNT)" NETWORK="$(or $(NETWORK),testnet)" ./scripts/deploy_testnet.sh