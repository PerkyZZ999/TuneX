//! Demo library builder for S6 GUI verification and profiling.
//!
//! Usage: `demo_library <music-root> [--albums N] [--tracks M]`
//!
//! Writes `<root>/<Artist>/<Album>/<NN> <Title>.<ext>` copies of the
//! committed tone fixtures with real tags (title, artist, album, album
//! artist, track, year, genre) and a deterministic artwork mix per album:
//! embedded cover, folder `cover.jpg`, embedded cover plus a losing
//! `folder.jpg`, no art at all (monogram placeholder), and an undecodable
//! `cover.jpg` (placeholder fallback). Covers are synthetic gradients drawn
//! here — fixture data for the pipeline, never presented as real artwork.

use std::path::{Path, PathBuf};

use image::{ImageEncoder as _, RgbImage};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile as _, TaggedFileExt as _};
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor as _, ItemKey, Tag, TagType};

/// Taggable fixture tones, cycled per track (M4A/WAV stay out: no tags).
const TONES: [&str; 4] = ["sine.flac", "sine.ogg", "sine.opus", "sine.mp3"];

/// Fictional artists; reused with a numeric suffix past the list.
const ARTISTS: [&str; 8] = [
    "Nova Rae",
    "The Lantern Choir",
    "Kite Theory",
    "Mira Vale",
    "Low Orbit",
    "Paper Harbor",
    "Silent Coast",
    "Amber Static",
];

/// Fictional album titles, paired with [`ARTISTS`] by index.
const ALBUMS: [&str; 8] = [
    "Still Here",
    "Night Signals",
    "Glass Harbor",
    "Northern Static",
    "Slow Orbit",
    "Paper Moons",
    "Quiet Machines",
    "Blue Hour",
];

/// Track-title vocabulary.
const WORDS: [&str; 16] = [
    "Midnight",
    "Drift",
    "Echoes",
    "Satellite",
    "Afterglow",
    "Harbor Lights",
    "Low Tide",
    "Neon Rain",
    "Cold Coffee",
    "Starlit",
    "Undertow",
    "Northbound",
    "Glasswork",
    "Parallel",
    "Signal Fire",
    "Lanterns",
];

/// Genres, one per album.
const GENRES: [&str; 6] = [
    "Ambient",
    "Electronic",
    "Indie",
    "Jazz",
    "Synthwave",
    "Lo-Fi",
];

/// Dark/light gradient stops per album cover.
const PALETTES: [([u8; 3], [u8; 3]); 8] = [
    ([10, 18, 52], [70, 110, 220]),
    ([36, 10, 44], [210, 90, 150]),
    ([6, 36, 40], [60, 190, 170]),
    ([44, 26, 8], [230, 160, 70]),
    ([12, 12, 18], [120, 130, 160]),
    ([28, 6, 14], [220, 70, 80]),
    ([8, 28, 18], [110, 200, 110]),
    ([18, 18, 56], [150, 120, 240]),
];

/// How one album carries its artwork (cycled by album index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtMix {
    /// PNG embedded as the front cover in every track.
    Embedded,
    /// `cover.jpg` beside the tracks, nothing embedded.
    Folder,
    /// Embedded JPEG wins over a different `folder.jpg`.
    EmbeddedOverFolder,
    /// No artwork anywhere: the monogram placeholder path.
    Missing,
    /// `cover.jpg` holding undecodable bytes: placeholder fallback.
    Broken,
}

impl ArtMix {
    fn for_album(index: usize) -> Self {
        match index % 8 {
            0 | 4 => Self::Embedded,
            1 | 5 => Self::Folder,
            2 | 6 => Self::EmbeddedOverFolder,
            3 => Self::Missing,
            _ => Self::Broken,
        }
    }
}

/// Clamp and round one colour channel.
fn channel(value: f64) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 first"
    )]
    let byte = value.clamp(0.0, 255.0).round() as u8;
    byte
}

/// Deterministic synthetic cover: diagonal gradient, soft moon, horizon.
fn cover(index: usize, size: u32) -> RgbImage {
    let (dark, light) = PALETTES[index % PALETTES.len()];
    let extent = f64::from(size);
    let step = f64::from(u32::try_from(index % 7).unwrap_or(0));
    let (moon_x, moon_y, moon_r) = (extent * (0.28 + 0.07 * step), extent * 0.34, extent * 0.16);
    RgbImage::from_fn(size, size, |x, y| {
        let (fx, fy) = (f64::from(x), f64::from(y));
        let t = ((fx + fy) / (2.0 * extent)).clamp(0.0, 1.0);
        let distance = ((fx - moon_x).powi(2) + (fy - moon_y).powi(2)).sqrt();
        let glow = (1.0 - (distance - moon_r).max(0.0) / (extent * 0.12)).clamp(0.0, 1.0);
        let disc = if distance <= moon_r { 0.85 } else { glow * 0.3 };
        let horizon = if fy > extent * 0.72 { 0.55 } else { 1.0 };
        let mut rgb = [0_u8; 3];
        for (slot, (low, high)) in rgb.iter_mut().zip(dark.iter().zip(light.iter())) {
            let (low, high) = (f64::from(*low), f64::from(*high));
            let sky = high * (1.0 - t) * 0.55 + low * (0.45 + t * 0.55);
            // The moon takes the palette's light tone, lifted toward white.
            let moon = high * 0.65 + 255.0 * 0.35;
            *slot = channel((sky * (1.0 - disc) + moon * disc) * horizon);
        }
        image::Rgb(rgb)
    })
}

