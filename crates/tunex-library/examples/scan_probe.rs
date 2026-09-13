//! S2 gate probe (W-018): scan a large fixture library and report counters.
//!
//! Usage: `scan_probe <library.db> <music-root> [--tag-first N]`
//!
//! The default mode opens (or reopens) the file index and runs one full
//! [`scan_folder`](tunex_library::scan_folder) pass, printing counters plus
//! wall time. `--tag-first N` instead tags the first `N` taggable copies
//! with seeded albums (setup only — never combined with a scan, so tag
//! writes never churn stable keys mid-observation).

use std::path::{Path, PathBuf};
use std::time::Instant;

use tunex_library::{open_file, scan_folder_with_callback};

/// Tag the first `count` taggable copies with seeded albums (6 albums).
///
/// Only Vorbis and ID3 containers are written (FLAC/OGG/Opus/MP3); M4A
/// copies stay untagged rather than pulling in MP4 tag plumbing for a
/// fixture.
fn tag_first(root: &Path, count: usize) {
    use lofty::file::{AudioFile as _, TaggedFileExt as _};
    use lofty::tag::{Accessor as _, Tag, TagType};

    let mut tagged = 0_usize;
    let mut entries: Vec<PathBuf> = tunex_library::collect_media_files(root);
    entries.sort();
    for path in entries {
        if tagged >= count {
            break;
        }
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_lowercase();
        let tag_type = match extension.as_str() {
            "flac" | "ogg" | "oga" | "opus" => TagType::VorbisComments,
            "mp3" => TagType::Id3v2,
            _ => continue,
        };
        let Ok(mut file) = lofty::probe::Probe::open(&path).and_then(lofty::probe::Probe::read)
        else {
            continue;
        };
        let album = tagged / 10 % 6;
        let mut tag = Tag::new(tag_type);
        tag.set_artist(format!("Gate Artist {album}"));
        tag.set_title(format!("Gate Song {tagged}"));
        tag.set_album(format!("Gate Tapes {album}"));
        tag.set_track(u32::try_from(tagged % 10 + 1).unwrap_or(u32::MAX));
        file.insert_tag(tag);
        if file
            .save_to_path(&path, lofty::config::WriteOptions::default())
            .is_ok()
        {
            tagged += 1;
        }
    }
    println!("tagged={tagged}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: scan_probe <library.db> <music-root> [--tag-first N]");
        std::process::exit(2);
    }
    let root = PathBuf::from(&args[2]);
    if args.get(3).is_some_and(|flag| flag == "--tag-first") {
        let count: usize = args
            .get(4)
            .expect("count follows --tag-first")
            .parse()
            .expect("count is a number");
        tag_first(&root, count);
        return;
    }
    let started = Instant::now();
    let mut db = open_file(Path::new(&args[1])).expect("index opens");
    let mut last_files = 0_u64;
    let stats = scan_folder_with_callback(&mut db, &root, |snapshot| {
        // Sparse progress line: first snapshot and every 2000 files after.
        if snapshot.files_seen >= last_files + 2000 {
            last_files = snapshot.files_seen;
            println!(
                "progress files_seen={} tracks_added={}",
                snapshot.files_seen, snapshot.tracks_added
            );
        }
    })
    .expect("scan completes");
    let tracks = tunex_library::list_tracks(&db).expect("list works");
    let artists =
        tunex_library::list_artists(&db, tunex_library::ArtistSort::Name).expect("artists list");
    let albums =
        tunex_library::list_albums(&db, tunex_library::AlbumSort::Title).expect("albums list");
    println!(
        "result files_seen={} tracks_added={} metadata_failed={} renamed={} missing_marked={} rows={} artists={} albums={} elapsed_ms={}",
        stats.files_seen,
        stats.tracks_added,
        stats.metadata_failed,
        stats.renamed,
        stats.missing_marked,
        tracks.len(),
        artists.len(),
        albums.len(),
        started.elapsed().as_millis()
    );
}
