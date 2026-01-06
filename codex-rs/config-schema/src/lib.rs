//! JSON Schema generation for config.toml.
//!
//! This crate provides functionality to generate a JSON Schema for the
//! `~/.codex/config.toml` configuration file. The schema can be used for
//! validation and editor autocompletion.

mod generate;
mod ordering;

pub use generate::SchemaConfig;
pub use generate::generate_config_schema;