/// PNG bytes for a cover.
fn png(image: &RgbImage) -> Vec<u8> {
    let mut encoded = Vec::new();
    image::codecs::png::PngEncoder::new(&mut encoded)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
        .expect("png encodes");
    encoded
}

/// JPEG bytes for a cover.
fn jpeg(image: &RgbImage) -> Vec<u8> {
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 88)
        .encode_image(image)
        .expect("jpeg encodes");
    encoded
}

/// Tags written into one track copy.
#[derive(Debug)]
struct TrackTags<'a> {
    title: &'a str,
    artist: &'a str,
    album: &'a str,
    genre: &'a str,
    year: u16,
    track: u32,
}

/// Write tags (and optionally an embedded front cover) into `path`.
fn write_tags(path: &Path, tags: &TrackTags<'_>, art: Option<(&[u8], MimeType)>) {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let tag_type = if extension == "mp3" {
        TagType::Id3v2
    } else {
        TagType::VorbisComments
    };
    let mut file = lofty::probe::Probe::open(path)
        .and_then(lofty::probe::Probe::read)
        .expect("fixture copy probes");
    let mut tag = Tag::new(tag_type);
    tag.set_title(tags.title.to_owned());
    tag.set_artist(tags.artist.to_owned());
    tag.set_album(tags.album.to_owned());
    tag.insert_text(ItemKey::AlbumArtist, tags.artist.to_owned());
    tag.set_genre(tags.genre.to_owned());
    tag.set_track(tags.track);
    tag.set_date(Timestamp {
        year: tags.year,
        ..Timestamp::default()
    });
    if let Some((bytes, mime)) = art {
        tag.push_picture(
            Picture::unchecked(bytes.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }
    file.insert_tag(tag);
    file.save_to_path(path, WriteOptions::default())
        .expect("tags save");
}

/// Name with a numeric suffix once the base list is exhausted.
fn cycled(names: &[&str], index: usize) -> String {
    let base = names[index % names.len()];
    if index < names.len() {
        base.to_owned()
    } else {
        format!("{base} {}", index / names.len() + 1)
    }
}

/// Build one album directory with its tracks and artwork mix.
fn build_album(root: &Path, fixtures: &Path, index: usize, tracks: usize) {
    let artist = cycled(&ARTISTS, index);
    let album = cycled(&ALBUMS, index);
    let dir = root.join(&artist).join(&album);
    std::fs::create_dir_all(&dir).expect("album dir creates");
    let mix = ArtMix::for_album(index);
    let art = cover(index, 600);
    let embedded = match mix {
        ArtMix::Embedded => Some((png(&art), MimeType::Png)),
        ArtMix::EmbeddedOverFolder => Some((jpeg(&art), MimeType::Jpeg)),
        _ => None,
    };
    match mix {
        ArtMix::Folder => std::fs::write(dir.join("cover.jpg"), jpeg(&art)),
        // A different image beside embedded art proves embedded wins.
        ArtMix::EmbeddedOverFolder => {
            std::fs::write(dir.join("folder.jpg"), jpeg(&cover(index + 3, 300)))
        }
        ArtMix::Broken => {
            std::fs::write(dir.join("cover.jpg"), b"not an image, placeholder expected")
        }
        ArtMix::Embedded | ArtMix::Missing => Ok(()),
    }
    .expect("folder art writes");
    let year = 1998 + u16::try_from(index % 28).unwrap_or(0);
    for track in 0..tracks {
        let tone = TONES[(index + track) % TONES.len()];
        let extension = tone.rsplit('.').next().unwrap_or("flac");
        let title = cycled(&WORDS, index * tracks + track);
        let number = track + 1;
        let path = dir.join(format!("{number:02} {title}.{extension}"));
        std::fs::copy(fixtures.join(tone), &path).expect("fixture copies");
        let tags = TrackTags {
            title: &title,
            artist: &artist,
            album: &album,
            genre: GENRES[index % GENRES.len()],
            year,
            track: u32::try_from(number).unwrap_or(u32::MAX),
        };
        let art_ref = embedded
            .as_ref()
            .map(|(bytes, mime)| (bytes.as_slice(), mime.clone()));
        write_tags(&path, &tags, art_ref);
    }
}

/// Numeric flag value (`--name N`), if present and valid.
fn flag(args: &[String], name: &str) -> Option<usize> {
    let at = args.iter().position(|arg| arg == name)?;
    args.get(at + 1)?.parse().ok()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(root) = args
        .get(1)
        .filter(|arg| !arg.starts_with("--"))
        .map(PathBuf::from)
    else {
        eprintln!("usage: demo_library <music-root> [--albums N] [--tracks M]");
        std::process::exit(2);
    };
    let albums = flag(&args, "--albums").unwrap_or(8);
    let tracks = flag(&args, "--tracks").unwrap_or(6);
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    for index in 0..albums {
        build_album(&root, &fixtures, index, tracks);
    }
    println!(
        "demo library: {albums} albums x {tracks} tracks in {}",
        root.display()
    );
}
