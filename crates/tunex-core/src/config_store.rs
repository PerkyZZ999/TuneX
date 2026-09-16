//! Config persistence (C15): XDG paths, atomic save, cached loads, and
//! profile path mapping. Schema and normalization stay in `config`; this
//! module only moves bytes between disk and the snapshot.

use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::SystemTime,
};

use super::TunexConfig;
use crate::{Error, Result};
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
///
/// Convenience over [`profile_db_path`]: loads the config file on every call
/// (mtime-guarded, so repeated calls within one file state parse once) and
/// substitutes defaults when the file is missing or corrupt — the same
/// degraded-empty story the rest of the app tells. Poll-rate callers should
/// prefer loading once and threading the snapshot through
/// [`profile_db_path`] instead.
#[must_use]
pub fn library_db_path() -> PathBuf {
    profile_db_path(&load_cached(&config_file()))
}

/// Snapshot of one config file, refreshed only when its mtime moves.
#[derive(Clone, Debug, Default)]
struct CachedConfig {
    modified: Option<SystemTime>,
    config: TunexConfig,
}

static CONFIG_CACHE: LazyLock<Mutex<HashMap<PathBuf, CachedConfig>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Load `path`, reusing the cached snapshot while the file is untouched.
/// Corrupt or missing files yield defaults (see [`library_db_path`]).
/// Poll-rate readers prefer this over [`load_from`]: repeated reads within
/// one file state parse once instead of hitting disk per tick. Writers keep
/// using [`load_from`]/[`update`] so read-modify-write always sees fresh
/// state (saves bump the mtime, invalidating the snapshot anyway).
#[must_use]
pub fn load_cached(path: &Path) -> TunexConfig {
    let modified = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok();
    let mut cache = CONFIG_CACHE.lock().unwrap_or_else(|poison| {
        // A poisoned mutex means a previous holder panicked mid-update; the
        // cached snapshot may be stale, so drop it and start clean rather
        // than serving it or deadlocking.
        let mut guard = poison.into_inner();
        guard.clear();
        guard
    });
    if let Some(hit) = cache.get(path)
        && hit.modified == modified
    {
        return hit.config.clone();
    }
    let config = load_from(path).unwrap_or_default();
    cache.insert(
        path.to_path_buf(),
        CachedConfig {
            modified,
            config: config.clone(),
        },
    );
    config
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
/// Leading/trailing hyphens trim; empty results fall back to `"library"`.
/// Distinct ids sharing a 40-char prefix collide by design — profile names
/// are user-chosen labels, so keep them distinguishable in the first 40
/// characters rather than working around it here.
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
/// Writes to a sibling temp file and renames over the target so a crash
/// mid-write cannot leave truncated TOML behind.
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
    let staging = path.with_extension("toml.tmp");
    std::fs::write(&staging, text).map_err(|source| Error::Io {
        path: staging.clone(),
        source,
    })?;
    std::fs::rename(&staging, path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}
