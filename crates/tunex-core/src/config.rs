//! Persistent application configuration (`~/.config/tunex/config.toml`).
//!
//! The schema grows additively, one settings section per slice. Every field
//! has a default (`#[serde(default)]`) and unknown TOML keys are ignored, so
//! older files always load after upgrades — covered by the round-trip and
//! forward-compatibility tests below.

use std::{
    env,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Repeat behavior for the queue (player slice owns the semantics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// Play through once and stop.
    #[default]
    Off,
    /// Repeat the queue.
    All,
    /// Repeat the current track.
    One,
}

/// Playback behavior settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    /// Whether output starts muted.
    pub muted: bool,
    /// Whether the queue shuffles.
    pub shuffle: bool,
    /// Queue repeat behavior.
    pub repeat_mode: RepeatMode,
}

/// Appearance and accessibility settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    /// Honor the OS reduce-motion preference (disables crossfades).
    pub reduce_motion: bool,
    /// Fall back to opaque surfaces (disables blur/transparency).
    pub reduce_transparency: bool,
}

/// Application settings. Sections land slice by slice; see SPEC §30.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TunexConfig {
    /// Watched music folders (XDG/music dirs chosen by the user).
    pub library_roots: Vec<PathBuf>,
    /// Output volume, `0.0` (mute) through `1.0` (full). Clamped on load.
    pub volume: f32,
    /// Playback behavior.
    pub playback: PlaybackConfig,
    /// Appearance and accessibility.
    pub appearance: AppearanceConfig,
}

impl Default for TunexConfig {
    fn default() -> Self {
        Self {
            library_roots: Vec::new(),
            volume: 1.0,
            playback: PlaybackConfig::default(),
            appearance: AppearanceConfig::default(),
        }
    }
}

impl TunexConfig {
    /// Clamp invariants that deserialization alone cannot express.
    fn normalize(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.0);
    }
}

/// Resolve the user configuration directory (`$XDG_CONFIG_HOME` or
/// `~/.config`), with the `tunex` leaf appended.
#[must_use]
pub fn config_dir() -> PathBuf {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        // Least surprise when neither is set (service/CI contexts): stay local.
        .unwrap_or_else(|| PathBuf::from(".config"));
    base.join("tunex")
}

/// Full path of the settings file.
#[must_use]
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// Load settings from `path`. Missing files are not an error — first runs
/// simply start from defaults (callers decide when to persist).
///
/// # Errors
///
/// Returns [`Error::Io`] when an existing file cannot be read and
/// [`Error::Config`] when its TOML does not parse.
pub fn load_from(path: &Path) -> Result<TunexConfig> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TunexConfig::default());
        }
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let mut config: TunexConfig =
        toml::from_str(&text).map_err(|source| Error::Config(source.to_string()))?;
    config.normalize();
    Ok(config)
}

/// Persist settings to `path`, creating parent directories as needed.
///
/// # Errors
///
/// Returns [`Error::Io`] when directories or the file cannot be written.
pub fn save_to(path: &Path, config: &TunexConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let text =
        toml::to_string_pretty(config).map_err(|source| Error::Config(source.to_string()))?;
    std::fs::write(path, text).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch directory unique to this process; removed afterwards.
    fn scratch_dir(case: &str) -> PathBuf {
        std::env::temp_dir().join(format!("tunex-config-test-{}-{case}", std::process::id()))
    }

    #[test]
    fn missing_file_loads_defaults() {
        let config = load_from(&scratch_dir("missing").join("config.toml"))
            .expect("missing file is not an error");
        assert_eq!(config, TunexConfig::default());
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let dir = scratch_dir("round-trip");
        let path = dir.join("config.toml");
        let config = TunexConfig {
            library_roots: vec![PathBuf::from("/music"), PathBuf::from("/media/nas/flac")],
            volume: 0.42,
            playback: PlaybackConfig {
                muted: true,
                shuffle: true,
                repeat_mode: RepeatMode::All,
            },
            appearance: AppearanceConfig {
                reduce_motion: true,
                reduce_transparency: false,
            },
        };
        save_to(&path, &config).expect("save works");
        let back = load_from(&path).expect("reload works");
        assert_eq!(back, config);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn volume_is_clamped_on_load() {
        let dir = scratch_dir("clamp");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "volume = 7.5\n").expect("setup works");
        let config = load_from(&path).expect("clamped load works");
        // Bit-exact: clamping to the literal yields its exact representation.
        assert_eq!(config.volume.to_bits(), 1.0f32.to_bits());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn unknown_future_keys_are_ignored() {
        let dir = scratch_dir("future");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "volume = 0.5\nvisualizer_mode = \"bars\"\n").expect("setup works");
        let config = load_from(&path).expect("future keys must not break loads");
        assert_eq!(config.volume.to_bits(), 0.5f32.to_bits());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn corrupt_toml_surfaces_config_error() {
        let dir = scratch_dir("corrupt");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "volume = [unclosed\n").expect("setup works");
        let err = load_from(&path).expect_err("corrupt files must be visible");
        assert!(matches!(err, Error::Config(_)));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
