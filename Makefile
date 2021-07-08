CARGO = cargo
NEAR  = near
FEATURES = mainnet

ifeq ($(evm-bully),yes)
  FEATURES := $(FEATURES),evm_bully
endif

all: release

mainnet: FEATURES=mainnet
mainnet: release

testnet: FEATURES=testnet
testnet: release

betanet: FEATURES=betanet
betanet: release

release: release.wasm

release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(shell find src -name "*.rs") etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=$(FEATURES) -Z avoid-dev-deps
	ls -l target/wasm32-unknown-unknown/release/aurora_engine.wasm

etc/eth-contracts/res/EvmErc20.bin: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json: $(shell find etc/eth-contracts/contracts -name "*.sol") etc/eth-contracts/package.json
	cd etc/eth-contracts && yarn && yarn build

debug: debug.wasm

debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/debug/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs) etc/eth-contracts/res/EvmErc20.bin
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

test-build: etc/eth-contracts/artifacts/contracts/test/StateTest.sol/StateTest.json etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=mainnet,integration-test,meta-call -Z avoid-dev-deps
	ln -sf target/wasm32-unknown-unknown/release/aurora_engine.wasm release.wasm
	ls -l target/wasm32-unknown-unknown/release/aurora_engine.wasm

.PHONY: all release debug eth-contracts mainnet testnet betanet

deploy: release.wasm
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
	@rm -Rf *.wasm target *~

.PHONY: deploy check check-format check-clippy test format clean

.SECONDARY:
.SUFFIXES:
