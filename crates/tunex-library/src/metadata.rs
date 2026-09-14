//! Audio metadata extraction behind a replaceable interface.
//!
//! [`read_metadata`] is the only entry point the scanner uses; `lofty` is an
//! implementation detail confined to this module, so the underlying library
//! can be swapped without touching callers.
//!
//! Honesty rule: every field is optional and empty strings normalize to
//! `None`. Unknown stays unknown — never fabricated, never guessed.

use std::{path::Path, time::Duration};

use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::{Accessor, ItemKey, Tag, TagType},
};
use tunex_core::{Error, Result};

/// Embedded artwork bytes with their MIME type. Decoding, thumbnails, and
/// cache keys belong to the artwork pipeline (W-015); this is transport only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddedArtwork {
    /// MIME type as stored (e.g. `image/jpeg`).
    pub mime: String,
    /// Raw image bytes, untouched.
    pub data: Vec<u8>,
}

/// File metadata with honest unknowns: `None` means untagged or unreadable,
/// never a guess.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileMetadata {
    /// Track title.
    pub title: Option<String>,
    /// Track artist.
    pub artist: Option<String>,
    /// Album title.
    pub album: Option<String>,
    /// Album artist (falls back to nothing, not to `artist`).
    pub album_artist: Option<String>,
    /// Composer.
    pub composer: Option<String>,
    /// Genre.
    pub genre: Option<String>,
    /// Release year.
    pub year: Option<u32>,
    /// Track number within the disc.
    pub track_number: Option<u32>,
    /// Disc number within the release.
    pub disc_number: Option<u32>,
    /// Playable duration from stream properties.
    pub duration: Option<Duration>,
    /// Embedded pictures, front cover preferred.
    pub artwork: Vec<EmbeddedArtwork>,
}

/// Read metadata for one audio file.
///
/// # Errors
///
/// Returns [`Error::Metadata`] when the file cannot be probed or parsed.
/// Callers (scanner) skip such files and continue the run.
pub fn read_metadata(path: &Path) -> Result<FileMetadata> {
    let tagged = Probe::open(path)
        .and_then(lofty::probe::Probe::read)
        .map_err(|err| Error::Metadata(format!("cannot read {}: {err}", path.display())))?;
    let mut metadata = FileMetadata {
        duration: Some(tagged.properties().duration()),
        ..Default::default()
    };
    let Some(tag) = tagged.primary_tag() else {
        return Ok(metadata);
    };
    metadata.title = text(tag.title());
    metadata.artist = text(tag.artist());
    metadata.album = text(tag.album());
    metadata.album_artist = text(tag.get_string(ItemKey::AlbumArtist));
    metadata.composer = text(tag.get_string(ItemKey::Composer));
    metadata.genre = text(tag.genre());
    metadata.year = tag.date().map(|stamp| u32::from(stamp.year));
    metadata.track_number = tag.track().filter(|number| *number > 0);
    metadata.disc_number = tag.disk().filter(|number| *number > 0);
    let mut pictures = tag.pictures().iter();
    if let Some(cover) = pictures
        .clone()
        .find(|picture| picture.pic_type() == lofty::picture::PictureType::CoverFront)
        .or_else(|| pictures.next())
    {
        if !cover.data().is_empty() {
            metadata.artwork.push(EmbeddedArtwork {
                mime: cover
                    .mime_type()
                    .map_or_else(String::new, |mime| mime.as_str().to_owned()),
                data: cover.data().to_vec(),
            });
        }
    }
    Ok(metadata)
}

/// User-authored tag edits. Empty strings are skipped (never invented).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagEdit {
    /// Track title.
    pub title: Option<String>,
    /// Track artist.
    pub artist: Option<String>,
    /// Album title.
    pub album: Option<String>,
    /// Track number.
    pub track_number: Option<u32>,
    /// Disc number.
    pub disc_number: Option<u32>,
    /// Release year.
    pub year: Option<u32>,
    /// Genre.
    pub genre: Option<String>,
    /// Composer.
    pub composer: Option<String>,
}

/// Write non-empty fields with lofty. Empty values are left untouched.
///
/// # Errors
///
/// Returns [`Error::Metadata`] when the file cannot be opened or is not writable.
pub fn write_tags(path: &Path, edit: &TagEdit) -> Result<()> {
    let mut tagged = Probe::open(path)
        .and_then(Probe::read)
        .map_err(|err| Error::Metadata(format!("cannot open {}: {err}", path.display())))?;
    let tag_type = tagged
        .primary_tag()
        .map_or_else(|| tag_type_for(path), Tag::tag_type);
    let mut tag = tagged
        .primary_tag()
        .cloned()
        .unwrap_or_else(|| Tag::new(tag_type));
    if let Some(title) = nonempty_owned(edit.title.as_ref()) {
        tag.set_title(title);
    }
    if let Some(artist) = nonempty_owned(edit.artist.as_ref()) {
        tag.set_artist(artist);
    }
    if let Some(album) = nonempty_owned(edit.album.as_ref()) {
        tag.set_album(album);
    }
    if let Some(genre) = nonempty_owned(edit.genre.as_ref()) {
        tag.set_genre(genre);
    }
    if let Some(composer) = nonempty_owned(edit.composer.as_ref()) {
        tag.insert_text(ItemKey::Composer, composer);
    }
    if let Some(number) = edit.track_number.filter(|number| *number > 0) {
        tag.set_track(number);
    }
    if let Some(number) = edit.disc_number.filter(|number| *number > 0) {
        tag.set_disk(number);
    }
    if let Some(year) = edit.year.filter(|year| *year > 0) {
        let year = u16::try_from(year).unwrap_or(0);
        tag.set_date(lofty::tag::items::Timestamp {
            year,
            ..lofty::tag::items::Timestamp::default()
        });
    }
    tagged.insert_tag(tag);
    tagged
        .save_to_path(path, lofty::config::WriteOptions::default())
        .map_err(|err| Error::Metadata(format!("cannot write {}: {err}", path.display())))?;
    Ok(())
}

fn tag_type_for(path: &Path) -> TagType {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "mp3" => TagType::Id3v2,
        _ => TagType::VorbisComments,
    }
}

fn nonempty_owned(value: Option<&String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Normalize optional tag text: empty or whitespace-only means untagged.
fn text(value: Option<impl Into<String>>) -> Option<String> {
    let text = value.map(Into::into)?;
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Committed untagged fixture: proves unknown stays unknown on real files.
    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(name)
    }

    #[test]
    fn untagged_file_yields_unknowns_with_duration() {
        let metadata = read_metadata(&fixture("sine.flac")).expect("reads");
        assert_eq!(metadata.title, None);
        assert_eq!(metadata.artist, None);
        assert_eq!(metadata.album, None);
        assert_eq!(metadata.album_artist, None);
        assert_eq!(metadata.composer, None);
        assert_eq!(metadata.genre, None);
        assert_eq!(metadata.year, None);
        assert_eq!(metadata.track_number, None);
        assert_eq!(metadata.disc_number, None);
        assert!(metadata.artwork.is_empty());
        let duration = metadata.duration.expect("decodable file has duration");
        assert!(
            (Duration::from_secs(4)..Duration::from_secs(6)).contains(&duration),
            "unexpected duration {duration:?}"
        );
    }

    #[test]
    fn corrupt_file_errors_explicitly() {
        let err = read_metadata(&fixture("corrupt.mp3")).expect_err("must fail loudly");
        assert!(matches!(err, Error::Metadata(_)));
    }

    /// Minimal 8x8 24-bit BMP: hand-encodable (no compression), valid enough
    /// to round-trip through tag storage byte-identical.
    fn red_square_bmp() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&8i32.to_le_bytes());
        bytes.extend_from_slice(&8i32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&24u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&192u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 12]); // x/y resolution + palette fields
        for _ in 0..64 {
            bytes.extend_from_slice(&[0x20, 0x20, 0xE0]);
        }
        // Fix the file size now that pixel data is known.
        let size = u32::try_from(bytes.len()).expect("fixture is tiny");
        bytes[2..6].copy_from_slice(&size.to_le_bytes());
        bytes
    }

    #[test]
    fn tagged_flac_round_trip() {
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::tag::{Tag, TagType};

        let dir = std::env::temp_dir().join(format!("tunex-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        let tagged = dir.join("tagged.flac");
        std::fs::copy(fixture("sine.flac"), &tagged).expect("setup works");

        let image = red_square_bmp();
        let mut file = lofty::probe::Probe::open(&tagged)
            .and_then(lofty::probe::Probe::read)
            .expect("setup reads");
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.set_artist("Nova Rae".to_owned());
        tag.set_title("Midnight".to_owned());
        tag.set_album("Night Tapes".to_owned());
        tag.set_genre("Ambient".to_owned());
        tag.set_track(3);
        tag.set_disk(1);
        tag.push_picture(
            Picture::unchecked(image.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Bmp)
                .build(),
        );
        file.insert_tag(tag);
        file.save_to_path(&tagged, lofty::config::WriteOptions::default())
            .expect("setup writes");

        let metadata = read_metadata(&tagged).expect("reads back");
        assert_eq!(metadata.title.as_deref(), Some("Midnight"));
        assert_eq!(metadata.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(metadata.album.as_deref(), Some("Night Tapes"));
        assert_eq!(metadata.album_artist, None);
        assert_eq!(metadata.genre.as_deref(), Some("Ambient"));
        assert_eq!(metadata.track_number, Some(3));
        assert_eq!(metadata.disc_number, Some(1));
        assert_eq!(metadata.artwork.len(), 1);
        assert_eq!(metadata.artwork[0].mime, "image/bmp");
        assert_eq!(metadata.artwork[0].data, image);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn write_tags_skips_empty_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("tunex-write-tags-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup");
        let tagged = dir.join("edit.flac");
        std::fs::copy(fixture("sine.flac"), &tagged).expect("copy");
        write_tags(
            &tagged,
            &TagEdit {
                title: Some("Midnight".to_owned()),
                artist: Some("Nova Rae".to_owned()),
                album: Some("Night Tapes".to_owned()),
                genre: Some("Ambient".to_owned()),
                composer: Some("A. Composer".to_owned()),
                track_number: Some(3),
                disc_number: Some(1),
                year: Some(2021),
            },
        )
        .expect("writes");
        write_tags(
            &tagged,
            &TagEdit {
                title: Some("  ".to_owned()),
                ..TagEdit::default()
            },
        )
        .expect("empty skipped");
        let metadata = read_metadata(&tagged).expect("reads back");
        assert_eq!(metadata.title.as_deref(), Some("Midnight"));
        assert_eq!(metadata.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(metadata.album.as_deref(), Some("Night Tapes"));
        assert_eq!(metadata.genre.as_deref(), Some("Ambient"));
        assert_eq!(metadata.composer.as_deref(), Some("A. Composer"));
        assert_eq!(metadata.track_number, Some(3));
        assert_eq!(metadata.disc_number, Some(1));
        assert_eq!(metadata.year, Some(2021));
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
