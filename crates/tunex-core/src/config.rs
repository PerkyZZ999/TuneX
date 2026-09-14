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

use super::playback::{RepeatMode, ReplayGainMode};
use crate::{Error, Result};

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
    /// Last playing URI (`file://…`). Empty/absent means nothing to restore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_uri: Option<String>,
    /// Last playback position in milliseconds (best-effort).
    #[serde(default)]
    pub last_position_ms: u64,
    /// `ReplayGain` mode. Missing tags stay at unity.
    pub replaygain: ReplayGainMode,
    /// Fade length in seconds on the about-to-finish handoff (0–12). `0` is a gapless cut.
    pub crossfade_secs: u8,
    /// `PipeWire`/`GStreamer` sink id. Empty means the system default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub output_device: String,
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

/// Window and desktop-shell settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Hide to the tray instead of quitting when the window closes.
    ///
    /// Default is on: this is a playback app, and the tray keeps the
    /// session reachable. Closing still quits when no tray host is present,
    /// so a missing tray never leaves a headless process.
    pub close_to_tray: bool,
    /// Last window width in pixels. `0` means “never saved” (use default).
    #[serde(default)]
    pub width: u32,
    /// Last window height in pixels. `0` means “never saved” (use default).
    #[serde(default)]
    pub height: u32,
    /// Last window x, when the session saved a position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    /// Last window y, when the session saved a position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            width: 0,
            height: 0,
            x: None,
            y: None,
        }
    }
}

impl WindowConfig {
    /// Shell minimum (DESIGN.md). Saved sizes clamp up to this.
    pub const MIN_WIDTH: u32 = 960;
    /// Shell minimum (DESIGN.md). Saved sizes clamp up to this.
    pub const MIN_HEIGHT: u32 = 640;
    /// First-run default width.
    pub const DEFAULT_WIDTH: u32 = 1280;
    /// First-run default height.
    pub const DEFAULT_HEIGHT: u32 = 800;

    /// Width to restore: default when unset, otherwise at least the minimum.
    #[must_use]
    pub fn restored_width(&self) -> u32 {
        if self.width == 0 {
            Self::DEFAULT_WIDTH
        } else {
            self.width.max(Self::MIN_WIDTH)
        }
    }

    /// Height to restore: default when unset, otherwise at least the minimum.
    #[must_use]
    pub fn restored_height(&self) -> u32 {
        if self.height == 0 {
            Self::DEFAULT_HEIGHT
        } else {
            self.height.max(Self::MIN_HEIGHT)
        }
    }
}

/// Last-used browse prefs (library tab and sort).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewConfig {
    /// Last Your Library tab (`songs` / `albums` / `artists` / `folders`).
    pub library_tab: String,
    /// Songs-tab sort key (`title` / `artist` / `album` / `date`).
    pub songs_sort: String,
    /// Songs-tab direction (`asc` / `desc`). Empty uses the default for the key.
    pub songs_sort_dir: String,
    /// Albums-tab sort key (`title` / `artist` / `date`).
    pub albums_sort: String,
    /// Albums-tab direction (`asc` / `desc`). Empty uses the default for the key.
    pub albums_sort_dir: String,
    /// Artists-tab sort key (`name` / `songs`).
    pub artists_sort: String,
    /// Artists-tab direction (`asc` / `desc`). Empty uses the default for the key.
    pub artists_sort_dir: String,
    /// Recent search queries, newest first, capped by the search view.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_searches: Vec<String>,
}

/// Resolve a stored sort direction. Empty/`unknown` keeps date and artist
/// track-count newest/most-first so legacy configs do not flip.
#[must_use]
pub fn sort_dir_is_desc(sort_key: &str, stored: &str) -> bool {
    match stored {
        "asc" => false,
        "desc" => true,
        _ => matches!(sort_key, "date" | "songs"),
    }
}

/// Named library profile (own index + roots). The empty id is the default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryProfile {
    /// Stable id (`default` or a slug). Used in the data-dir path.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Watched folders for this profile.
    pub library_roots: Vec<PathBuf>,
}

/// Opt-in online enrichment. Default off (D-014).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EnrichmentConfig {
    /// `MusicBrainz` / Cover Art Archive. Never required; only fills missing tags/art.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub musicbrainz: bool,
}

