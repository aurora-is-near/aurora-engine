CARGO = cargo
NEAR  = near
FEATURES = mainnet

ifeq ($(evm-bully),yes)
  FEATURES := $(FEATURES),evm_bully
endif

# TODO: This isn't updating the `FEATURES` for some reason. Disabled to prevent accidental compilation of the same binary for release.
#all: mainnet testnet betanet

mainnet: FEATURES=mainnet
mainnet: mainnet-release.wasm

testnet: FEATURES=testnet
testnet: testnet-release.wasm

betanet: FEATURES=betanet
betanet: betanet-release.wasm

mainnet-release.wasm: compile-release
	ln -sf $< $@

testnet-release.wasm: compile-release
	ln -sf $< $@

betanet-release.wasm: compile-release
	ln -sf $< $@

compile-release: Cargo.toml Cargo.lock $(shell find src -name "*.rs") etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=$(FEATURES) -Z avoid-dev-deps
	ls -l target/wasm32-unknown-unknown/release/aurora_engine.wasm

etc/eth-contracts/res/EvmErc20.bin: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

# TODO: This isn't updating the `FEATURES` for some reason. Disabled to prevent accidental compilation of the same binary for debug.
all-debug: mainnet-debug testnet-debug betanet-debug

mainnet-debug: FEATURES=mainnet
mainnet-debug: mainnet-debug.wasm

testnet-debug: FEATURES=testnet
testnet-debug: testnet-debug.wasm

betanet-debug: FEATURES=betanet
betanet-debug: betanet-debug.wasm

mainnet-debug.wasm: compile-debug
	ln -sf $< $@

testnet-debug.wasm: compile-debug
	ln -sf $< $@

betanet-debug.wasm: compile-debug
	ln -sf $< $@

compile-debug: Cargo.toml Cargo.lock $(wildcard src/*.rs) etc/eth-contracts/res/EvmErc20.bin
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

test-build: etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=mainnet,integration-test,meta-call -Z avoid-dev-deps
	ln -sf target/wasm32-unknown-unknown/release/aurora_engine.wasm release.wasm
	ls -l target/wasm32-unknown-unknown/release/aurora_engine.wasm

.PHONY: all release debug eth-contracts mainnet testnet betanet

deploy: mainnet-release.wasm
	$(NEAR) deploy --account-id=$(or $(NEAR_EVM_ACCOUNT),aurora.test.near) --wasm-file=$<

check: test test-sol check-format check-clippy

check-format:
	$(CARGO) fmt -- --check

check-clippy:
	$(CARGO) clippy --no-default-features --features=$(FEATURES) -- -D warnings

# test depends on release since `tests/test_upgrade.rs` includes `release.wasm`
test: test-build
	$(CARGO) test --features meta-call

test-sol:
	cd etc/eth-contracts && yarn && yarn test

format:
	$(CARGO) fmt

clean:
	@rm -Rf *.wasm
	cargo clean

.PHONY: deploy check check-format check-clippy test format clean mainnet testnet betanet compile-release mainnet-debug testnet-debug betanet-debug compile-debug

.SECONDARY:
.SUFFIXES:
