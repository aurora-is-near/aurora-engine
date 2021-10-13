CARGO = cargo
NEAR  = near
FEATURES = mainnet
# More strict clippy rules
FEATURES_CLIPPY = contract
ADDITIONAL_FEATURES =

ifeq ($(evm-bully),yes)
  ADDITIONAL_FEATURES := $(ADDITIONAL_FEATURES),evm_bully
endif

ifeq ($(error-refund),yes)
  ADDITIONAL_FEATURES := $(ADDITIONAL_FEATURES),error_refund
endif

# TODO: This isn't updating the `FEATURES` for some reason. Disabled to prevent accidental compilation of the same binary.
# all: mainnet testnet betanet
# all-debug: mainnet-debug testnet-debug betanet-debug

release: mainnet
debug: mainnet-debug
check: test test-sol check-format check-clippy
test: test-mainnet

deploy: mainnet-release.wasm
	$(NEAR) deploy --account-id=$(or $(NEAR_EVM_ACCOUNT),aurora.test.near) --wasm-file=$<

mainnet: FEATURES=mainnet
mainnet: mainnet-release.wasm
mainnet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

testnet: FEATURES=testnet
testnet: testnet-release.wasm
testnet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

betanet: FEATURES=betanet
betanet: betanet-release.wasm
betanet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

mainnet-debug: FEATURES=mainnet
mainnet-debug: mainnet-debug.wasm
mainnet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

testnet-debug: FEATURES=testnet
testnet-debug: testnet-debug.wasm
testnet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

betanet-debug: FEATURES=betanet
betanet-debug: betanet-debug.wasm
betanet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

# test builds depend on release since `tests/test_upgrade.rs` includes `mainnet-release.wasm`

test-mainnet: mainnet-test-build
	$(CARGO) test --features mainnet-test$(ADDITIONAL_FEATURES)
mainnet-test-build: FEATURES=mainnet,integration-test,meta-call
mainnet-test-build: mainnet-test.wasm
mainnet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

test-testnet: testnet-test-build
	$(CARGO) test --features testnet-test$(ADDITIONAL_FEATURES)
testnet-test-build: FEATURES=testnet,integration-test,meta-call
testnet-test-build: testnet-test.wasm
testnet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

test-betanet: betanet-test-build
	$(CARGO) test --features betanet-test$(ADDITIONAL_FEATURES)
betanet-test-build: FEATURES=betanet,integration-test,meta-call
betanet-test-build: betanet-test.wasm
betanet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(shell find src -name "*.rs") etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build \
		--target wasm32-unknown-unknown \
		--release \
		--verbose \
		--no-default-features \
		--features=$(FEATURES)$(ADDITIONAL_FEATURES) \
		-Z avoid-dev-deps

target/wasm32-unknown-unknown/debug/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs) etc/eth-contracts/res/EvmErc20.bin
	$(CARGO) build \
		--target wasm32-unknown-unknown \
		--no-default-features \
		--features=$(FEATURES)$(ADDITIONAL_FEATURES) \
		-Z avoid-dev-deps

etc/eth-contracts/res/EvmErc20.bin: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

check-format:
	$(CARGO) fmt -- --check

check-clippy:
	$(CARGO) clippy --no-default-features --features=$(FEATURES_CLIPPY)$(ADDITIONAL_FEATURES) -- -D warnings

test-sol:
	cd etc/eth-contracts && yarn && yarn test

format:
	$(CARGO) fmt

clean:
	@rm -Rf *.wasm
	@rm -Rf etc/eth-contracts/res
	cargo clean

.PHONY: release debug check test deploy
.PHONY: mainnet mainnet-debug test-mainnet mainnet-test-build
.PHONY: testnet testnet-debug test-testnet testnet-test-build
.PHONY: betanet betanet-debug test-betanet betanet-test-build
.PHONY: target/wasm32-unknown-unknown/release/aurora_engine.wasm
.PHONY: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
.PHONY: check-format check-clippy test-sol format clean

.SECONDARY:
.SUFFIXES:
