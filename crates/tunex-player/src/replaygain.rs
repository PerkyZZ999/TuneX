//! `ReplayGain` tag read and linear conversion (L-001).
//!
//! Tags come from `lofty` at load and from the `GStreamer` tag tap while
//! decoding. Missing values stay at unity — never a guessed loudness.

use std::path::Path;

use lofty::{file::TaggedFileExt as _, probe::Probe, tag::ItemKey};
use tunex_core::ReplayGainMode;

use crate::uri_to_path;

/// Track and album `ReplayGain` in dB, each optional.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayGainTags {
    /// Per-track gain in dB.
    pub track_db: Option<f64>,
    /// Album gain in dB.
    pub album_db: Option<f64>,
}

/// Parse a `ReplayGain` text field (`-6.50 dB`, `-6.50dB`, or `-6.50`).
#[must_use]
pub fn parse_gain_text(raw: &str) -> Option<f64> {
    let trimmed = raw
        .trim()
        .trim_end_matches("dB")
        .trim_end_matches("db")
        .trim_end_matches("DB")
        .trim();
    trimmed.parse().ok().filter(|value: &f64| value.is_finite())
}

/// Convert a gain in dB to a linear volume multiplier.
#[must_use]
pub fn linear_from_db(db: f64) -> f64 {
    10_f64.powf(db / 20.0)
}

/// Linear multiplier for the current mode. Missing tags are unity (`1.0`).
#[must_use]
pub fn replaygain_multiplier(
    mode: ReplayGainMode,
    track_db: Option<f64>,
    album_db: Option<f64>,
) -> f64 {
    let db = match mode {
        ReplayGainMode::Off => None,
        ReplayGainMode::Track => track_db,
        ReplayGainMode::Album => album_db.or(track_db),
    };
    db.map_or(1.0, linear_from_db)
}

/// Read `ReplayGain` from a local file. Unreadable or untagged files yield empty tags.
#[must_use]
pub fn read_replaygain(path: &Path) -> ReplayGainTags {
    let Ok(tagged) = Probe::open(path).and_then(Probe::read) else {
        return ReplayGainTags::default();
    };
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return ReplayGainTags::default();
    };
    ReplayGainTags {
        track_db: tag
            .get_string(ItemKey::ReplayGainTrackGain)
            .and_then(parse_gain_text),
        album_db: tag
            .get_string(ItemKey::ReplayGainAlbumGain)
            .and_then(parse_gain_text),
    }
}

/// Read `ReplayGain` from a `file://` URI. Non-file URIs yield empty tags.
#[must_use]
pub fn read_replaygain_uri(uri: &str) -> ReplayGainTags {
    uri_to_path(uri).map_or_else(ReplayGainTags::default, |path| read_replaygain(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gain_text_accepts_db_suffix() {
        assert_eq!(parse_gain_text("-6.50 dB"), Some(-6.5));
        assert_eq!(parse_gain_text("3.0dB"), Some(3.0));
        assert_eq!(parse_gain_text("0"), Some(0.0));
        assert_eq!(parse_gain_text("not-a-gain"), None);
    }

    #[test]
    fn missing_tags_stay_at_unity() {
        assert!(
            (replaygain_multiplier(ReplayGainMode::Track, None, None) - 1.0).abs() < f64::EPSILON
        );
        assert!(
            (replaygain_multiplier(ReplayGainMode::Album, None, Some(-6.0)) - linear_from_db(-6.0))
                .abs()
                < f64::EPSILON
        );
        assert!(
            (replaygain_multiplier(ReplayGainMode::Off, Some(-12.0), Some(-6.0)) - 1.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn track_mode_ignores_album_when_track_is_present() {
        let track = replaygain_multiplier(ReplayGainMode::Track, Some(-6.0), Some(-12.0));
        assert!((track - linear_from_db(-6.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn unreadable_path_yields_empty_tags() {
        assert_eq!(
            read_replaygain(Path::new("/no/such/tunex-replaygain.flac")),
            ReplayGainTags::default()
        );
    }
}
