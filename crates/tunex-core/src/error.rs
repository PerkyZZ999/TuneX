//! Crate error type for `tunex-core`.
//!
//! A single canonical error enum crosses every crate boundary, so callers
//! match one stable vocabulary instead of each crate's ad-hoc failures.
//! Variants stay coarse until a subsystem (database, audio, config) earns its
//! own typed payload — see the roadmap slices for when each lands.

use std::{io, path::PathBuf};

/// Errors producible anywhere in `TuneX`.
///
/// `#[non_exhaustive]` keeps matching call sites forward-compatible while the
/// subsystem errors are still being designed slice by slice.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem path could not be read, written, or watched.
    #[error("cannot access `{path}`: {source}")]
    Io {
        /// File or directory that triggered the failure.
        path: PathBuf,
        /// Underlying OS error; inspect via [`std::error::Error::source`].
        #[source]
        source: io::Error,
    },

    /// Configuration could not be loaded or validated.
    #[error("invalid configuration: {0}")]
    Config(String),

    /// Player subsystem failure (typed finer per slice as engine, queue,
    /// and device handling land).
    #[error("playback error: {0}")]
    Player(String),

    /// SQLite/library-index failure. Carries the message only, so this crate
    /// never depends on any database driver (D-008).
    #[error("database error: {0}")]
    Database(String),

    /// Cancellable background work (scan, search) was asked to stop.
    ///
    /// Cancellation is routine control flow, never a malfunction: report it,
    /// do not log it as a failure.
    #[error("operation cancelled")]
    Cancelled,
}

/// Fallible result using the crate error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn io_error_display_names_path_and_cause() {
        let err = Error::Io {
            path: PathBuf::from("/music/gone.flac"),
            source: io::Error::new(io::ErrorKind::NotFound, "entombed"),
        };
        assert_eq!(
            err.to_string(),
            "cannot access `/music/gone.flac`: entombed"
        );
    }

    #[test]
    fn io_error_source_chain_exposes_os_error() {
        let err = Error::Io {
            path: PathBuf::from("/music/locked.flac"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "nope"),
        };
        let source = err.source().expect("`Io` keeps its OS error as source");
        assert!(
            source
                .downcast_ref::<io::Error>()
                .is_some_and(|os| os.kind() == io::ErrorKind::PermissionDenied)
        );
    }

    #[test]
    fn config_error_display_carries_message() {
        let err = Error::Config("music folder is not a directory".to_owned());
        assert_eq!(
            err.to_string(),
            "invalid configuration: music folder is not a directory"
        );
    }

    #[test]
    fn cancelled_error_display_is_stable() {
        assert_eq!(Error::Cancelled.to_string(), "operation cancelled");
    }
}
