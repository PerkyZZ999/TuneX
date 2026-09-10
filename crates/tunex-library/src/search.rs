//! Grouped library search over the FTS5 index (SPEC §11.3, R-008).
//!
//! [`search_library`] turns one raw query string into ranked track, album,
//! and artist groups: the input splits into terms on whitespace, every term
//! is quoted (filename punctuation searches literally) with a prefix tail,
//! terms AND together across all seven indexed fields, and the best 200
//! track hits hydrate to [`TrackRow`]s in BM25 rank order while their
//! distinct albums and artists resolve to [`AlbumRow`]/[`ArtistRow`] groups
//! in first-seen order. Empty or whitespace-only input short-circuits to
//! empty results without touching the database.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use tunex_core::Result;

use super::db::{self, AlbumRow, ArtistRow, TrackRow};

/// Ranked search results: up to 200 tracks in BM25 order plus the distinct
/// albums and artists behind them, each in first-seen order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchResults {
    /// Matching tracks, best first.
    pub tracks: Vec<TrackRow>,
    /// Distinct albums behind the matched tracks.
    pub albums: Vec<AlbumRow>,
    /// Distinct artists behind the matched tracks.
    pub artists: Vec<ArtistRow>,
}

/// Ranked track/album/artist groups for one raw query string.
///
/// # Errors
///
/// Returns [`Error::Database`](tunex_core::Error::Database) when any index
/// query fails.
pub fn search_library(db: &Connection, query: &str) -> Result<SearchResults> {
    let terms = parse_query(query);
    if terms.is_empty() {
        return Ok(SearchResults::default());
    }
    let expression = match_query(terms.iter().map(String::as_str));
    let ids = db::matched_ids(db, &expression)?;
    if ids.is_empty() {
        return Ok(SearchResults::default());
    }
    let tracks = tracks_in_rank_order(db, &ids)?;
    let albums = resolve_albums(db, &tracks)?;
    let artists = resolve_artists(db, &tracks)?;
    Ok(SearchResults {
        tracks,
        albums,
        artists,
    })
}

/// Split raw input into search terms on whitespace.
fn parse_query(input: &str) -> Vec<String> {
    input.split_whitespace().map(str::to_owned).collect()
}

/// Join terms into one FTS5 MATCH expression: every term quoted (so dots,
/// dashes, and other filename punctuation match literally instead of
/// erroring) with a prefix tail for as-you-type stems, terms combined with
/// implicit AND — a row must match every term in any of the seven fields.
pub(crate) fn match_query<'a>(terms: impl IntoIterator<Item = &'a str>) -> String {
    terms
        .into_iter()
        .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hydrate matched ids to rows in BM25 rank order: the IN-query returns rows
/// in index order, so reorder in memory (at most 200 rows).
fn tracks_in_rank_order(db: &Connection, ids: &[i64]) -> Result<Vec<TrackRow>> {
    let mut by_id: HashMap<i64, TrackRow> = db::tracks_by_ids(db, ids)?
        .into_iter()
        .map(|row| (row.id, row))
        .collect();
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

/// Distinct values of one optional display field in first-seen (rank) order.
fn distinct_field(tracks: &[TrackRow], field: impl Fn(&TrackRow) -> Option<&str>) -> Vec<String> {
    let mut seen = HashSet::new();
    tracks
        .iter()
        .filter_map(field)
        .filter(|value| seen.insert(*value))
        .map(str::to_owned)
        .collect()
}

/// Album groups for the matched tracks: every distinct title resolves with
/// its counts, same-titled albums included only when their own tracks
/// matched, rows ordered by first-seen title (stable for duplicates).
fn resolve_albums(db: &Connection, tracks: &[TrackRow]) -> Result<Vec<AlbumRow>> {
    let titles = distinct_field(tracks, |row| row.album.as_deref());
    if titles.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i64> = tracks.iter().map(|row| row.id).collect();
    let mut rows = db::albums_for_tracks(db, &titles, &ids)?;
    let rank: HashMap<&str, usize> = titles
        .iter()
        .enumerate()
        .map(|(index, title)| (title.as_str(), index))
        .collect();
    rows.sort_by_key(|row| rank.get(row.title.as_str()).copied().unwrap_or(usize::MAX));
    Ok(rows)
}

/// Artist groups for the matched tracks, ordered by first-seen name.
fn resolve_artists(db: &Connection, tracks: &[TrackRow]) -> Result<Vec<ArtistRow>> {
    let names = distinct_field(tracks, |row| row.artist.as_deref());
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows = db::artists_by_names(db, &names)?;
    let rank: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    rows.sort_by_key(|row| rank.get(row.name.as_str()).copied().unwrap_or(usize::MAX));
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::super::db::{NewTrack, open_memory, upsert_track};
    use super::*;

    fn seed_track(
        db: &mut Connection,
        path: &str,
        title: &str,
        artist: &str,
        album: &str,
        genre: &str,
    ) {
        upsert_track(
            db,
            &NewTrack {
                path: path.to_owned(),
                stable_key: path.to_owned(),
                title: Some(title.to_owned()),
                artist: Some(artist.to_owned()),
                album: Some(album.to_owned()),
                genre: Some(genre.to_owned()),
                ..Default::default()
            },
        )
        .expect("seed works");
    }

    /// Three tracks: two share an artist, two share an album *title* under
    /// different artists (the duplicate-title group case).
    fn seeded_db() -> Connection {
        let mut db = open_memory().expect("in-memory opens");
        seed_track(
            &mut db,
            "/m/midnight.flac",
            "Midnight",
            "Nova Rae",
            "Night Tapes",
            "Ambient",
        );
        seed_track(
            &mut db,
            "/m/solaris.flac",
            "Solaris",
            "Nova Rae",
            "Day Tapes",
            "Rock",
        );
        seed_track(
            &mut db,
            "/m/echoes.flac",
            "Echoes",
            "Distant Shore",
            "Night Tapes",
            "Ambient",
        );
        db
    }

    fn titles(results: &SearchResults) -> Vec<&str> {
        results
            .tracks
            .iter()
            .map(|row| row.title.as_deref().unwrap_or("<unknown>"))
            .collect()
    }

    #[test]
    fn parse_splits_on_whitespace_and_drops_blanks() {
        assert!(parse_query("").is_empty());
        assert!(parse_query("   \t ").is_empty());
        assert_eq!(parse_query("  midnight  nova "), ["midnight", "nova"]);
    }

    #[test]
    fn match_expression_quotes_terms_with_prefix_tails() {
        assert_eq!(match_query(["mid"]), "\"mid\"*");
        assert_eq!(match_query(["a.b", "c-d"]), "\"a.b\"* \"c-d\"*");
        assert_eq!(match_query(["say \"hi\""]), "\"say \"\"hi\"\"\"*");
    }

    #[test]
    fn empty_and_blank_queries_short_circuit() {
        let db = seeded_db();
        for query in ["", "   "] {
            let results = search_library(&db, query).expect("empty queries never fail");
            assert_eq!(results, SearchResults::default());
        }
    }

    #[test]
    fn single_term_matches_title_and_groups_resolve() {
        let db = seeded_db();
        let results = search_library(&db, "midnight").expect("search works");
        assert_eq!(titles(&results), ["Midnight"]);
        assert_eq!(results.albums.len(), 1);
        assert_eq!(results.albums[0].title, "Night Tapes");
        assert_eq!(results.albums[0].artist.as_deref(), Some("Nova Rae"));
        assert_eq!(results.albums[0].track_count, 1);
        assert_eq!(results.artists.len(), 1);
        assert_eq!(results.artists[0].name, "Nova Rae");
    }

    #[test]
    fn artist_term_fans_out_to_tracks_and_albums() {
        let db = seeded_db();
        let results = search_library(&db, "nova").expect("search works");
        assert_eq!(titles(&results).len(), 2);
        assert_eq!(results.artists.len(), 1);
        assert_eq!(results.artists[0].name, "Nova Rae");
        assert_eq!(
            (
                results.artists[0].album_count,
                results.artists[0].track_count
            ),
            (2, 2)
        );
        // Both of Nova's albums resolve; Distant Shore's same-titled album
        // stays out (none of its tracks matched).
        assert_eq!(results.albums.len(), 2);
    }

    #[test]
    fn multi_term_queries_and_across_fields() {
        let db = seeded_db();
        let ambient = search_library(&db, "nova ambient").expect("search works");
        assert_eq!(titles(&ambient), ["Midnight"]);
        let rock = search_library(&db, "nova rock").expect("search works");
        assert_eq!(titles(&rock), ["Solaris"]);
        let none = search_library(&db, "nova polka").expect("search works");
        assert_eq!(none, SearchResults::default());
    }

    #[test]
    fn prefix_stems_match_as_you_type() {
        let db = seeded_db();
        let results = search_library(&db, "mid").expect("search works");
        assert_eq!(titles(&results), ["Midnight"]);
    }

    #[test]
    fn punctuation_searches_literally_without_erroring() {
        let db = seeded_db();
        let results = search_library(&db, "echoes.flac").expect("dots stay literal");
        assert_eq!(titles(&results), ["Echoes"]);
    }

    #[test]
    fn duplicate_album_titles_all_resolve() {
        let db = seeded_db();
        let results = search_library(&db, "tapes").expect("search works");
        assert_eq!(results.tracks.len(), 3);
        let night_tapes = results
            .albums
            .iter()
            .filter(|album| album.title == "Night Tapes")
            .count();
        assert_eq!(
            night_tapes, 2,
            "both same-titled albums resolve, got {:?}",
            results.albums
        );
    }

    #[test]
    fn no_match_yields_empty_groups() {
        let db = seeded_db();
        let results = search_library(&db, "zzz-no-such-thing").expect("search works");
        assert_eq!(results, SearchResults::default());
    }
}
