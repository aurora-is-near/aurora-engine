CARGO = cargo
NEAR  = near
FEATURES = contract

ifeq ($(evm-bully),yes)
  FEATURES := $(FEATURES),evm_bully
endif

all: release

release: release.wasm

release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs) etc/eth-contracts/res/EvmErc20.bin
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

etc/eth-contracts/res/EvmErc20.bin:
	cd etc/eth-contracts && yarn && yarn compile

debug: debug.wasm

debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/debug/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs)
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

.PHONY: all release debug

deploy: release.wasm
	$(NEAR) deploy --account-id=$(or $(NEAR_EVM_ACCOUNT),aurora.test.near) --wasm-file=$<

check: test check-format check-clippy

check-format:
	$(CARGO) fmt -- --check

check-clippy:
	$(CARGO) +nightly clippy --no-default-features --features=$(FEATURES) -- -D warnings

# test depends on release since `tests/test_upgrade.rs` includes `release.wasm`
test: release
	$(CARGO) test

format:
	$(CARGO) fmt

clean:
	@rm -Rf *.wasm target *~

.PHONY: deploy check check-format check-clippy test format clean

.SECONDARY:
.SUFFIXES:
