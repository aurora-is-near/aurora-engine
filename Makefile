CARGO = cargo
NEAR  = near
FEATURES = mainnet

ifeq ($(evm-bully),yes)
  FEATURES := $(FEATURES),evm_bully
endif

# TODO: This isn't updating the `FEATURES` for some reason. Disabled to prevent accidental compilation of the same binary for release.
#all: mainnet testnet betanet

release: mainnet

mainnet: FEATURES=mainnet
mainnet: mainnet-release.wasm

testnet: FEATURES=testnet
testnet: testnet-release.wasm

betanet: FEATURES=betanet
betanet: betanet-release.wasm

mainnet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

testnet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

betanet-release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(shell find src -name "*.rs") etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

etc/eth-contracts/res/EvmErc20.bin: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

# TODO: This isn't updating the `FEATURES` for some reason. Disabled to prevent accidental compilation of the same binary for debug.
#all-debug: mainnet-debug testnet-debug betanet-debug

debug: mainnet-debug

mainnet-debug: FEATURES=mainnet
mainnet-debug: mainnet-debug.wasm

testnet-debug: FEATURES=testnet
testnet-debug: testnet-debug.wasm

betanet-debug: FEATURES=betanet
betanet-debug: betanet-debug.wasm

mainnet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

testnet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

betanet-debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	cp $< $@

target/wasm32-unknown-unknown/debug/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs) etc/eth-contracts/res/EvmErc20.bin
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

# test depends on release since `tests/test_upgrade.rs` includes `mainnet-release.wasm`
test: test-mainnet

mainnet-test-build: FEATURES=mainnet,integration-test,meta-call
mainnet-test-build: mainnet-test.wasm

betanet-test-build: FEATURES=betanet,integration-test,meta-call
betanet-test-build: betanet-test.wasm

testnet-test-build: FEATURES=testnet,integration-test,meta-call
testnet-test-build: testnet-test.wasm

mainnet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

testnet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

betanet-test.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	cp $< $@

test-mainnet: mainnet-test-build
	$(CARGO) test --features mainnet-test

test-testnet: testnet-test-build
	$(CARGO) test --features testnet-test

test-betanet: betanet-test-build
	$(CARGO) test --features betanet-test

deploy: mainnet-release.wasm
	$(NEAR) deploy --account-id=$(or $(NEAR_EVM_ACCOUNT),aurora.test.near) --wasm-file=$<

check: test test-sol check-format check-clippy

check-format:
	$(CARGO) fmt -- --check

check-clippy:
	$(CARGO) clippy --no-default-features --features=$(FEATURES) -- -D warnings

test-sol:
	cd etc/eth-contracts && yarn && yarn test

format:
	$(CARGO) fmt

clean:
	@rm -Rf *.wasm
	@rm -Rf etc/eth-contracts/res
	cargo clean

.PHONY: release mainnet testnet betanet compile-release test-build deploy check check-format check-clippy test test-sol format clean debug mainnet-debug testnet-debug betanet-debug compile-debug mainnet-test-build testnet-test-build betanet-test-build target/wasm32-unknown-unknown/release/aurora_engine.wasm target/wasm32-unknown-unknown/debug/aurora_engine.wasm

.SECONDARY:
.SUFFIXES:
