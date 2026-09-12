//! Artwork pipeline: embedded or folder art into the XDG thumbnail cache.
//!
//! Priority is locked (D-010): embedded art first, then `cover.jpg`, then
//! `folder.jpg`, then `front.jpg` (each with `.jpeg`/`.png` siblings). Anything
//! else — oversized, corrupt, undecodable — resolves to `None` and the UI
//! shows its generated placeholder; unknown art stays a placeholder, never a
//! guess (R-005, R-NFR-02).
//!
//! Layout under the cache root (default
//! `$XDG_CACHE_HOME/tunex/art`, `~/.cache/tunex/art` fallback):
//!
//! ```text
//! <cache>/<blake3-hex>/orig.<ext>   exact embedded/folder bytes
//! <cache>/<blake3-hex>/64.jpg       thumbnails (long edge capped)
//! <cache>/<blake3-hex>/256.jpg
//! <cache>/<blake3-hex>/512.jpg
//! <cache>/manifest.txt              `key bytes last_used_unix` per line
//! ```
//!
//! Keys are content hashes, so identical art shares one entry across albums.
//! A manifest tracks sizes and recency; inserts evict least-recently-used
//! entries past [`CACHE_BUDGET_BYTES`]. Everything here is synchronous and
//! cheap to call from worker threads — the scanner stays tag-only and fast,
//! while browse views resolve visible rows lazily in the background (lazy art
//! is a correctness rule: UI-thread decode is forbidden).
//!
//! Decode bombs are guarded three deep: input byte cap, header-dimension cap
//! before pixel allocation, and the `image` crate's own limits.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageFormat, ImageReader};
use tunex_core::{Error, Result};

use crate::metadata::EmbeddedArtwork;

/// Thumbnail long-edge caps served from cache (SPEC §10.1).
pub const THUMB_SIZES: [u32; 3] = [64, 256, 512];

/// Largest single image accepted into the pipeline (image bombs rejected).
pub const MAX_ART_BYTES: u64 = 32 * 1024 * 1024;

/// Largest decodable dimension; headers beyond this never reach the decoder.
pub const MAX_ART_DIMENSION: u32 = 4096;

/// Cache budget before least-recently-used eviction (~2 GiB, SPEC §10.1).
pub const CACHE_BUDGET_BYTES: u64 = 2_000_000_000;

/// Manifest filename inside the cache root.
const MANIFEST_NAME: &str = "manifest.txt";

/// Folder-art basenames in priority order (D-010).
const FOLDER_ART_NAMES: [&str; 3] = ["cover", "folder", "front"];

/// Folder-art extensions accepted per basename.
const FOLDER_ART_EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

/// Cached artwork for one content key: every path is absolute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedArt {
    /// Content key (`blake3` hex of the source bytes).
    pub key: String,
    /// Exact source bytes (extension reflects the sniffed format).
    pub orig: PathBuf,
    /// 64 px thumbnail (long edge capped).
    pub thumb64: PathBuf,
    /// 256 px thumbnail (long edge capped).
    pub thumb256: PathBuf,
    /// 512 px thumbnail (long edge capped).
    pub thumb512: PathBuf,
}

/// Default cache root: `$XDG_CACHE_HOME/tunex/art`, else `~/.cache/tunex/art`.
#[must_use]
pub fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("tunex/art");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/tunex/art");
    }
    std::env::temp_dir().join("tunex/art")
}

/// Content key for artwork bytes.
#[must_use]
pub fn artwork_key(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Highest-priority folder art in `dir`, or `None`.
///
/// Reads the directory once and matches lowercase names against the
/// `cover > folder > front` priority (D-010); nothing else qualifies.
#[must_use]
pub fn find_folder_art(dir: &Path) -> Option<PathBuf> {
    let mut candidates: HashMap<String, PathBuf> = HashMap::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let name = path.file_name()?.to_string_lossy().to_lowercase();
        candidates.entry(name).or_insert(path);
    }
    for base in FOLDER_ART_NAMES {
        for extension in FOLDER_ART_EXTENSIONS {
            if let Some(path) = candidates.get(&format!("{base}.{extension}")) {
                return Some(path.clone());
            }
        }
    }
    None
}

/// Decode `bytes` honoring the pipeline caps, or `None` for placeholder.
///
/// Rejects over-sized inputs and over-dimensioned headers before pixel
/// allocation, then decodes; anything the decoder chokes on also yields
/// `None`. Never errors — undecodable art is routine, not a malfunction.
fn decode_capped(bytes: &[u8]) -> Option<(DynamicImage, ImageFormat)> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_ART_BYTES {
        tracing::debug!(
            name = "art.rejected",
            reason = "byte_cap",
            "artwork too large"
        );
        return None;
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let format = reader.format()?;
    let (width, height) = reader.into_dimensions().ok()?;
    if width > MAX_ART_DIMENSION || height > MAX_ART_DIMENSION {
        tracing::debug!(
            name = "art.rejected",
            reason = "dimensions",
            "artwork too large"
        );
        return None;
    }
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    Some((image, format))
}

/// How far two channel values may differ and still count as the same colour.
/// JPEG ringing means a "solid white" bar is never exactly 255 everywhere.
const BAR_TOLERANCE: u8 = 12;
/// A bar thinner than this percent of the edge is noise, not padding.
const MIN_BAR_PERCENT: u32 = 2;
/// Never keep less than this percent of an edge: an image that is mostly one
/// colour is a design, not a padded picture.
const MIN_KEPT_PERCENT: u32 = 50;

/// Trim uniform letterbox or pillarbox padding, or return the image untouched.
///
/// Cover art ripped from video sources is routinely a 16:9 frame pasted onto
/// a square canvas, so the padding is *in the file* and every surface that
/// draws it 1:1 shows slabs of flat colour. `PreserveAspectCrop` cannot help:
/// the image really is square.
///
/// The rule is deliberately narrow, because this alters how someone's artwork
/// is displayed and a real cover may be framed on purpose:
///
/// - bars must sit on exactly one axis, both sides — a genuine letterbox or
///   pillarbox, never a border on all four edges,
/// - both bars must be the same flat colour within [`BAR_TOLERANCE`],
/// - each bar must be at least [`MIN_BAR_PERCENT`] of that edge, and
/// - at least [`MIN_KEPT_PERCENT`] of the edge must survive.
///
/// Anything that fails a clause is returned unchanged. Squares, gradients,
/// and covers with a painted border are all left alone.
fn trim_uniform_bars(image: &DynamicImage) -> Option<DynamicImage> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    if width < 8 || height < 8 {
        return None;
    }
    let uniform_row =
        |y: u32, colour: [u8; 3]| (0..width).all(|x| near(rgb.get_pixel(x, y).0, colour));
    let uniform_col =
        |x: u32, colour: [u8; 3]| (0..height).all(|y| near(rgb.get_pixel(x, y).0, colour));

    // Horizontal bars (letterbox): the top and bottom edges must agree.
    let top_colour = rgb.get_pixel(0, 0).0;
    let bottom_colour = rgb.get_pixel(0, height - 1).0;
    let horizontal = if near(top_colour, bottom_colour)
        && uniform_row(0, top_colour)
        && uniform_row(height - 1, top_colour)
    {
        let top = (0..height)
            .take_while(|y| uniform_row(*y, top_colour))
            .count();
        let bottom = (0..height)
            .rev()
            .take_while(|y| uniform_row(*y, top_colour))
            .count();
        bar_span(top, bottom, height)
    } else {
        None
    };

    // Vertical bars (pillarbox): the left and right edges must agree.
    let left_colour = rgb.get_pixel(0, 0).0;
    let right_colour = rgb.get_pixel(width - 1, 0).0;
    let vertical = if near(left_colour, right_colour)
        && uniform_col(0, left_colour)
        && uniform_col(width - 1, left_colour)
    {
        let left = (0..width)
            .take_while(|x| uniform_col(*x, left_colour))
            .count();
        let right = (0..width)
            .rev()
            .take_while(|x| uniform_col(*x, left_colour))
            .count();
        bar_span(left, right, width)
    } else {
        None
    };

    // Exactly one axis: bars on both is a frame around the whole picture, and
    // cropping a frame is a judgement call this has no business making.
    match (horizontal, vertical) {
        (Some((top, keep)), None) => Some(image.crop_imm(0, top, width, keep)),
        (None, Some((left, keep))) => Some(image.crop_imm(left, 0, keep, height)),
        _ => None,
    }
}

/// Whether two colours are the same within [`BAR_TOLERANCE`].
fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(left, right)| left.abs_diff(*right) <= BAR_TOLERANCE)
}

/// Validate a leading/trailing bar pair against the size rules, returning the
/// offset and length to keep.
fn bar_span(leading: usize, trailing: usize, edge: u32) -> Option<(u32, u32)> {
    let leading = u32::try_from(leading).ok()?;
    let trailing = u32::try_from(trailing).ok()?;
    // Integer percentages throughout: these are pixel counts, and a float
    // round-trip here buys nothing but precision lints.
    let minimum = (edge * MIN_BAR_PERCENT).div_ceil(100);
    if leading < minimum || trailing < minimum {
        return None;
    }
    let keep = edge.checked_sub(leading)?.checked_sub(trailing)?;
    if keep * 100 < edge * MIN_KEPT_PERCENT {
        return None;
    }
    Some((leading, keep))
}

/// Thumbnail long edge capped at `size`, JPEG-encoded at quality 85.
fn encode_thumb(image: &DynamicImage, size: u32) -> Option<Vec<u8>> {
    let thumb = image.thumbnail(size, size);
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
        .encode_image(&thumb)
        .ok()?;
    Some(encoded)
}

/// Manifest entry: cached size plus recency for eviction.
#[derive(Clone, Copy, Debug, Default)]
struct ManifestEntry {
    /// Total bytes of `orig` plus thumbnails.
    bytes: u64,
    /// Last resolve/store as unix seconds.
    last_used: u64,
}

/// Load the manifest, tolerating a missing or partially corrupt file.
///
/// A corrupt line is skipped, never fatal: worst case the budget accounting
/// restarts and orphaned entries get reaped on the next eviction pass.
fn load_manifest(cache: &Path) -> HashMap<String, ManifestEntry> {
    let mut manifest = HashMap::new();
    let Ok(text) = std::fs::read_to_string(cache.join(MANIFEST_NAME)) else {
        return manifest;
    };
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(key), Some(bytes), Some(last_used)) =
            (fields.next(), fields.next(), fields.next())
        {
            if let (Ok(bytes), Ok(last_used)) = (bytes.parse(), last_used.parse()) {
                manifest.insert(key.to_owned(), ManifestEntry { bytes, last_used });
            }
        }
    }
    manifest
}

/// Persist the manifest atomically (temp file + rename).
///
/// # Errors
///
/// Returns [`Error::Io`] when the cache is unwritable.
fn save_manifest(cache: &Path, manifest: &HashMap<String, ManifestEntry>) -> Result<()> {
    use std::fmt::Write as _;
    let mut text = String::new();
    let mut keys: Vec<&String> = manifest.keys().collect();
    keys.sort();
    for key in keys {
        let entry = &manifest[key];
        let _ = writeln!(text, "{} {} {}", key, entry.bytes, entry.last_used);
    }
    let staging = cache.join(format!("{MANIFEST_NAME}.tmp"));
    std::fs::write(&staging, text).map_err(|source| Error::Io {
        path: staging.clone(),
        source,
    })?;
    std::fs::rename(&staging, cache.join(MANIFEST_NAME)).map_err(|source| Error::Io {
        path: cache.join(MANIFEST_NAME),
        source,
    })?;
    Ok(())
}

/// Current unix time in seconds (0 when the clock misbehaves).
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |age| age.as_secs())
}

/// Evict least-recently-used entries past `budget`, deleting their dirs.
///
/// The `keep` key (the entry just stored) is never evicted by its own
/// write; ties break on the key for determinism.
///
/// # Errors
///
/// Returns [`Error::Io`] when the manifest cannot be persisted.
fn evict_over_budget(
    cache: &Path,
    manifest: &mut HashMap<String, ManifestEntry>,
    budget: u64,
    keep: &str,
) -> Result<()> {
    let mut total: u64 = manifest.values().map(|entry| entry.bytes).sum();
    if total <= budget {
        return Ok(());
    }
    let mut by_recency: Vec<(String, u64)> = manifest
        .iter()
        .map(|(key, entry)| (key.clone(), entry.last_used))
        .collect();
    by_recency.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    for (key, _) in by_recency {
        if total <= budget {
            break;
        }
        if key == keep {
            continue;
        }
        if let Some(entry) = manifest.remove(&key) {
            total = total.saturating_sub(entry.bytes);
            let dir = cache.join(&key);
            if let Err(err) = std::fs::remove_dir_all(&dir) {
                tracing::debug!(
                    name = "art.evict_failed",
                    key = %key,
                    error = %err,
                    "orphaned cache entry"
                );
            }
        }
    }
    save_manifest(cache, manifest)
}

/// Paths for one content key under the cache root.
fn art_paths(cache: &Path, key: &str, extension: &str) -> CachedArt {
    let dir = cache.join(key);
    CachedArt {
        key: key.to_owned(),
        orig: dir.join(format!("orig.{extension}")),
        thumb64: dir.join("64.jpg"),
        thumb256: dir.join("256.jpg"),
        thumb512: dir.join("512.jpg"),
    }
}

/// Touch a manifest entry's recency (best effort; missing keys are ignored).
fn touch_manifest(cache: &Path, key: &str) {
    let mut manifest = load_manifest(cache);
    if let Some(entry) = manifest.get_mut(key) {
        entry.last_used = now_unix();
        let _ = save_manifest(cache, &manifest);
    }
}

/// Cached entry for `key` when every file is present, refreshing recency.
#[must_use]
pub fn cached_art(cache: &Path, key: &str) -> Option<CachedArt> {
    let manifest = load_manifest(cache);
    let entry = manifest.get(key)?;
    let _ = entry;
    // Extension is unknown pre-decode, so accept whatever `orig.*` exists.
    let orig = std::fs::read_dir(cache.join(key))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.file_stem().is_some_and(|stem| stem == "orig"))?;
    let paths = CachedArt {
        key: key.to_owned(),
        orig,
        thumb64: cache.join(key).join("64.jpg"),
        thumb256: cache.join(key).join("256.jpg"),
        thumb512: cache.join(key).join("512.jpg"),
    };
    let complete = paths.thumb64.is_file() && paths.thumb256.is_file() && paths.thumb512.is_file();
    if complete {
        touch_manifest(cache, key);
    }
    complete.then_some(paths)
}

/// Decode `bytes` into the cache under `CACHE_BUDGET_BYTES`.
///
/// Returns `Ok(None)` for placeholder cases (oversized, corrupt,
/// undecodable); only cache-infrastructure failures error.
///
/// # Errors
///
/// Returns [`Error::Io`] when cache directories or files cannot be written.
pub fn store_artwork(cache: &Path, bytes: &[u8]) -> Result<Option<CachedArt>> {
    store_artwork_with_budget(cache, bytes, CACHE_BUDGET_BYTES)
}

/// [`store_artwork`] with an explicit budget (tests shrink it to force
/// eviction; production always uses [`CACHE_BUDGET_BYTES`]).
///
/// # Errors
///
/// Same as [`store_artwork`].
pub fn store_artwork_with_budget(
    cache: &Path,
    bytes: &[u8],
    budget: u64,
) -> Result<Option<CachedArt>> {
    let key = artwork_key(bytes);
    if let Some(hit) = cached_art(cache, &key) {
        return Ok(Some(hit));
    }
    let Some((image, format)) = decode_capped(bytes) else {
        return Ok(None);
    };
    let extension = match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        ImageFormat::Bmp => "bmp",
        _ => "bin",
    };
    let paths = art_paths(cache, &key, extension);
    let dir = cache.join(&key);
    std::fs::create_dir_all(&dir).map_err(|source| Error::Io {
        path: dir.clone(),
        source,
    })?;
    std::fs::write(&paths.orig, bytes).map_err(|source| Error::Io {
        path: paths.orig.clone(),
        source,
    })?;
    // Thumbnails draw the picture; `orig` keeps the file exactly as shipped,
    // so trimming padding here never destroys the original bytes.
    let drawn = trim_uniform_bars(&image).unwrap_or(image);
    for (size, thumb) in [
        (THUMB_SIZES[0], &paths.thumb64),
        (THUMB_SIZES[1], &paths.thumb256),
        (THUMB_SIZES[2], &paths.thumb512),
    ] {
        let Some(encoded) = encode_thumb(&drawn, size) else {
            continue;
        };
        std::fs::write(thumb, encoded).map_err(|source| Error::Io {
            path: thumb.clone(),
            source,
        })?;
    }
    let total: u64 = [
        &paths.orig,
        &paths.thumb64,
        &paths.thumb256,
        &paths.thumb512,
    ]
    .iter()
    .filter_map(|path| std::fs::metadata(path).ok())
    .map(|metadata| metadata.len())
    .sum();
    let mut manifest = load_manifest(cache);
    manifest.insert(
        key.clone(),
        ManifestEntry {
            bytes: total,
            last_used: now_unix(),
        },
    );
    save_manifest(cache, &manifest)?;
    evict_over_budget(cache, &mut manifest, budget, &key)?;
    // Re-read through the hit path so recency is uniform; the fresh entry
    // is keep-protected, so this only misses on I/O failure mid-write.
    Ok(cached_art(cache, &key))
}

