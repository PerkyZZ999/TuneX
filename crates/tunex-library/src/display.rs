//! Display labels for indexed tracks.
//!
//! Tags win. When a title (or both title and artist) is missing, the file
//! stem fills the gap so lists do not read as "Unknown Title — Unknown Artist"
//! for ordinary `Artist - Title.mp3` files. Parsed names are never written
//! back as tags (R-NFR-02).

use std::path::Path;

/// Placeholder when no tag and no usable file stem exist.
pub const UNKNOWN_TITLE: &str = "Unknown Title";
/// Placeholder when no tag and no `Artist - Title` stem exist.
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";

/// Title and artist shown in lists, the queue, and Now Playing.
#[must_use]
pub fn display_title_artist(
    path: &str,
    title: Option<&str>,
    artist: Option<&str>,
) -> (String, String) {
    let tagged_title = nonempty(title);
    let tagged_artist = nonempty(artist);
    if tagged_title.is_none() && tagged_artist.is_none() {
        let (name_title, name_artist) = filename_labels(path);
        return (
            name_title.unwrap_or_else(|| UNKNOWN_TITLE.to_owned()),
            name_artist.unwrap_or_else(|| UNKNOWN_ARTIST.to_owned()),
        );
    }
    let title = tagged_title.map_or_else(
        || filename_stem(path).unwrap_or_else(|| UNKNOWN_TITLE.to_owned()),
        str::to_owned,
    );
    let artist = tagged_artist.map_or_else(|| UNKNOWN_ARTIST.to_owned(), str::to_owned);
    (title, artist)
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn filename_stem(path: &str) -> Option<String> {
    let stem = Path::new(path).file_stem()?.to_string_lossy();
    let stem = stem.trim();
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// Split `Artist - Title` only when both tags are missing.
fn filename_labels(path: &str) -> (Option<String>, Option<String>) {
    let Some(stem) = filename_stem(path) else {
        return (None, None);
    };
    if let Some((left, right)) = stem.split_once(" - ") {
        let left = left.trim();
        let right = right.trim();
        if !left.is_empty() && !right.is_empty() {
            return (Some(right.to_owned()), Some(left.to_owned()));
        }
    }
    (Some(stem), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_win_over_the_filename() {
        let (title, artist) = display_title_artist(
            "/music/Wrong - Name.mp3",
            Some("Real Title"),
            Some("Real Artist"),
        );
        assert_eq!(title, "Real Title");
        assert_eq!(artist, "Real Artist");
    }

    #[test]
    fn both_missing_splits_artist_dash_title() {
        let (title, artist) = display_title_artist("/music/Juice WRLD - Both Ways.mp3", None, None);
        assert_eq!(title, "Both Ways");
        assert_eq!(artist, "Juice WRLD");
    }

    #[test]
    fn both_missing_uses_the_stem_when_there_is_no_dash() {
        let (title, artist) = display_title_artist("/music/Another Way.mp3", None, None);
        assert_eq!(title, "Another Way");
        assert_eq!(artist, UNKNOWN_ARTIST);
    }

    #[test]
    fn title_only_keeps_unknown_artist_and_does_not_split() {
        let (title, artist) =
            display_title_artist("/music/Juice WRLD - Both Ways.mp3", Some("Tagged"), None);
        assert_eq!(title, "Tagged");
        assert_eq!(artist, UNKNOWN_ARTIST);
    }

    #[test]
    fn artist_only_uses_the_full_stem_as_title() {
        let (title, artist) =
            display_title_artist("/music/Juice WRLD - Both Ways.mp3", None, Some("Juice"));
        assert_eq!(title, "Juice WRLD - Both Ways");
        assert_eq!(artist, "Juice");
    }

    #[test]
    fn whitespace_tags_count_as_missing() {
        let (title, artist) = display_title_artist("/music/Stem.flac", Some("  "), Some("\t"));
        assert_eq!(title, "Stem");
        assert_eq!(artist, UNKNOWN_ARTIST);
    }
}
