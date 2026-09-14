//! Local lyrics: sidecar `.lrc` and embedded unsynced tags (L-010).
//!
//! Offline-only. No providers. A sidecar next to the audio file wins; otherwise
//! lofty unsynced lyrics are used. Lines that look like LRC are treated as
//! synced even when they came from a tag.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

/// One lyric line. `time_ms` is set for synced LRC rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LyricLine {
    /// Start time, when the line is synced.
    pub time_ms: Option<u64>,
    /// Display text (empty lines are kept so timing still lines up).
    pub text: String,
}

/// Lyrics for one audio file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lyrics {
    /// Synced or unsynced rows. Empty when nothing was found.
    pub lines: Vec<LyricLine>,
    /// Full unsynced text (also filled for LRC as joined lines).
    pub plain: String,
}

impl Lyrics {
    /// Whether any line carries a timestamp.
    #[must_use]
    pub fn is_synced(&self) -> bool {
        self.lines.iter().any(|line| line.time_ms.is_some())
    }
}

/// Load lyrics for `audio_path`. Missing sidecar/tags yield an empty value.
#[must_use]
pub fn load_lyrics(audio_path: &Path) -> Lyrics {
    let sidecar = audio_path.with_extension("lrc");
    if sidecar.is_file() {
        if let Ok(text) = std::fs::read_to_string(&sidecar) {
            return parse_lrc(&text);
        }
    }
    match embedded_lyrics(audio_path) {
        Some(text) if looks_like_lrc(&text) => parse_lrc(&text),
        Some(text) => Lyrics {
            lines: Vec::new(),
            plain: text,
        },
        None => Lyrics::default(),
    }
}

/// Parse LRC text into timed lines. Metadata tags (`ti`/`ar`/…) are skipped.
#[must_use]
pub fn parse_lrc(text: &str) -> Lyrics {
    let mut offset_ms: i64 = 0;
    let mut lines = Vec::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(offset) = meta_offset(trimmed) {
            offset_ms = offset;
            continue;
        }
        if is_meta_tag(trimmed) {
            continue;
        }
        let (stamps, rest) = split_stamps(trimmed);
        if stamps.is_empty() {
            continue;
        }
        for stamp in stamps {
            let shifted = i64::try_from(stamp).unwrap_or(0) + offset_ms;
            let time_ms = u64::try_from(shifted.max(0)).unwrap_or(0);
            lines.push(LyricLine {
                time_ms: Some(time_ms),
                text: rest.to_owned(),
            });
        }
    }
    lines.sort_by_key(|line| line.time_ms.unwrap_or(0));
    let plain = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Lyrics { lines, plain }
}

/// Index of the last synced line at or before `position_ms` (`-1` when none).
#[must_use]
pub fn active_line(lines: &[LyricLine], position_ms: u64) -> i32 {
    let mut active = -1;
    for (index, line) in lines.iter().enumerate() {
        let Some(time) = line.time_ms else {
            continue;
        };
        if time <= position_ms {
            active = i32::try_from(index).unwrap_or(i32::MAX);
        } else {
            break;
        }
    }
    active
}

fn embedded_lyrics(path: &Path) -> Option<String> {
    let tagged = Probe::open(path).and_then(Probe::read).ok()?;
    let tag = tagged.primary_tag()?;
    let text = tag.get_string(ItemKey::Lyrics)?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn looks_like_lrc(text: &str) -> bool {
    text.lines()
        .any(|line| !split_stamps(line.trim()).0.is_empty())
}

fn is_meta_tag(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("[ti:")
        || lower.starts_with("[ar:")
        || lower.starts_with("[al:")
        || lower.starts_with("[by:")
        || lower.starts_with("[id:")
        || lower.starts_with("[length:")
}

fn meta_offset(line: &str) -> Option<i64> {
    let lower = line.to_ascii_lowercase();
    let rest = lower.strip_prefix("[offset:")?;
    let inner = rest.strip_suffix(']')?;
    inner.trim().parse::<i64>().ok()
}

fn split_stamps(line: &str) -> (Vec<u64>, &str) {
    let mut stamps = Vec::new();
    let mut rest = line;
    while rest.starts_with('[') {
        let Some(end) = rest.find(']') else {
            break;
        };
        let inner = &rest[1..end];
        if let Some(ms) = parse_stamp(inner) {
            stamps.push(ms);
            rest = rest[end + 1..].trim_start();
        } else {
            break;
        }
    }
    (stamps, rest.trim())
}

fn parse_stamp(inner: &str) -> Option<u64> {
    let inner = inner.trim();
    if inner.chars().any(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    let parts: Vec<&str> = inner.split([':', '.']).collect();
    match parts.as_slice() {
        [minutes, seconds] => {
            let minutes: u64 = minutes.parse().ok()?;
            let seconds: u64 = seconds.parse().ok()?;
            Some(minutes.saturating_mul(60_000) + seconds.saturating_mul(1000))
        }
        [minutes, seconds, fraction] => {
            let minutes: u64 = minutes.parse().ok()?;
            let seconds: u64 = seconds.parse().ok()?;
            let frac = if fraction.len() == 2 {
                fraction.parse::<u64>().ok()? * 10
            } else if fraction.len() == 3 {
                fraction.parse().ok()?
            } else {
                return None;
            };
            Some(minutes.saturating_mul(60_000) + seconds.saturating_mul(1000) + frac)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lrc_times_and_skips_meta() {
        let lyrics =
            parse_lrc("[ti:Midnight]\n[ar:Nova]\n[offset:0]\n[00:01.00]First\n[00:02.50]Second\n");
        assert!(lyrics.is_synced());
        assert_eq!(lyrics.lines.len(), 2);
        assert_eq!(lyrics.lines[0].time_ms, Some(1000));
        assert_eq!(lyrics.lines[0].text, "First");
        assert_eq!(lyrics.lines[1].time_ms, Some(2500));
        assert_eq!(active_line(&lyrics.lines, 0), -1);
        assert_eq!(active_line(&lyrics.lines, 1000), 0);
        assert_eq!(active_line(&lyrics.lines, 2600), 1);
    }

    #[test]
    fn sidecar_wins_over_missing_tags() {
        let dir = std::env::temp_dir().join(format!("tunex-lrc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup");
        let audio = dir.join("track.flac");
        std::fs::write(&audio, []).expect("audio");
        std::fs::write(dir.join("track.lrc"), "[00:00.00]Hello\n").expect("lrc");
        let lyrics = load_lyrics(&audio);
        assert_eq!(lyrics.lines[0].text, "Hello");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
