//! Lyrics state and resolution (C13): URI-keyed text plus the background
//! resolver behind the shared worker shape.

use std::path::PathBuf;

use cxx_qt_lib::QString;

use super::queue_model::{LyricsWorker, QueueModelRust};
use tunex_player::{PlaybackController, uri_to_path};

fn spawn_lyrics_worker() -> Option<LyricsWorker> {
    crate::worker::Worker::spawn("tunex-lyrics", |(uri, path): (String, PathBuf)| {
        let lyrics = tunex_library::load_lyrics(&path);
        (uri, lyrics)
    })
}

impl QueueModelRust {
    /// Load sidecar or embedded lyrics when the current URI changes.
    /// File I/O and parsing run on the resolver thread (a track change must
    /// never stall the poll); answers carry their URI so a resolution that
    /// lands after another track change is dropped, and a definite
    /// no-lyrics answer is cached exactly like cover art.
    pub(crate) fn maybe_load_lyrics(&mut self) {
        let settled = self
            .lyrics_worker
            .as_mut()
            .map(|worker| worker.drain())
            .unwrap_or_default();
        for (uri, lyrics) in settled {
            if uri == self.lyrics_uri {
                self.lyrics_plain = lyrics.plain;
                self.lyrics_lines = lyrics
                    .lines
                    .into_iter()
                    .map(|line| {
                        (
                            line.time_ms
                                .and_then(|ms| i32::try_from(ms).ok())
                                .unwrap_or(-1),
                            line.text,
                        )
                    })
                    .collect();
            }
        }
        let uri = self
            .controller
            .as_ref()
            .and_then(PlaybackController::current_item)
            .map(|item| item.uri.clone())
            .unwrap_or_default();
        if uri == self.lyrics_uri {
            return;
        }
        self.lyrics_uri.clone_from(&uri);
        self.lyrics_plain.clear();
        self.lyrics_lines.clear();
        let Some(path) = uri_to_path(&uri) else {
            return;
        };
        if self.lyrics_worker.is_none() {
            self.lyrics_worker = spawn_lyrics_worker();
        }
        let alive = self
            .lyrics_worker
            .as_ref()
            .is_some_and(|worker| worker.request((uri, path)));
        if !alive {
            // Resolver died or refused to spawn; drop it so the next poll
            // starts fresh instead of going silent.
            self.lyrics_worker = None;
            self.lyrics_uri.clear();
        }
    }

    /// Unsynced or joined LRC text.
    pub(crate) fn lyrics_plain(&self) -> QString {
        QString::from(self.lyrics_plain.as_str())
    }

    pub(crate) fn lyrics_line_count(&self) -> i32 {
        i32::try_from(self.lyrics_lines.len()).unwrap_or(i32::MAX)
    }

    pub(crate) fn lyrics_line_at(&self, row: i32) -> QString {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.lyrics_lines.get(index))
            .map(|line| QString::from(line.1.as_str()))
            .unwrap_or_default()
    }

    pub(crate) fn lyrics_time_at(&self, row: i32) -> i32 {
        usize::try_from(row)
            .ok()
            .and_then(|index| self.lyrics_lines.get(index))
            .map_or(-1, |line| line.0)
    }

    pub(crate) fn lyrics_active_index(&self, position_ms: i32) -> i32 {
        let position = u64::try_from(position_ms.max(0)).unwrap_or(0);
        let mut active = -1;
        for (index, (time, _)) in self.lyrics_lines.iter().enumerate() {
            if *time >= 0 && u64::try_from(*time).unwrap_or(0) <= position {
                active = i32::try_from(index).unwrap_or(i32::MAX);
            }
        }
        active
    }

    pub(crate) fn lyrics_synced(&self) -> bool {
        self.lyrics_lines.iter().any(|(time, _)| *time >= 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lyrics_worker_resolves_off_thread_with_uri_keys() {
        let dir = std::env::temp_dir().join(format!("tunex-lyrics-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("setup works");
        let audio = dir.join("track.flac");
        std::fs::write(&audio, []).expect("setup works");
        std::fs::write(dir.join("track.lrc"), "[00:01.00]Hello\n").expect("setup works");
        let worker = spawn_lyrics_worker().expect("worker spawns");
        assert!(
            worker.request(("file:///track.flac".to_owned(), audio.clone())),
            "first request starts the worker"
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let settled = loop {
            let settled = worker.drain();
            if !settled.is_empty() || std::time::Instant::now() >= deadline {
                break settled;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(settled.len(), 1, "exactly one answer lands");
        assert_eq!(settled[0].0, "file:///track.flac");
        assert_eq!(settled[0].1.lines[0].text, "Hello");
        assert!(worker.drain().is_empty(), "drain is one-shot");
        std::fs::remove_dir_all(&dir).expect("cleanup works");
    }
}
