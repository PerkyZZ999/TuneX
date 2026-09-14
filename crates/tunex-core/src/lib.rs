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
pub mod equalizer;
pub mod error;
pub mod playback;

pub use config::{
    AppearanceConfig, EnrichmentConfig, LibraryProfile, NotifyConfig, PlaybackConfig, TunexConfig,
    ViewConfig, WindowConfig, active_roots, config_dir, config_file, data_dir, library_db_path,
    load_from, profile_db_path, sanitize_profile_id, save_to, sort_dir_is_desc, update,
};
pub use equalizer::{
    EQ_BAND_COUNT, EQ_BAND_LABELS, EQ_GAIN_MAX, EQ_GAIN_MIN, EQ_PRESET_IDS, SPECTRUM_BANDS,
    WAVEFORM_SAMPLES, bands_from_pcm, clamp_bands, clamp_gain, matching_preset,
    normalize_spectrum_db, parse_pcm_f32le, parse_pcm_s16le, preset_bands, push_waveform,
};
pub use error::{Error, Result};
pub use playback::{PlaybackState, PlayerEvent, RepeatMode, ReplayGainMode, VisualizerMode};
