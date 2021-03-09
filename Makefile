CARGO = cargo

all: release

release: target/wasm32-unknown-unknown/release/aurora_engine.wasm

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml src/lib.rs src/sdk.rs
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=contract -Z avoid-dev-deps

format:
	$(CARGO) fmt

clean:
	@rm -Rf target *~

.PHONY: format clean

.SECONDARY:
.SUFFIXES:
