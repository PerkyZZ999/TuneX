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
pub mod playback;

pub use config::{
    AppearanceConfig, TunexConfig, WindowConfig, config_dir, config_file, data_dir,
    library_db_path, load_from, save_to, update,
};
pub use error::{Error, Result};
pub use playback::{PlaybackState, PlayerEvent, RepeatMode};
