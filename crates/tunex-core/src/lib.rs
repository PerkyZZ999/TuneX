//! `tunex-core`: pure domain layer for `TuneX`.
//!
//! Owns shared domain types, the [`AppEvent`] bus vocabulary, configuration
//! schema, and crate error types. Deliberately dependency-free (outside `std`)
//! so the library, player, and app crates can all build on it without cycles.
//!
//! # Dependency rules (D-008, enforced by review + `cargo-shear`)
//!
//! - No `cxx-qt`/Qt, `GStreamer`, `SQLite`, or `notify` imports — ever.
//! - `tunex-library` and `tunex-player` communicate only through these types
//!   plus channels; they never import each other.

pub mod config;
pub mod error;

pub use config::{TunexConfig, config_dir, config_file, load_from, save_to};
pub use error::{Error, Result};
