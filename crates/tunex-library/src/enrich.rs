//! Opt-in `MusicBrainz` / Cover Art Archive lookup (L-009).
//!
//! Default **off**. Network is never required: callers skip this module when
//! the `MusicBrainz` setting is false. Only empty tags and missing artwork are
//! filled — user edits are never overwritten.

use std::fmt::Write;
use std::time::Duration;

use tunex_core::{Error, Result};

/// User-Agent required by `MusicBrainz` (include a contact URL).
pub const USER_AGENT: &str = "TuneX/0.1.0 ( https://github.com/PerkyZZ999/TuneX )";
/// Hard ceiling so a missing network never stalls a scan.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Fields that may be filled from `MusicBrainz`. `None` means leave the file alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Enrichment {
    /// Title when the file had none.
    pub title: Option<String>,
    /// Artist when the file had none.
    pub artist: Option<String>,
    /// Album when the file had none.
    pub album: Option<String>,
    /// Cover Art Archive release id, when local art is missing.
    pub cover_release_id: Option<String>,
}

/// GET used by the live client and by tests.
pub trait HttpGet {
    /// Fetch `url` as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] on timeout, HTTP errors, or a closed network.
    fn get_text(&self, url: &str) -> Result<String>;

    /// Fetch `url` as bytes (Cover Art Archive).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] on timeout, HTTP errors, or a closed network.
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>>;
}

/// Parse a `MusicBrainz` recording-search JSON document.
///
/// # Errors
///
/// Returns [`Error::Config`] when the payload is not an object.
pub fn parse_recording_search(json: &str) -> Result<Enrichment> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|err| Error::Config(err.to_string()))?;
    let Some(recording) = value
        .get("recordings")
        .and_then(serde_json::Value::as_array)
        .and_then(|rows| rows.first())
    else {
        return Ok(Enrichment::default());
    };
    let title = recording
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let artist = recording
        .get("artist-credit")
        .and_then(serde_json::Value::as_array)
        .and_then(|credits| credits.first())
        .and_then(|credit| credit.get("name").or_else(|| credit.get("artist")))
        .and_then(|artist| {
            artist.as_str().map(str::to_owned).or_else(|| {
                artist
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
        });
    let (album, cover_release_id) = recording
        .get("releases")
        .and_then(serde_json::Value::as_array)
        .and_then(|rows| rows.first())
        .map_or((None, None), |release| {
            (
                release
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                release
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            )
        });
    Ok(Enrichment {
        title,
        artist,
        album,
        cover_release_id,
    })
}

/// Keep only fields the file is missing (`has_*` true means already tagged).
#[must_use]
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "four independent presence flags, not a state machine"
)]
pub fn fill_missing(
    found: Enrichment,
    has_title: bool,
    has_artist: bool,
    has_album: bool,
    has_art: bool,
) -> Enrichment {
    Enrichment {
        title: found.title.filter(|_| !has_title),
        artist: found.artist.filter(|_| !has_artist),
        album: found.album.filter(|_| !has_album),
        cover_release_id: found.cover_release_id.filter(|_| !has_art),
    }
}

/// Build the recording-search URL for one title/artist pair.
#[must_use]
pub fn recording_search_url(title: &str, artist: &str) -> String {
    let query = format!("recording:\"{title}\" AND artist:\"{artist}\"");
    format!(
        "https://musicbrainz.org/ws/2/recording/?query={}&fmt=json&limit=1",
        encode_query(&query)
    )
}

fn encode_query(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '&' => out.push_str("%26"),
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '*') => {
                out.push(c);
            }
            c => {
                let mut buf = [0; 4];
                for byte in c.encode_utf8(&mut buf).bytes() {
                    let _ = write!(out, "%{byte:02X}");
                }
            }
        }
    }
    out
}

/// Cover Art Archive front image for one `MusicBrainz` release.
#[must_use]
pub fn cover_art_url(release_id: &str) -> String {
    format!("https://coverartarchive.org/release/{release_id}/front-250")
}

/// Fetch enrichment over `http`. Callers must skip this when the setting is off.
///
/// # Errors
///
/// Returns [`Error::Config`] when the request or JSON parse fails.
pub fn lookup_recording<H: HttpGet>(http: &H, title: &str, artist: &str) -> Result<Enrichment> {
    if title.trim().is_empty() || artist.trim().is_empty() {
        return Ok(Enrichment::default());
    }
    let body = http.get_text(&recording_search_url(title, artist))?;
    parse_recording_search(&body)
}

/// Live `ureq` client with a short timeout. Constructed only when enrichment is on.
#[derive(Debug, Default)]
pub struct UreqClient;

impl HttpGet for UreqClient {
    fn get_text(&self, url: &str) -> Result<String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .user_agent(USER_AGENT)
            .https_only(true)
            .build()
            .into();
        let mut response = agent
            .get(url)
            .call()
            .map_err(|err| Error::Config(format!("musicbrainz: {err}")))?;
        response
            .body_mut()
            .read_to_string()
            .map_err(|err| Error::Config(format!("musicbrainz: {err}")))
    }

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .user_agent(USER_AGENT)
            .https_only(true)
            .build()
            .into();
        let mut response = agent
            .get(url)
            .call()
            .map_err(|err| Error::Config(format!("coverart: {err}")))?;
        response
            .body_mut()
            .read_to_vec()
            .map_err(|err| Error::Config(format!("coverart: {err}")))
    }
}

