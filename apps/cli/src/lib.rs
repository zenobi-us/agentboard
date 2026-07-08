//! Public library surface for the AgentBoard CLI.
//!
//! The CLI owns workspace loading, validation, local store paths, run/watch
//! orchestration, template rendering, and dispatch into source/action crates.

pub mod adapters;
pub mod cli;
pub mod config;
pub mod runtime;
pub mod store;
pub mod template;
