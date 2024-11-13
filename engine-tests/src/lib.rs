#![allow(stable_features)]
#![feature(lazy_cell)]
#![deny(clippy::pedantic, clippy::nursery)]
#![allow(clippy::unreadable_literal, clippy::module_name_repetitions)]
#[cfg(test)]
mod benches;
#[cfg(test)]
mod prelude;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod utils;
