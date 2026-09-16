//! Index reads and sorts (C14): track/artist/album/genre/composer/folder
//! browse projections with their sort dialects, plus FTS id lookup and
//! search hydration. Pure queries over an open connection; mutations live
//! in `db_write`.

use rusqlite::{Connection, Row};

use super::{TrackIdentity, TrackRow, db_error, matched_ids, placeholders};
use tunex_core::Result;
/// Shared track-list projection: resolved display names over the lookup
/// joins. Callers append their own `WHERE` / `ORDER BY`.
const TRACK_LIST_SELECT: &str =
    "SELECT tracks.id AS id, tracks.path AS path, tracks.title AS title,
                    tracks.stable_key AS stable_key,
                    artists.name AS artist, albums.title AS album, genres.name AS genre,
                    tracks.composer AS composer, tracks.year AS year,
                    tracks.track_number AS track_number, tracks.disc_number AS disc_number,
                    tracks.duration_ms AS duration_ms, tracks.missing AS missing
             FROM tracks
             LEFT JOIN artists ON artists.id = tracks.artist_id
             LEFT JOIN albums ON albums.id = tracks.album_id
             LEFT JOIN genres ON genres.id = tracks.genre_id";

/// All indexed tracks ordered by path, with resolved display names.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks(db: &Connection) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!("{TRACK_LIST_SELECT} ORDER BY tracks.path"))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// One artist row for the browse view: name plus collection counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistRow {
    /// Artist name.
    pub name: String,
    /// Albums attributed to this artist.
    pub album_count: i64,
    /// Tracks attributed to this artist.
    pub track_count: i64,
}

/// One album row for the browse view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumRow {
    /// Database row id (drill-down key for [`list_tracks_in_album`]).
    pub id: i64,
    /// Album title.
    pub title: String,
    /// Attributed artist, if any.
    pub artist: Option<String>,
    /// Release year, if known.
    pub year: Option<i64>,
    /// Indexed tracks on this album.
    pub track_count: i64,
    /// A track on this album to read artwork from (`None` for an album with
    /// no indexed tracks). Any track resolves the same folder art, and
    /// embedded covers on an album agree in practice — the grid needs one
    /// source, not all of them.
    pub art_source: Option<String>,
}

/// Songs-tab / folder-track browse order (V1-basic; L-006 advanced filters
/// stay out of scope). Keys match the IA labels the UI shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackSort {
    /// Title A–Z, then path.
    #[default]
    Title,
    /// Artist A–Z, then title.
    Artist,
    /// Album A–Z, then disc/track.
    Album,
    /// Year newest-first; untagged years last.
    Date,
}

impl TrackSort {
    /// Parse a UI sort key; unknown values fall back to [`TrackSort::Title`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "artist" => Self::Artist,
            "album" => Self::Album,
            "date" => Self::Date,
            _ => Self::Title,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Date => "date",
        }
    }
}

/// Albums-tab browse order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlbumSort {
    /// Title A–Z.
    #[default]
    Title,
    /// Artist A–Z, then title.
    Artist,
    /// Year newest-first; untagged years last.
    Date,
}

impl AlbumSort {
    /// Parse a UI sort key; unknown values fall back to [`AlbumSort::Title`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "artist" => Self::Artist,
            "date" => Self::Date,
            _ => Self::Title,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Artist => "artist",
            Self::Date => "date",
        }
    }
}

/// Artists-tab browse order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArtistSort {
    /// Name A–Z.
    #[default]
    Name,
    /// Song count, most first, then name.
    Songs,
}

impl ArtistSort {
    /// Parse a UI sort key; unknown values fall back to [`ArtistSort::Name`].
    #[must_use]
    pub fn from_key(key: &str) -> Self {
        match key {
            "songs" => Self::Songs,
            _ => Self::Name,
        }
    }

    /// Stable UI key for this order.
    #[must_use]
    pub fn as_key(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Songs => "songs",
        }
    }
}

/// Ascending or descending primary key. Tie-breakers stay stable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDir {
    /// A–Z / oldest / fewest.
    #[default]
    Asc,
    /// Z–A / newest / most. Default for date and artist track-count.
    Desc,
}

impl SortDir {
    /// `true` → [`SortDir::Desc`].
    #[must_use]
    pub fn from_descending(descending: bool) -> Self {
        if descending { Self::Desc } else { Self::Asc }
    }

    /// Whether this is descending.
    #[must_use]
    pub fn is_desc(self) -> bool {
        matches!(self, Self::Desc)
    }

    /// Historical default for a songs-tab key (date newest-first).
    #[must_use]
    pub fn for_track(sort: TrackSort) -> Self {
        match sort {
            TrackSort::Date => Self::Desc,
            TrackSort::Title | TrackSort::Artist | TrackSort::Album => Self::Asc,
        }
    }

    /// Historical default for an albums-tab key (date newest-first).
    #[must_use]
    pub fn for_album(sort: AlbumSort) -> Self {
        match sort {
            AlbumSort::Date => Self::Desc,
            AlbumSort::Title | AlbumSort::Artist => Self::Asc,
        }
    }

    /// Historical default for an artists-tab key (track count most-first).
    #[must_use]
    pub fn for_artist(sort: ArtistSort) -> Self {
        match sort {
            ArtistSort::Songs => Self::Desc,
            ArtistSort::Name => Self::Asc,
        }
    }
}

/// One folder that directly contains indexed tracks (browse, not library-root
/// management).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRow {
    /// Absolute directory path (drill-down key for [`list_tracks_in_folder`]).
    pub path: String,
    /// Last path component, for the row title.
    pub name: String,
    /// Indexed tracks whose parent directory is this folder.
    pub track_count: i64,
}

/// Shared album-list projection: attributed artist plus track counts.
/// Callers append their own `WHERE` before [`ALBUM_LIST_TAIL`].
pub(crate) const ALBUM_LIST_SELECT: &str = "SELECT albums.id AS id, albums.title AS title,
                    artists.name AS artist, albums.year AS year,
                    COUNT(tracks.id) AS track_count,
                    MIN(tracks.path) AS art_source
             FROM albums
             LEFT JOIN artists ON artists.id = albums.artist_id
             LEFT JOIN tracks ON tracks.album_id = albums.id";

/// Grouping shared by every album-list query (order is appended per sort).
pub(crate) const ALBUM_LIST_GROUP: &str = "GROUP BY albums.id";

/// Grouping + title order: search hydrates groups then reorders in memory.
const ALBUM_LIST_TAIL: &str = "GROUP BY albums.id ORDER BY albums.title COLLATE NOCASE";

/// Shared artist-list projection: album/track counts per name. Callers append
/// their own `WHERE` before [`ARTIST_LIST_TAIL`].
const ARTIST_LIST_SELECT: &str = "SELECT artists.name AS name,
                    COUNT(DISTINCT tracks.album_id) AS album_count,
                    COUNT(tracks.id) AS track_count
             FROM artists
             LEFT JOIN tracks ON tracks.artist_id = artists.id";

/// Grouping shared by every artist-list query (order is appended per sort).
const ARTIST_LIST_GROUP: &str = "GROUP BY artists.id";

/// Grouping + name order: search hydrates groups then reorders in memory.
const ARTIST_LIST_TAIL: &str = "GROUP BY artists.id ORDER BY artists.name COLLATE NOCASE";

/// Static `ORDER BY` for the songs browse (never interpolates user text).
fn track_order_sql(sort: TrackSort, dir: SortDir) -> &'static str {
    match (sort, dir) {
        (TrackSort::Title, SortDir::Asc) => {
            "ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE ASC, tracks.path"
        }
        (TrackSort::Title, SortDir::Desc) => {
            "ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE DESC, tracks.path"
        }
        (TrackSort::Artist, SortDir::Asc) => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE ASC, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
        (TrackSort::Artist, SortDir::Desc) => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE DESC, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
        (TrackSort::Album, SortDir::Asc) => {
            "ORDER BY COALESCE(albums.title, '') COLLATE NOCASE ASC, COALESCE(tracks.disc_number, 1), COALESCE(tracks.track_number, 1000000), tracks.path"
        }
        (TrackSort::Album, SortDir::Desc) => {
            "ORDER BY COALESCE(albums.title, '') COLLATE NOCASE DESC, COALESCE(tracks.disc_number, 1), COALESCE(tracks.track_number, 1000000), tracks.path"
        }
        (TrackSort::Date, SortDir::Asc) => {
            "ORDER BY COALESCE(tracks.year, 0) ASC, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
        (TrackSort::Date, SortDir::Desc) => {
            "ORDER BY COALESCE(tracks.year, 0) DESC, COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        }
    }
}

/// Static `ORDER BY` for the albums browse (never interpolates user text).
fn album_order_sql(sort: AlbumSort, dir: SortDir) -> &'static str {
    match (sort, dir) {
        (AlbumSort::Title, SortDir::Asc) => "ORDER BY albums.title COLLATE NOCASE ASC",
        (AlbumSort::Title, SortDir::Desc) => "ORDER BY albums.title COLLATE NOCASE DESC",
        (AlbumSort::Artist, SortDir::Asc) => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE ASC, albums.title COLLATE NOCASE"
        }
        (AlbumSort::Artist, SortDir::Desc) => {
            "ORDER BY COALESCE(artists.name, '') COLLATE NOCASE DESC, albums.title COLLATE NOCASE"
        }
        (AlbumSort::Date, SortDir::Asc) => {
            "ORDER BY COALESCE(albums.year, 0) ASC, albums.title COLLATE NOCASE"
        }
        (AlbumSort::Date, SortDir::Desc) => {
            "ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE"
        }
    }
}

/// Static `ORDER BY` for the artists browse (never interpolates user text).
fn artist_order_sql(sort: ArtistSort, dir: SortDir) -> &'static str {
    match (sort, dir) {
        (ArtistSort::Name, SortDir::Asc) => "ORDER BY artists.name COLLATE NOCASE ASC",
        (ArtistSort::Name, SortDir::Desc) => "ORDER BY artists.name COLLATE NOCASE DESC",
        (ArtistSort::Songs, SortDir::Asc) => {
            "ORDER BY track_count ASC, artists.name COLLATE NOCASE"
        }
        (ArtistSort::Songs, SortDir::Desc) => {
            "ORDER BY track_count DESC, artists.name COLLATE NOCASE"
        }
    }
}

/// Every attributed artist with album/track counts, ordered by [`ArtistSort`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_artists(db: &Connection, sort: ArtistSort, dir: SortDir) -> Result<Vec<ArtistRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} {ARTIST_LIST_GROUP} {}",
            artist_order_sql(sort, dir)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(ArtistRow {
                name: row.get("name")?,
                album_count: row.get("album_count")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<ArtistRow>>>()
        .map_err(|err| db_error(&err))
}

/// Every album with artist, year, and track count, ordered by [`AlbumSort`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_albums(db: &Connection, sort: AlbumSort, dir: SortDir) -> Result<Vec<AlbumRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT} {ALBUM_LIST_GROUP} {}",
            album_order_sql(sort, dir)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(AlbumRow {
                id: row.get("id")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                year: row.get("year")?,
                track_count: row.get("track_count")?,
                art_source: row.get("art_source")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

/// One album by row id (landing-page header).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn album_by_id(db: &Connection, album_id: i64) -> Result<Option<AlbumRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT} WHERE albums.id = ?1 {ALBUM_LIST_GROUP}"
        ))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([album_id], album_from_list_row)
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Total duration of an album's indexed tracks, in milliseconds.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn album_duration_ms(db: &Connection, album_id: i64) -> Result<i64> {
    db.query_row(
        "SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE album_id = ?1",
        [album_id],
        |row| row.get(0),
    )
    .map_err(|err| db_error(&err))
}

/// Albums attributed to one artist, optionally excluding one album id
/// (“more by this artist”).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_albums_for_artist(
    db: &Connection,
    artist: &str,
    exclude_id: Option<i64>,
) -> Result<Vec<AlbumRow>> {
    let sql = if exclude_id.is_some() {
        format!(
            "{ALBUM_LIST_SELECT}
             WHERE artists.name = ?1 AND albums.id != ?2
             {ALBUM_LIST_GROUP}
             ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE"
        )
    } else {
        format!(
            "{ALBUM_LIST_SELECT}
             WHERE artists.name = ?1
             {ALBUM_LIST_GROUP}
             ORDER BY COALESCE(albums.year, 0) DESC, albums.title COLLATE NOCASE"
        )
    };
    let mut statement = db.prepare(&sql).map_err(|err| db_error(&err))?;
    let mapped = if let Some(exclude) = exclude_id {
        statement.query_map(rusqlite::params![artist, exclude], album_from_list_row)
    } else {
        statement.query_map(rusqlite::params![artist], album_from_list_row)
    }
    .map_err(|err| db_error(&err))?;
    mapped
        .collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

pub(crate) fn album_from_list_row(row: &Row<'_>) -> rusqlite::Result<AlbumRow> {
    Ok(AlbumRow {
        id: row.get("id")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        year: row.get("year")?,
        track_count: row.get("track_count")?,
        art_source: row.get("art_source")?,
    })
}

/// One artist by name (landing-page header).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn artist_by_name(db: &Connection, name: &str) -> Result<Option<ArtistRow>> {
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} WHERE artists.name = ?1 {ARTIST_LIST_GROUP}"
        ))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([name], |row| {
            Ok(ArtistRow {
                name: row.get("name")?,
                album_count: row.get("album_count")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Display name used when a genre or composer tag is missing.
pub const UNKNOWN_FACET: &str = "Unknown";

/// One genre or composer group for the library rail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetRow {
    /// Display name (`Unknown` when untagged).
    pub name: String,
    /// Indexed tracks in this group.
    pub track_count: i64,
}

/// Every genre with a track count. Untagged rows group as [`UNKNOWN_FACET`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_genres(db: &Connection) -> Result<Vec<FacetRow>> {
    list_facets(
        db,
        "SELECT COALESCE(NULLIF(genres.name, ''), 'Unknown') AS name,
                COUNT(tracks.id) AS track_count
         FROM tracks
         LEFT JOIN genres ON genres.id = tracks.genre_id
         GROUP BY 1
         ORDER BY name COLLATE NOCASE",
    )
}

/// Every composer with a track count. Untagged rows group as [`UNKNOWN_FACET`].
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_composers(db: &Connection) -> Result<Vec<FacetRow>> {
    list_facets(
        db,
        "SELECT CASE WHEN tracks.composer IS NULL OR TRIM(tracks.composer) = ''
                     THEN 'Unknown' ELSE tracks.composer END AS name,
                COUNT(tracks.id) AS track_count
         FROM tracks
         GROUP BY 1
         ORDER BY name COLLATE NOCASE",
    )
}

fn list_facets(db: &Connection, sql: &str) -> Result<Vec<FacetRow>> {
    let mut statement = db.prepare(sql).map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(FacetRow {
                name: row.get("name")?,
                track_count: row.get("track_count")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<FacetRow>>>()
        .map_err(|err| db_error(&err))
}

/// Tracks on one album in disc/track order (untagged numbers sort last).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_in_album(db: &Connection, album_id: i64) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.album_id = ?1
             ORDER BY COALESCE(tracks.disc_number, 1),
                      COALESCE(tracks.track_number, 1000000),
                      tracks.path"
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([album_id], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Tracks by one artist in album order (release year, album title, then
/// disc/track), for artist enqueue. Untagged years sort last.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_artist(db: &Connection, artist: &str) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE artists.name = ?1
             ORDER BY COALESCE(tracks.year, 9999),
                      albums.title COLLATE NOCASE,
                      COALESCE(tracks.disc_number, 1),
                      COALESCE(tracks.track_number, 1000000),
                      tracks.path"
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([artist], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Tracks in one genre group (untagged rows live under [`UNKNOWN_FACET`]).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_genre(db: &Connection, name: &str) -> Result<Vec<TrackRow>> {
    let sql = if name == UNKNOWN_FACET {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.genre_id IS NULL OR genres.name IS NULL OR TRIM(genres.name) = ''
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    } else {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE genres.name = ?1
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    };
    query_named_tracks(db, &sql, name == UNKNOWN_FACET, name)
}

/// Tracks credited to one composer (untagged rows live under [`UNKNOWN_FACET`]).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_for_composer(db: &Connection, name: &str) -> Result<Vec<TrackRow>> {
    let sql = if name == UNKNOWN_FACET {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.composer IS NULL OR TRIM(tracks.composer) = ''
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    } else {
        format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.composer = ?1
             ORDER BY COALESCE(tracks.title, '') COLLATE NOCASE, tracks.path"
        )
    };
    query_named_tracks(db, &sql, name == UNKNOWN_FACET, name)
}

fn query_named_tracks(
    db: &Connection,
    sql: &str,
    untagged: bool,
    name: &str,
) -> Result<Vec<TrackRow>> {
    let mut statement = db.prepare(sql).map_err(|err| db_error(&err))?;
    let rows = if untagged {
        statement.query_map([], TrackRow::from_row)
    } else {
        statement.query_map([name], TrackRow::from_row)
    }
    .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// First `limit` tracks ordered by [`TrackSort`] (songs tab over large
/// libraries stays bounded; full paging stays with search).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_capped(
    db: &Connection,
    limit: u32,
    sort: TrackSort,
    dir: SortDir,
) -> Result<Vec<TrackRow>> {
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT} {} LIMIT ?1",
            track_order_sql(sort, dir)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([limit], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Distinct parent directories of indexed tracks, ordered by display name.
///
/// This is the Folders *browse* list (tracks grouped by the folder they live
/// in). Library-root add/remove stays on [`crate::db::remove_library_root`]
/// / config, not here.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_track_folders(db: &Connection) -> Result<Vec<FolderRow>> {
    let mut statement = db
        .prepare("SELECT path FROM tracks")
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| db_error(&err))?;
    let mut counts = std::collections::BTreeMap::<String, i64>::new();
    for path in rows {
        let path = path.map_err(|err| db_error(&err))?;
        let Some(parent) = std::path::Path::new(&path).parent() else {
            continue;
        };
        let Some(parent) = parent.to_str() else {
            continue;
        };
        if parent.is_empty() {
            continue;
        }
        *counts.entry(parent.to_owned()).or_insert(0) += 1;
    }
    let mut rows: Vec<FolderRow> = counts
        .into_iter()
        .map(|(path, track_count)| FolderRow {
            name: folder_display_name(&path),
            path,
            track_count,
        })
        .collect();
    rows.sort_by_key(|row| (row.name.to_lowercase(), row.path.clone()));
    Ok(rows)
}

/// Tracks whose parent directory is `folder`, ordered by [`TrackSort`] and
/// capped at `limit`. Nested subfolders are listed separately.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn list_tracks_in_folder(
    db: &Connection,
    folder: &str,
    limit: u32,
    sort: TrackSort,
    dir: SortDir,
) -> Result<Vec<TrackRow>> {
    let sep = std::path::MAIN_SEPARATOR;
    let prefix = format!("{folder}{sep}%", folder = like_escaped(folder));
    let nested = format!(
        "{folder}{sep}%{sep}%",
        folder = like_escaped(folder),
        sep = std::path::MAIN_SEPARATOR
    );
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT}
             WHERE tracks.path LIKE ?1 ESCAPE '\\'
               AND tracks.path NOT LIKE ?2 ESCAPE '\\'
             {} LIMIT ?3",
            track_order_sql(sort, dir)
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(rusqlite::params![prefix, nested, limit], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// Last path component for a folder row title.
fn folder_display_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

/// Escape `\`, `%`, and `_` so a `LIKE` prefix matches the path literally.
pub(crate) fn like_escaped(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Every indexed row's reconcile identity, ordered by path.
///
/// The scanner loads this once per run and matches walked files against it:
/// same path is a refresh, same `file_id` under a new path is a rename,
/// anything left over is a deletion candidate (flagged missing, never
/// dropped — playlists and history keep pointing at the row).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_identities(db: &Connection) -> Result<Vec<TrackIdentity>> {
    let mut statement = db
        .prepare(
            "SELECT id, path, file_id, stable_key, missing,
                    (title IS NULL OR trim(title) = '') AS incomplete
             FROM tracks ORDER BY path",
        )
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map([], |row| {
            Ok(TrackIdentity {
                id: row.get("id")?,
                path: row.get("path")?,
                file_id: row.get("file_id")?,
                stable_key: row.get("stable_key")?,
                missing: row.get("missing")?,
                incomplete: row.get("incomplete")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackIdentity>>>()
        .map_err(|err| db_error(&err))
}

/// Prefix search over the FTS5 index, best matches first (BM25).
///
/// Single-term queries only; the S3 search module builds multi-term parsing
/// on top. The term is quoted so filename punctuation (dots, dashes)
/// searches literally instead of erroring; results cap at [`GROUP_LIMIT`]
/// rows per SPEC §11.3.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn search_track_ids(db: &Connection, term: &str) -> Result<Vec<i64>> {
    matched_ids(db, &crate::search::match_query([term]))
}

/// Hydrate track ids to rows with resolved display names. Returned in index
/// order — callers needing rank order reorder in memory.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn tracks_by_ids(db: &Connection, ids: &[i64]) -> Result<Vec<TrackRow>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{TRACK_LIST_SELECT} WHERE tracks.id IN ({})",
            placeholders(ids.len())
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(ids.iter().copied()),
            TrackRow::from_row,
        )
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<TrackRow>>>()
        .map_err(|err| db_error(&err))
}

/// One indexed track by row id (Up Next enqueue from row identity).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_by_id(db: &Connection, id: i64) -> Result<Option<TrackRow>> {
    Ok(tracks_by_ids(db, &[id])?.pop())
}

/// One indexed track by canonical path (session restore).
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub fn track_by_path(db: &Connection, path: &str) -> Result<Option<TrackRow>> {
    let mut statement = db
        .prepare(&format!("{TRACK_LIST_SELECT} WHERE tracks.path = ?1"))
        .map_err(|err| db_error(&err))?;
    let mut rows = statement
        .query_map([path], TrackRow::from_row)
        .map_err(|err| db_error(&err))?;
    rows.next().transpose().map_err(|err| db_error(&err))
}

/// Album rows for the given titles that own at least one of `track_ids`
/// (same-titled albums by different artists all resolve, but only when their
/// own tracks matched — a shared title never drags in an unrelated album).
/// Returned in title order — callers needing rank order reorder in memory.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn albums_for_tracks(
    db: &Connection,
    titles: &[String],
    track_ids: &[i64],
) -> Result<Vec<AlbumRow>> {
    if titles.is_empty() || track_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{ALBUM_LIST_SELECT}
             WHERE albums.title IN ({}) AND albums.id IN (
                SELECT album_id FROM tracks WHERE tracks.id IN ({})
             )
             {ALBUM_LIST_TAIL}",
            placeholders(titles.len()),
            placeholders(track_ids.len())
        ))
        .map_err(|err| db_error(&err))?;
    // Mixed bindings (titles then ids) go through `Value`: the two IN-lists
    // share one positional parameter sequence.
    let params: Vec<rusqlite::types::Value> = titles
        .iter()
        .map(|title| rusqlite::types::Value::Text(title.clone()))
        .chain(track_ids.iter().copied().map(rusqlite::types::Value::from))
        .collect();
    let rows = statement
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(AlbumRow {
                id: row.get("id")?,
                title: row.get("title")?,
                artist: row.get("artist")?,
                year: row.get("year")?,
                track_count: row.get("track_count")?,
                art_source: row.get("art_source")?,
            })
        })
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<AlbumRow>>>()
        .map_err(|err| db_error(&err))
}

/// Artist rows for the given names (names are unique, so at most one row per
/// name). Returned in name order — callers needing rank order reorder.
///
/// # Errors
///
/// Returns [`Error::Database`] when the query fails.
pub(crate) fn artists_by_names(db: &Connection, names: &[String]) -> Result<Vec<ArtistRow>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = db
        .prepare(&format!(
            "{ARTIST_LIST_SELECT} WHERE artists.name IN ({}) {ARTIST_LIST_TAIL}",
            placeholders(names.len())
        ))
        .map_err(|err| db_error(&err))?;
    let rows = statement
        .query_map(
            rusqlite::params_from_iter(names.iter().map(String::as_str)),
            |row| {
                Ok(ArtistRow {
                    name: row.get("name")?,
                    album_count: row.get("album_count")?,
                    track_count: row.get("track_count")?,
                })
            },
        )
        .map_err(|err| db_error(&err))?;
    rows.collect::<rusqlite::Result<Vec<ArtistRow>>>()
        .map_err(|err| db_error(&err))
}
