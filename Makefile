CARGO = cargo

all: release

release: release.wasm

release.wasm: target/wasm32-unknown-unknown/release/aurora_engine.wasm
	ln -sf $< $@

target/wasm32-unknown-unknown/release/aurora_engine.wasm: Cargo.toml Cargo.lock $(wildcard src/*.rs)
	RUSTFLAGS='-C link-arg=-s' $(CARGO) build --target wasm32-unknown-unknown --release --no-default-features --features=contract -Z avoid-dev-deps

format:
	$(CARGO) fmt

clean:
	@rm -Rf release.wasm target *~

.PHONY: format clean

.SECONDARY:
.SUFFIXES:
