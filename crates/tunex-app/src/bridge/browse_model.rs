//! Shared browse-model helpers (C10): the lifecycle policy every index-fed
//! list repeats — minus what Qt forces per type.
//!
//! What lives here: opening the index for a load (absent/unreadable means
//! the empty-library state, warned once, never an error surface).
//!
//! What deliberately stays per model: the reset pairing (`begin/end_reset`
//! are inherited per `QObject` type, so no shared function can name them),
//! row shapes, queries, and sorts. The pairing convention — open, rebuild,
//! close on one path with a `SAFETY` note — is load-bearing and stays
//! explicit at each site; centralizing it would trade visible pairing for
//! trait indirection without removing any unsafe code.

use std::path::Path;

/// Open the library index for a browse load.
///
/// Absent files and failed opens yield `None` (the caller returns empty
/// rows): a missing index is the empty-library state, never an error
/// surface. `what` names the browse area for the warn log.
#[must_use]
pub fn open_browse_index(path: &Path, what: &str) -> Option<rusqlite::Connection> {
    if !path.is_file() {
        return None;
    }
    match tunex_library::open_file(path) {
        Ok(db) => Some(db),
        Err(err) => {
            tracing::warn!(name = "browse.load_failed", area = what, error = %err, "index unreadable");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_directory_paths_yield_none() {
        let missing =
            std::env::temp_dir().join(format!("tunex-browse-missing-{}", std::process::id()));
        assert!(open_browse_index(&missing.join("library.db"), "tracks").is_none());
        let dir = std::env::temp_dir().join(format!("tunex-browse-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        assert!(open_browse_index(&dir, "tracks").is_none());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn migrated_index_opens() {
        let dir = std::env::temp_dir().join(format!("tunex-browse-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("library.db");
        tunex_library::open_file(&path).expect("seed opens");
        assert!(open_browse_index(&path, "tracks").is_some());
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
