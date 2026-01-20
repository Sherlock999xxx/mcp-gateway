//! Shared OpenAPI->MCP tooling.
//!
//! This crate is intended to be used by:
//! - `unrelated-mcp-adapter` (standalone mode)
//! - `unrelated-mcp-gateway` (gateway-native `OpenAPI` tool sources)
//!
//! It intentionally contains **no** tenant storage logic and **no** gateway-specific policy.

pub mod config;
pub mod error;
pub mod resolver;
pub mod runtime;
