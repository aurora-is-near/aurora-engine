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

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs)
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

debug: debug.wasm

debug.wasm: target/wasm32-unknown-unknown/debug/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/debug/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs)
	$(CARGO) build --target wasm32-unknown-unknown --no-default-features --features=$(FEATURES) -Z avoid-dev-deps

.PHONY: all release debug

deploy: release.wasm
	$(NEAR) deploy --account-id=$(or $(NEAR_EVM_ACCOUNT),evm.test.near) --wasm-file=$<

check:
	$(CARGO) test

format:
	$(CARGO) fmt

clean:
	@rm -Rf release.wasm target *~

.PHONY: deploy check format clean

.SECONDARY:
.SUFFIXES:
