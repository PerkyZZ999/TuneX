//! Build a large synthetic index for the M6 profile pass (S6 W-041).
//!
//! The rows are real — same schema, same FTS5 triggers, same upsert path the
//! scanner uses — but no audio is written: at 50k tracks the files would cost
//! gigabytes and measure the filesystem rather than the app. Anything that
//! needs to *play* uses the real fixtures instead.
//!
//! ```text
//! cargo run --release -p tunex-library --example bench_library -- <db> [tracks]
//! ```
//!
//! The index lands at `<db>` (created or extended). Titles, artists, albums
//! and genres are drawn from small word lists, so search has plausible term
//! frequencies instead of one repeated string.

use std::path::PathBuf;
use std::time::Instant;

use tunex_library::{NewTrack, open_file, upsert_track};

/// Tracks per album, so album and artist counts scale with the library.
const TRACKS_PER_ALBUM: usize = 12;
/// Albums per artist.
const ALBUMS_PER_ARTIST: usize = 8;

const ADJECTIVES: [&str; 16] = [
    "Northern", "Quiet", "Glass", "Amber", "Silent", "Paper", "Kite", "Low", "Night", "Hollow",
    "Bright", "Slow", "Velvet", "Iron", "Winter", "Copper",
];
const NOUNS: [&str; 16] = [
    "Signals", "Machines", "Harbor", "Static", "Coast", "Moons", "Theory", "Orbit", "Tapes",
    "Fields", "Lantern", "Drift", "Ember", "Current", "Echo", "Circuit",
];
const GENRES: [&str; 6] = [
    "Ambient",
    "Electronic",
    "Jazz",
    "Post-Rock",
    "Folk",
    "Shoegaze",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(db_path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: bench_library <db-path> [tracks]");
        std::process::exit(2);
    };
    let tracks: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50_000);

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).expect("index directory");
    }
    let mut db = open_file(&db_path).expect("index opens");

    let started = Instant::now();
    for index in 0..tracks {
        upsert_track(&mut db, &synthetic_track(index)).expect("row inserts");
        if index % 5_000 == 4_999 {
            println!("  {} rows in {:?}", index + 1, started.elapsed());
        }
    }
    let elapsed = started.elapsed();

    let albums = tracks.div_ceil(TRACKS_PER_ALBUM);
    let artists = albums.div_ceil(ALBUMS_PER_ARTIST);
    println!(
        "indexed {tracks} tracks ({albums} albums, {artists} artists) into {} in {elapsed:?}",
        db_path.display()
    );
}

/// One synthetic row: unique path and title, album/artist grouping by index.
fn synthetic_track(index: usize) -> NewTrack {
    let album_index = index / TRACKS_PER_ALBUM;
    let artist_index = album_index / ALBUMS_PER_ARTIST;
    let track_number = index % TRACKS_PER_ALBUM + 1;
    let artist = format!(
        "{} {}",
        ADJECTIVES[artist_index % ADJECTIVES.len()],
        NOUNS[(artist_index / ADJECTIVES.len()) % NOUNS.len()]
    );
    let album = format!(
        "{} {} {album_index}",
        ADJECTIVES[(album_index + 3) % ADJECTIVES.len()],
        NOUNS[(album_index + 7) % NOUNS.len()]
    );
    let title = format!(
        "{} {} {index}",
        ADJECTIVES[(index + 5) % ADJECTIVES.len()],
        NOUNS[(index + 11) % NOUNS.len()]
    );
    let path = format!("/bench/{artist}/{album}/{track_number:02} {title}.flac");
    NewTrack {
        stable_key: path.clone(),
        path,
        title: Some(title),
        artist: Some(artist.clone()),
        album: Some(album),
        album_artist: Some(artist),
        composer: None,
        genre: Some(GENRES[album_index % GENRES.len()].to_owned()),
        year: Some(1990 + i64::try_from(album_index % 35).unwrap_or(0)),
        track_number: Some(i64::try_from(track_number).unwrap_or(1)),
        disc_number: Some(1),
        duration_ms: Some(180_000 + i64::try_from(index % 120_000).unwrap_or(0)),
        ..Default::default()
    }
}