/// Fill missing album tags and Cover Art Archive images for a few indexed tracks.
/// Rate-limited; errors skip. Never overwrites tags or local art.
///
/// # Errors
///
/// Returns [`Error::Database`] when the index cannot be opened.
pub fn run_missing_pass(db_path: &std::path::Path) -> Result<()> {
    use super::NewTrack;
    use super::artwork::{default_cache_dir, load_remote_art, store_remote_art};
    use super::db::{list_tracks, open_file, upsert_track};
    use super::metadata::{TagEdit, read_metadata, write_tags};
    use super::scan::{file_id, stable_key};

    if !db_path.is_file() {
        return Ok(());
    }
    let mut db = open_file(db_path)?;
    let tracks = list_tracks(&db)?;
    let http = UreqClient;
    let cache = default_cache_dir();
    let mut done = 0;
    for track in tracks {
        if done >= 8 {
            break;
        }
        let Some(title) = track.title.as_deref() else {
            continue;
        };
        let Some(artist) = track.artist.as_deref() else {
            continue;
        };
        let path = std::path::PathBuf::from(&track.path);
        if !path.is_file() {
            continue;
        }
        let needs_album = track.album.is_none();
        let needs_art = !has_local_art(&path) && load_remote_art(&cache, &path).is_none();
        if !needs_album && !needs_art {
            continue;
        }
        let found = match lookup_recording(&http, title, artist) {
            Ok(found) => found,
            Err(err) => {
                tracing::debug!(name = "enrich.lookup_failed", error = %err, "skipping track");
                continue;
            }
        };
        let fill = fill_missing(found, true, true, !needs_album, !needs_art);
        if let Some(album) = fill.album.clone() {
            if write_tags(
                &path,
                &TagEdit {
                    album: Some(album),
                    ..TagEdit::default()
                },
            )
            .is_ok()
            {
                if let Ok(metadata) = read_metadata(&path) {
                    let row = NewTrack {
                        path: track.path.clone(),
                        stable_key: stable_key(&path),
                        file_id: file_id(&path),
                        title: metadata.title,
                        artist: metadata.artist,
                        album: metadata.album,
                        album_artist: metadata.album_artist,
                        composer: metadata.composer,
                        genre: metadata.genre,
                        year: metadata.year.map(i64::from),
                        track_number: metadata.track_number.map(i64::from),
                        disc_number: metadata.disc_number.map(i64::from),
                        duration_ms: metadata.duration.map(|duration| {
                            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
                        }),
                    };
                    let _ = upsert_track(&mut db, &row);
                }
            }
        }
        if let Some(release_id) = fill.cover_release_id.as_deref() {
            if let Ok(bytes) = http.get_bytes(&cover_art_url(release_id)) {
                let _ = store_remote_art(&cache, &path, &bytes);
            }
        }
        done += 1;
        std::thread::sleep(Duration::from_millis(1100));
    }
    Ok(())
}

fn has_local_art(path: &std::path::Path) -> bool {
    use super::artwork::find_folder_art;
    use super::metadata::read_metadata;

    if read_metadata(path).is_ok_and(|meta| meta.artwork.iter().any(|art| !art.data.is_empty())) {
        return true;
    }
    path.parent()
        .is_some_and(|parent| find_folder_art(parent).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RejectHttp;

    impl HttpGet for RejectHttp {
        fn get_text(&self, _url: &str) -> Result<String> {
            Err(Error::Config("network should not run".to_owned()))
        }

        fn get_bytes(&self, _url: &str) -> Result<Vec<u8>> {
            Err(Error::Config("network should not run".to_owned()))
        }
    }

    #[test]
    fn parse_recording_search_reads_first_hit() {
        let json = r#"{
            "recordings": [{
                "title": "Midnight",
                "artist-credit": [{"name": "Nova Rae"}],
                "releases": [{"id": "rel-1", "title": "Night Tapes"}]
            }]
        }"#;
        let found = parse_recording_search(json).expect("parses");
        assert_eq!(found.title.as_deref(), Some("Midnight"));
        assert_eq!(found.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(found.album.as_deref(), Some("Night Tapes"));
        assert_eq!(found.cover_release_id.as_deref(), Some("rel-1"));
    }

    #[test]
    fn fill_missing_never_overwrites_user_tags() {
        let found = Enrichment {
            title: Some("New".to_owned()),
            artist: Some("Other".to_owned()),
            album: Some("Album".to_owned()),
            cover_release_id: Some("rel".to_owned()),
        };
        let kept = fill_missing(found, true, true, false, true);
        assert_eq!(kept.title, None);
        assert_eq!(kept.artist, None);
        assert_eq!(kept.album.as_deref(), Some("Album"));
        assert_eq!(kept.cover_release_id, None);
        let art = fill_missing(
            Enrichment {
                cover_release_id: Some("rel".to_owned()),
                ..Enrichment::default()
            },
            true,
            true,
            true,
            false,
        );
        assert_eq!(art.cover_release_id.as_deref(), Some("rel"));
    }

    #[test]
    fn cover_art_url_uses_the_release_id() {
        assert_eq!(
            cover_art_url("rel-1"),
            "https://coverartarchive.org/release/rel-1/front-250"
        );
    }

    #[test]
    fn lookup_skips_blank_without_http() {
        let found = lookup_recording(&RejectHttp, "", "Nova").expect("blank skip");
        assert_eq!(found, Enrichment::default());
    }
}