fn enrichment_is_off(config: &EnrichmentConfig) -> bool {
    !config.musicbrainz
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
    /// Window close, tray, and geometry.
    pub window: WindowConfig,
    /// Last library tab and sort.
    pub view: ViewConfig,
    /// Active library profile id. Empty means the default (`library.db`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub active_profile: String,
    /// Extra named profiles (the default uses `library_roots` on this struct).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<LibraryProfile>,
    /// Opt-in online enrichment (default off).
    #[serde(default, skip_serializing_if = "enrichment_is_off")]
    pub enrichment: EnrichmentConfig,
}

impl Default for TunexConfig {
    fn default() -> Self {
        Self {
            library_roots: Vec::new(),
            volume: 1.0,
            playback: PlaybackConfig::default(),
            appearance: AppearanceConfig::default(),
            window: WindowConfig::default(),
            view: ViewConfig::default(),
            active_profile: String::new(),
            profiles: Vec::new(),
            enrichment: EnrichmentConfig::default(),
        }
    }
}

impl TunexConfig {
    /// Clamp invariants that deserialization alone cannot express.
    fn normalize(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.0);
        self.playback.crossfade_secs = self.playback.crossfade_secs.min(12);
        if self.window.width > 0 {
            self.window.width = self.window.width.max(WindowConfig::MIN_WIDTH);
        }
        if self.window.height > 0 {
            self.window.height = self.window.height.max(WindowConfig::MIN_HEIGHT);
        }
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

/// Resolve the user data directory (`$XDG_DATA_HOME` or `~/.local/share`),
/// with the `tunex` leaf appended. The library index lives here, so a
/// restart reopens the same database (R-006).
#[must_use]
pub fn data_dir() -> PathBuf {
    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        // Least surprise when neither is set (service/CI contexts): stay local.
        .unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join("tunex")
}

/// Full path of the library index database for the active profile.
#[must_use]
pub fn library_db_path() -> PathBuf {
    profile_db_path(&load_from(&config_file()).unwrap_or_default())
}

/// Index path for one config snapshot.
#[must_use]
pub fn profile_db_path(config: &TunexConfig) -> PathBuf {
    if config.active_profile.is_empty() {
        data_dir().join("library.db")
    } else {
        data_dir()
            .join("profiles")
            .join(sanitize_profile_id(&config.active_profile))
            .join("library.db")
    }
}

/// Roots for the active profile (default uses `library_roots`).
#[must_use]
pub fn active_roots(config: &TunexConfig) -> Vec<PathBuf> {
    if config.active_profile.is_empty() {
        return config.library_roots.clone();
    }
    config
        .profiles
        .iter()
        .find(|profile| profile.id == config.active_profile)
        .map(|profile| profile.library_roots.clone())
        .unwrap_or_default()
}

/// Slug a profile id: lowercase ASCII letters, digits, and hyphen.
#[must_use]
pub fn sanitize_profile_id(id: &str) -> String {
    let slug: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "library".to_owned()
    } else {
        slug.chars().take(40).collect()
    }
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

/// Load, patch, and persist settings in one step. Missing files start from
/// defaults, so the first write still creates `config.toml`.
///
/// # Errors
///
/// Returns [`Error::Io`] or [`Error::Config`] from [`load_from`] / [`save_to`].
pub fn update(path: &Path, apply: impl FnOnce(&mut TunexConfig)) -> Result<()> {
    let mut config = load_from(path)?;
    apply(&mut config);
    save_to(path, &config)
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
                last_uri: None,
                last_position_ms: 0,
                replaygain: ReplayGainMode::Track,
                crossfade_secs: 4,
                output_device: "alsa_output.pci-0.analog-stereo".to_owned(),
            },
            appearance: AppearanceConfig {
                reduce_motion: true,
                reduce_transparency: false,
            },
            window: WindowConfig {
                close_to_tray: false,
                width: 1100,
                height: 760,
                x: Some(40),
                y: Some(80),
            },
            view: ViewConfig {
                library_tab: "albums".to_owned(),
                songs_sort: "artist".to_owned(),
                songs_sort_dir: "desc".to_owned(),
                albums_sort: "date".to_owned(),
                albums_sort_dir: String::new(),
                artists_sort: "songs".to_owned(),
                artists_sort_dir: String::new(),
                recent_searches: vec!["nova".to_owned()],
            },
            active_profile: String::new(),
            profiles: Vec::new(),
            enrichment: EnrichmentConfig::default(),
        };
        save_to(&path, &config).expect("save works");
        let back = load_from(&path).expect("reload works");
        assert_eq!(back, config);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn last_track_round_trips() {
        let dir = scratch_dir("last-track");
        let path = dir.join("config.toml");
        let mut config = TunexConfig::default();
        config.playback.last_uri = Some("file:///music/still-here.flac".to_owned());
        config.playback.last_position_ms = 12_345;
        save_to(&path, &config).expect("save works");
        let back = load_from(&path).expect("reload works");
        assert_eq!(
            back.playback.last_uri.as_deref(),
            Some("file:///music/still-here.flac")
        );
        assert_eq!(back.playback.last_position_ms, 12_345);
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
    fn playback_enrichment_clamps_and_round_trips() {
        assert_eq!(
            TunexConfig::default().playback.replaygain,
            ReplayGainMode::Off
        );
        assert_eq!(TunexConfig::default().playback.crossfade_secs, 0);
        assert!(TunexConfig::default().playback.output_device.is_empty());
        let dir = scratch_dir("enrichment");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(
            &path,
            "[playback]\ncrossfade_secs = 99\nreplaygain = \"album\"\n",
        )
        .expect("setup works");
        let loaded = load_from(&path).expect("legacy-large fade loads");
        assert_eq!(loaded.playback.crossfade_secs, 12);
        assert_eq!(loaded.playback.replaygain, ReplayGainMode::Album);
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

    #[test]
    fn close_to_tray_defaults_on_and_round_trips_off() {
        assert!(
            TunexConfig::default().window.close_to_tray,
            "V1 default keeps playback in the tray"
        );
        let dir = scratch_dir("close-to-tray");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        // Older files without a [window] section pick up the default.
        std::fs::write(&path, "volume = 0.5\n").expect("setup works");
        let loaded = load_from(&path).expect("legacy file loads");
        assert!(loaded.window.close_to_tray);
        update(&path, |config| config.window.close_to_tray = false).expect("patch works");
        let back = load_from(&path).expect("reload works");
        assert!(!back.window.close_to_tray);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn library_db_lives_under_the_data_dir() {
        assert_eq!(
            profile_db_path(&TunexConfig::default()),
            data_dir().join("library.db")
        );
        let named = TunexConfig {
            active_profile: "Work Music".to_owned(),
            ..TunexConfig::default()
        };
        assert_eq!(
            profile_db_path(&named),
            data_dir().join("profiles/work-music/library.db")
        );
        assert_eq!(sanitize_profile_id("Work Music"), "work-music");
        assert!(
            data_dir().ends_with("tunex"),
            "data dir carries the app leaf"
        );
        assert!(
            !TunexConfig::default().enrichment.musicbrainz,
            "MusicBrainz is opt-in"
        );
        let dir = scratch_dir("enrich-off");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "volume = 0.5\n").expect("setup works");
        let loaded = load_from(&path).expect("legacy file loads");
        assert!(!loaded.enrichment.musicbrainz);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn window_geometry_clamps_and_defaults() {
        let unset = WindowConfig::default();
        assert_eq!(unset.restored_width(), WindowConfig::DEFAULT_WIDTH);
        assert_eq!(unset.restored_height(), WindowConfig::DEFAULT_HEIGHT);
        let tiny = WindowConfig {
            close_to_tray: true,
            width: 12,
            height: 12,
            x: Some(0),
            y: Some(0),
        };
        assert_eq!(tiny.restored_width(), WindowConfig::MIN_WIDTH);
        assert_eq!(tiny.restored_height(), WindowConfig::MIN_HEIGHT);
        let dir = scratch_dir("geom-clamp");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "[window]\nwidth = 100\nheight = 50\n").expect("setup works");
        let loaded = load_from(&path).expect("legacy-small sizes load");
        assert_eq!(loaded.window.width, WindowConfig::MIN_WIDTH);
        assert_eq!(loaded.window.height, WindowConfig::MIN_HEIGHT);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn view_prefs_round_trip_and_legacy_files_default() {
        let dir = scratch_dir("view-prefs");
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).expect("setup works");
        std::fs::write(&path, "volume = 0.5\n").expect("setup works");
        let loaded = load_from(&path).expect("legacy file loads");
        assert_eq!(loaded.view, ViewConfig::default());
        update(&path, |config| {
            config.view.library_tab = "folders".to_owned();
            config.view.songs_sort = "date".to_owned();
        })
        .expect("patch works");
        let back = load_from(&path).expect("reload works");
        assert_eq!(back.view.library_tab, "folders");
        assert_eq!(back.view.songs_sort, "date");
        assert!(
            crate::sort_dir_is_desc("date", &back.view.songs_sort_dir),
            "legacy files without a dir keep date newest-first"
        );
        assert!(!crate::sort_dir_is_desc("title", ""));
        assert!(crate::sort_dir_is_desc("title", "desc"));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
