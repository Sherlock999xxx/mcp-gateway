//! Shared HTTP tool DSL + runtime utilities.
//!
//! This crate is intended to be used by:
//! - `unrelated-mcp-adapter` (standalone mode)
//! - `unrelated-mcp-gateway` (gateway-native HTTP tool sources)
//!
//! It intentionally contains **no** tenant storage logic and **no** gateway-specific policy.

pub mod config;
pub mod response_shaping;
pub mod runtime;
pub mod safety;
pub mod semantics;