/// Resolve artwork for one track: embedded bytes first, then folder art.
///
/// Lazy by design — callers invoke this on background threads for visible
/// rows only, never during scans or on the UI thread. Returns `Ok(None)`
/// when no art exists or nothing qualified (placeholder case).
///
/// # Errors
///
/// Returns [`Error::Io`] when the cache is unwritable or folder art cannot
/// be read.
pub fn resolve_track_art(
    cache: &Path,
    track: &Path,
    embedded: Option<&EmbeddedArtwork>,
) -> Result<Option<CachedArt>> {
    if let Some(art) = embedded {
        if !art.data.is_empty() {
            // Embedded bytes win even when undecodable-looking; only fall
            // through to folder art when they fail the pipeline.
            if let Some(cached) = store_artwork(cache, &art.data)? {
                return Ok(Some(cached));
            }
        }
    }
    if let Some(parent) = track.parent() {
        if let Some(folder) = find_folder_art(parent) {
            let bytes = std::fs::read(&folder).map_err(|source| Error::Io {
                path: folder.clone(),
                source,
            })?;
            if let Some(cached) = store_artwork(cache, &bytes)? {
                return Ok(Some(cached));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scratch cache root, unique per test (parallel-safe).
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tunex-art-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("setup works");
        dir
    }

    /// Solid-color PNG bytes at any dimension (tiny on disk when flat).
    fn png_bytes(width: u32, height: u32, pixel: [u8; 3]) -> Vec<u8> {
        use image::ImageEncoder;
        let image = image::RgbImage::from_pixel(width, height, image::Rgb(pixel));
        let mut encoded = Vec::new();
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(
                image.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgb8,
            )
            .expect("test encodes");
        encoded
    }

    /// A `content` band centred in a `pad`-coloured canvas of the given size.
    fn padded_image(
        width: u32,
        height: u32,
        band_top: u32,
        band_height: u32,
        pad: [u8; 3],
        content: [u8; 3],
    ) -> DynamicImage {
        let mut image = image::RgbImage::from_pixel(width, height, image::Rgb(pad));
        for y in band_top..band_top + band_height {
            for x in 0..width {
                // Vary the content so it is never mistaken for a flat bar.
                let shade = u8::try_from((x + y) % 64).unwrap_or(0);
                image.put_pixel(
                    x,
                    y,
                    image::Rgb([content[0].saturating_add(shade), content[1], content[2]]),
                );
            }
        }
        DynamicImage::ImageRgb8(image)
    }

    #[test]
    fn letterboxed_art_loses_its_bars() {
        // The shape this exists for: a 16:9 frame pasted onto a square.
        let image = padded_image(512, 512, 112, 288, [255, 255, 255], [20, 40, 90]);
        let trimmed = trim_uniform_bars(&image).expect("padding is trimmed");
        assert_eq!(trimmed.width(), 512);
        assert_eq!(trimmed.height(), 288);
    }

    #[test]
    fn pillarboxed_art_loses_its_bars() {
        // The same idea rotated onto the other axis.
        let mut raw = image::RgbImage::from_pixel(512, 512, image::Rgb([8, 8, 8]));
        for x in 96..416 {
            for y in 0..512 {
                let shade = u8::try_from((x + y) % 64).unwrap_or(0);
                raw.put_pixel(x, y, image::Rgb([20u8.saturating_add(shade), 40, 90]));
            }
        }
        let trimmed = trim_uniform_bars(&DynamicImage::ImageRgb8(raw)).expect("padding is trimmed");
        assert_eq!(trimmed.width(), 320);
        assert_eq!(trimmed.height(), 512);
    }

    #[test]
    fn a_framed_cover_keeps_its_border() {
        // Bars on all four edges are a design, not padding: leave it alone.
        let mut raw = image::RgbImage::from_pixel(400, 400, image::Rgb([250, 250, 250]));
        for y in 60..340 {
            for x in 60..340 {
                let shade = u8::try_from((x + y) % 64).unwrap_or(0);
                raw.put_pixel(x, y, image::Rgb([10u8.saturating_add(shade), 30, 70]));
            }
        }
        assert!(trim_uniform_bars(&DynamicImage::ImageRgb8(raw)).is_none());
    }

    #[test]
    fn a_normal_cover_is_never_trimmed() {
        let image = padded_image(400, 400, 0, 400, [0, 0, 0], [90, 30, 30]);
        assert!(trim_uniform_bars(&image).is_none());
    }

    #[test]
    fn a_hairline_edge_is_not_treated_as_padding() {
        // One flat row top and bottom is compression noise, not a letterbox.
        let image = padded_image(400, 400, 1, 398, [255, 255, 255], [20, 40, 90]);
        assert!(trim_uniform_bars(&image).is_none());
    }

    #[test]
    fn a_mostly_blank_image_is_left_alone() {
        // Keeping 10% of the height would be a sliver, not a crop.
        let image = padded_image(400, 400, 180, 40, [255, 255, 255], [20, 40, 90]);
        assert!(trim_uniform_bars(&image).is_none());
    }

    #[test]
    fn cached_thumbnails_use_the_trimmed_picture() {
        let cache = scratch("trim");
        let mut encoded = Vec::new();
        let image = padded_image(512, 512, 112, 288, [255, 255, 255], [20, 40, 90]);
        image
            .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
            .expect("test encodes");
        let art = store_artwork(&cache, &encoded)
            .expect("store works")
            .expect("valid art caches");
        let thumb = image::open(&art.thumb512).expect("thumb decodes");
        assert!(
            thumb.height() < thumb.width(),
            "the square padding is gone from what the UI draws"
        );
        // The original bytes are kept exactly as they shipped.
        let original = image::open(&art.orig).expect("orig decodes");
        assert_eq!(original.width(), 512);
        assert_eq!(original.height(), 512);
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }

    #[test]
    fn png_caches_orig_plus_three_thumbs() {
        let cache = scratch("basic");
        let bytes = png_bytes(600, 400, [200, 40, 40]);
        let art = store_artwork(&cache, &bytes)
            .expect("store works")
            .expect("valid art caches");
        assert_eq!(art.key, artwork_key(&bytes), "keys are content hashes");
        assert!(art.orig.is_file());
        for (thumb, cap) in [
            (&art.thumb64, 64),
            (&art.thumb256, 256),
            (&art.thumb512, 512),
        ] {
            assert!(thumb.is_file());
            let decoded = image::open(thumb).expect("thumb decodes");
            assert!(
                decoded.width() <= cap && decoded.height() <= cap,
                "thumb capped at {cap}px"
            );
        }
        // Second store is a cache hit: same key, same paths.
        let again = store_artwork(&cache, &bytes)
            .expect("store works")
            .expect("hit returns");
        assert_eq!(again, art);
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }

    #[test]
    fn oversized_dimensions_reject_to_placeholder() {
        let cache = scratch("dims");
        // Flat color: kilobytes on disk, megapixels in headers.
        let bytes = png_bytes(MAX_ART_DIMENSION + 1, 8, [10, 10, 10]);
        let stored = store_artwork(&cache, &bytes).expect("store works");
        assert_eq!(stored, None, "over-dimension art must not cache");
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }

    #[test]
    fn corrupt_and_huge_inputs_reject_to_placeholder() {
        let cache = scratch("bad");
        assert_eq!(
            store_artwork(&cache, &[0xFF; 64]).expect("store works"),
            None,
            "undecodable bytes must not cache"
        );
        let over_cap = usize::try_from(MAX_ART_BYTES).expect("test runs 64-bit") + 1;
        assert_eq!(
            store_artwork(&cache, &vec![0u8; over_cap]).expect("store works"),
            None,
            "over-cap bytes must not decode"
        );
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }

    #[test]
    fn folder_art_prefers_cover_over_folder_over_front() {
        let dir = scratch("folder");
        std::fs::write(dir.join("front.png"), png_bytes(8, 8, [1, 1, 1])).expect("setup works");
        std::fs::write(dir.join("folder.png"), png_bytes(8, 8, [2, 2, 2])).expect("setup works");
        let found = find_folder_art(&dir).expect("folder art found");
        assert!(found.ends_with("folder.png"));
        std::fs::write(dir.join("cover.jpg"), png_bytes(8, 8, [3, 3, 3])).expect("setup works");
        let found = find_folder_art(&dir).expect("cover wins");
        assert!(found.ends_with("cover.jpg"));
        assert_eq!(find_folder_art(&dir.join("missing")), None);
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }

    #[test]
    fn resolve_prefers_embedded_then_folder_then_none() {
        let cache = scratch("resolve");
        let music = cache.join("music");
        std::fs::create_dir_all(&music).expect("setup works");
        let folder_png = png_bytes(8, 8, [9, 9, 9]);
        std::fs::write(music.join("cover.png"), &folder_png).expect("setup works");
        let track = music.join("song.flac");

        // Embedded wins over folder art.
        let embedded_png = png_bytes(8, 8, [7, 7, 7]);
        let embedded = EmbeddedArtwork {
            mime: "image/png".to_owned(),
            data: embedded_png.clone(),
        };
        let art = resolve_track_art(&cache, &track, Some(&embedded))
            .expect("resolve works")
            .expect("embedded resolves");
        assert_eq!(art.key, artwork_key(&embedded_png));

        // Without embedded bytes the folder art resolves instead.
        let art = resolve_track_art(&cache, &track, None)
            .expect("resolve works")
            .expect("folder resolves");
        assert_eq!(art.key, artwork_key(&folder_png));

        // Nowhere to look: placeholder.
        let bare = cache.join("bare").join("song.flac");
        assert_eq!(
            resolve_track_art(&cache, &bare, None).expect("resolve works"),
            None
        );
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }

    #[test]
    fn lru_evicts_oldest_under_tiny_budget() {
        let cache = scratch("lru");
        let first = store_artwork_with_budget(&cache, &png_bytes(64, 64, [11, 0, 0]), u64::MAX)
            .expect("store works")
            .expect("first caches");
        let manifest = load_manifest(&cache);
        let first_total = manifest.get(&first.key).expect("manifest tracks").bytes;
        // Budget fits one entry: the next store must evict the first.
        let second = store_artwork_with_budget(&cache, &png_bytes(64, 64, [0, 22, 0]), first_total)
            .expect("store works")
            .expect("second caches");
        assert!(
            !cache.join(&first.key).exists(),
            "least-recently-used entry evicted"
        );
        assert!(cache.join(&second.key).exists());
        assert!(
            cached_art(&cache, &first.key).is_none(),
            "evicted keys miss"
        );
        std::fs::remove_dir_all(&cache).expect("cleanup works");
    }
}
