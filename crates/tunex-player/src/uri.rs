//! Filesystem path → `file://` URI conversion for `playbin3`.
//!
//! Kept separate (and dependency-light) so URI edge cases stay unit-tested
//! without spinning up pipelines.

use std::path::Path;

use tunex_core::{Error, Result};

/// Convert a filesystem path to the percent-encoded `file://` URI that
/// `playbin3` expects. Spaces, unicode, and reserved characters are encoded;
/// a bare `format!("file://{}")` would break on all three.
///
/// # Errors
///
/// Returns [`Error::Player`](tunex_core::Error::Player) when the path cannot
/// become a URI.
pub fn path_to_uri(path: &Path) -> Result<String> {
    gstreamer::glib::filename_to_uri(path, None)
        .map(|uri| uri.to_string())
        .map_err(|err| {
            Error::Player(format!(
                "cannot build playback URI for `{}`: {err}",
                path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn plain_path_becomes_file_uri() {
        let uri = path_to_uri(&PathBuf::from("/music/Nova Rae/Still Here.flac"))
            .expect("valid path converts");
        assert_eq!(uri, "file:///music/Nova%20Rae/Still%20Here.flac");
    }

    #[test]
    fn unicode_and_reserved_chars_are_encoded() {
        let uri = path_to_uri(&PathBuf::from("/music/Café & Co [2024]/01 - a#b.ogg"))
            .expect("valid path converts");
        assert!(uri.starts_with("file:///music/"), "keeps file scheme");
        assert!(!uri.contains(' '), "no raw spaces");
        assert!(!uri.contains('#'), "no raw fragment marker");
        assert!(uri.contains("Caf"), "stem survives encoding");
    }
}
