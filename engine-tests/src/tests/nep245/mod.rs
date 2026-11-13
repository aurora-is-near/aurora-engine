//! NEP-245 multi-token connector tests against the compiled engine.
//!
//! Unit tests of the connector logic live next to the implementation in
//! `engine/src/contract_methods/connector/tests.rs`, and standalone replay tests live in
//! `tests::standalone::sync`. Sandbox tests use the `etc/tests/mt-token` and
//! `etc/tests/ft-receiver` fixture contracts; `defuse` downloads the Defuse release.
mod defuse;
mod deploy;
mod deposit;
mod exit;
mod mirror;
mod utils;
