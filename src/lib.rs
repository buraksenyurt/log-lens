//! Log Lens library crate.
//!
//! The UI (app, components) and the pure logic (parser, stats, filter) live here
//! so that the binary can mount the app and native benchmarks/tests can reuse
//! the same code without a browser.

pub mod app;
pub mod components;
pub mod filter;
pub mod parser;
pub mod stats;
