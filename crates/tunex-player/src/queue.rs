//! In-memory playback queue: order, cursor, shuffle/repeat, gapless lookahead.
//!
//! The queue owns *intent* (what plays next); [`PlayerEngine`](super::PlayerEngine)
//! owns *execution* (preloading, transitions). All mutation is synchronous and
//! fully unit-tested — no `GStreamer` handle required.

use std::{collections::VecDeque, time::Duration};

use tunex_core::RepeatMode;

/// Position within the current track above which `previous` restarts it
/// instead of stepping back. Matches the de-facto player convention; the
/// caller (which owns position polling) passes the current position in.
const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
/// Cap on remembered history so all-day sessions cannot grow it unboundedly.
/// Oldest entries drop first; only `previous` navigation reads history.
const HISTORY_CAP: usize = 512;

/// One queued entry: playback URI plus library display data.
///
/// Entries from the library carry their row identity and tags (W-021); ad-hoc
/// entries (tests, future URL drops) carry URI + title with the rest `None`.
/// Artwork is intentionally absent: Up Next renders the monogram placeholder
/// until the lazy art pipeline lands (S2 deferral), never a blocking decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueItem {
    /// Playback URI (`file://` for local tracks).
    pub uri: String,
    /// Display title (filename fallback when untagged — never fabricated tags).
    pub title: String,
    /// Library row id, when the item came from the index.
    pub track_id: Option<i64>,
    /// Display artist, when tagged.
    pub artist: Option<String>,
    /// Display album, when tagged.
    pub album: Option<String>,
    /// Tagged duration, when known.
    pub duration: Option<Duration>,
}

impl QueueItem {
    /// Convenience constructor for ad-hoc items and tests.
    #[must_use]
    pub fn new(uri: &str, title: &str) -> Self {
        Self {
            uri: uri.to_owned(),
            title: title.to_owned(),
            track_id: None,
            artist: None,
            album: None,
            duration: None,
        }
    }

    /// Attach the library row id (Up Next identity, missing-state joins).
    #[must_use]
    pub fn with_track_id(mut self, track_id: i64) -> Self {
        self.track_id = Some(track_id);
        self
    }

    /// Attach the display artist.
    #[must_use]
    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    /// Attach the display album.
    #[must_use]
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Attach the tagged duration.
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Resolution of [`Queue::next`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Advance {
    /// Play the item at this index.
    Item(usize),
    /// Queue exhausted (repeat is off and the end was reached).
    End,
}

/// Resolution of [`Queue::previous`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rewind {
    /// Restart the current track (caller seeks to zero, cursor unchanged).
    Restart,
    /// Play the item at this index.
    Item(usize),
    /// Nothing is current; there is nothing to do.
    None,
}

/// Playback queue with shuffle/repeat and gapless lookahead.
///
/// Shuffle uses a hash-chained pick over the eligible set (no RNG dependency
/// by design): uniform over `SipHash` output, never repeats the current item
/// back-to-back while alternatives exist, and — crucially — a pure function
/// of the pick counter, so [`peek_next_uri`](Self::peek_next_uri) previews
/// exactly what [`next`](Self::next) will take.
#[derive(Debug)]
pub struct Queue {
    items: VecDeque<QueueItem>,
    current: Option<usize>,
    history: VecDeque<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    picks: u64,
}

impl Queue {
    /// Empty queue, repeat off, shuffle off.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            current: None,
            history: VecDeque::new(),
            shuffle: false,
            repeat: RepeatMode::Off,
            picks: 0,
        }
    }

    /// Number of queued items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether any items are queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Currently playing item, if the cursor points at one.
    #[must_use]
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.and_then(|index| self.items.get(index))
    }

    /// Cursor position, for Up Next highlight and controller handoffs.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }

    /// Entry by position, for Up Next rows.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&QueueItem> {
        self.items.get(index)
    }

    /// Jump the cursor to an entry (play-now, Up Next direct play). The
    /// previous cursor (if any) lands on history, so `previous` steps back
    /// to it. Returns false when out of range (cursor unchanged).
    pub fn jump(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if let Some(current) = self.current {
            self.push_history(current);
        }
        self.current = Some(index);
        true
    }

    /// Error-aware advance: like [`next`](Self::next), but repeat-one steps
    /// forward instead of replaying — a broken track must not replay itself
    /// forever through the skip path.
    pub fn advance_past_error(&mut self) -> Advance {
        if self.repeat != RepeatMode::One {
            return self.next();
        }
        let saved = std::mem::replace(&mut self.repeat, RepeatMode::Off);
        let outcome = self.next();
        self.repeat = saved;
        outcome
    }

    /// Append to the end of the queue; returns the item index.
    pub fn push_back(&mut self, item: QueueItem) -> usize {
        self.items.push_back(item);
        self.items.len() - 1
    }

    /// Insert directly after the cursor (or at the front with no cursor) so
    /// it plays next; returns the item index.
    pub fn push_next(&mut self, item: QueueItem) -> usize {
        let at = self
            .current
            .map_or(0, |current| current + 1)
            .min(self.items.len());
        self.items.insert(at, item);
        self.shift_cursor_for_insert(at);
        at
    }

    /// Remove an entry, returning it. Removing the cursor clears it (the
    /// loaded track keeps playing; the next advance starts fresh).
    pub fn remove(&mut self, index: usize) -> Option<QueueItem> {
        let removed = self.items.remove(index)?;
        if self.current == Some(index) {
            self.current = None;
        } else if let Some(current) = self.current {
            if index < current {
                self.current = Some(current - 1);
            }
        }
        self.history.retain(|past| *past != index);
        for past in &mut self.history {
            if *past > index {
                *past -= 1;
            }
        }
        Some(removed)
    }

    /// Move an entry, keeping cursor and history pointing at the same tracks.
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from == to || from >= self.items.len() || to >= self.items.len() {
            return;
        }
        // `VecDeque::remove` + `insert` keeps it a move, not a copy.
        if let Some(item) = self.items.remove(from) {
            self.items.insert(to, item);
            self.current = self.current.map(|current| remap_index(current, from, to));
            for past in &mut self.history {
                *past = remap_index(*past, from, to);
            }
        }
    }

    /// Empty the queue and reset cursor, history, and shuffle state.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
        self.history.clear();
        self.picks = 0;
    }

    /// Enable or disable shuffle. History survives toggling (indices still
    /// address the same items).
    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
    }

    /// Whether shuffle is on.
    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Set the repeat mode.
    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Current repeat mode.
    #[must_use]
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Advance the cursor, resolving what plays next. With no cursor this
    /// starts playback (first item, or a shuffled pick).
    ///
    /// Named for the domain (`next`/`previous` mirror MPRIS and SPEC wording)
    /// rather than `Iterator`, whose contract this signature cannot satisfy.
    #[allow(
        clippy::should_implement_trait,
        reason = "domain vocabulary; Advance is not Option<Item>"
    )]
    pub fn next(&mut self) -> Advance {
        if self.items.is_empty() {
            return Advance::End;
        }
        if self.repeat == RepeatMode::One {
            if let Some(current) = self.current {
                return Advance::Item(current);
            }
        }
        let next = match self.current {
            None => Some(self.pick_first()),
            Some(current) => self.pick_after(current),
        };
        // The shuffle pick is a pure function of the counter: consuming it
        // here is what makes consecutive picks differ (peeks never consume).
        if self.shuffle {
            self.consume_pick();
        }
        if let Some(index) = next {
            if let Some(current) = self.current {
                self.push_history(current);
            }
            self.current = Some(index);
            Advance::Item(index)
        } else {
            self.current = None;
            Advance::End
        }
    }

    /// Step back, honoring the restart threshold: past it, the caller restarts
    /// the current track; otherwise the cursor moves to the previously played
    /// item (shuffle-aware via history, linear otherwise).
    pub fn previous(&mut self, position: Duration) -> Rewind {
        let Some(current) = self.current else {
            return Rewind::None;
        };
        if position > RESTART_THRESHOLD {
            return Rewind::Restart;
        }
        // History first: correct under both orderings.
        if let Some(previous) = self.history.pop_back() {
            self.current = Some(previous);
            return Rewind::Item(previous);
        }
        if !self.shuffle && current > 0 {
            self.current = Some(current - 1);
            return Rewind::Item(current - 1);
        }
        Rewind::Restart
    }

    /// Preview what [`next`](Self::next) will take, without mutating cursor,
    /// history, or shuffle state. Feeds `about-to-finish` preloading; repeated
    /// calls are idempotent by construction.
    #[must_use]
    pub fn peek_next_uri(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        if self.repeat == RepeatMode::One {
            if let Some(current) = self.current {
                return self.items.get(current).map(|item| item.uri.clone());
            }
        }
        let index = match self.current {
            None => Some(self.pick_first()),
            Some(current) => self.pick_after(current),
        }?;
        self.items.get(index).map(|item| item.uri.clone())
    }

    /// First pick when nothing is current.
    fn pick_first(&self) -> usize {
        if self.shuffle && self.items.len() > 1 {
            self.shuffled_pick(None)
        } else {
            0
        }
    }

    /// Pick after `current`, or `None` at the linear end with repeat off.
    fn pick_after(&self, current: usize) -> Option<usize> {
        if self.shuffle {
            return Some(self.shuffled_pick(Some(current)));
        }
        let next = current + 1;
        if next < self.items.len() {
            Some(next)
        } else if self.repeat == RepeatMode::All && !self.items.is_empty() {
            Some(0)
        } else {
            None
        }
    }

    /// Uniform pick over eligible indices via a hash chain: no RNG state to
    /// seed or persist, deterministic per `(picks, current)`, and never the
    /// current item back-to-back while alternatives exist.
    fn shuffled_pick(&self, current: Option<usize>) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut eligible: Vec<usize> = (0..self.items.len()).collect();
        if eligible.len() > 1 {
            if let Some(current) = current {
                eligible.retain(|index| *index != current);
            }
        }
        let mut hasher = DefaultHasher::new();
        self.picks.hash(&mut hasher);
        current.hash(&mut hasher);
        let pick = usize::try_from(hasher.finish() % eligible.len() as u64)
            .expect("pick is modulo the eligible length, so it always fits");
        eligible[pick]
    }

    /// Advance the pick counter after a computed pick is consumed, so the
    /// next shuffle differs. Called exactly once per consumed pick.
    fn consume_pick(&mut self) {
        self.picks = self.picks.wrapping_add(1);
    }

    fn push_history(&mut self, index: usize) {
        if self.history.len() >= HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(index);
    }

    /// Fix up one stored index after inserting at `at`.
    fn shift_cursor_for_insert(&mut self, at: usize) {
        if let Some(current) = self.current {
            if at <= current {
                self.current = Some(current + 1);
            }
        }
        for past in &mut self.history {
            if *past >= at {
                *past += 1;
            }
        }
    }
}

/// Remap one stored index across a move of the item at `from` to `to`.
fn remap_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_track_queue() -> Queue {
        let mut queue = Queue::new();
        queue.push_back(QueueItem::new("file:///a.flac", "A"));
        queue.push_back(QueueItem::new("file:///b.flac", "B"));
        queue.push_back(QueueItem::new("file:///c.flac", "C"));
        queue
    }

    #[test]
    fn next_on_empty_queue_ends() {
        let mut queue = Queue::new();
        assert_eq!(queue.next(), Advance::End);
        assert_eq!(queue.current(), None);
    }

    #[test]
    fn next_starts_at_first_item() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("A"));
    }

    #[test]
    fn linear_playback_walks_then_ends() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.next(), Advance::Item(2));
        assert_eq!(queue.next(), Advance::End);
        assert_eq!(queue.current(), None);
    }

    #[test]
    fn repeat_all_wraps_around() {
        let mut queue = three_track_queue();
        queue.set_repeat(RepeatMode::All);
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.next(), Advance::Item(2));
        assert_eq!(queue.next(), Advance::Item(0));
    }

    #[test]
    fn repeat_one_replays_current() {
        let mut queue = three_track_queue();
        queue.set_repeat(RepeatMode::One);
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("A"));
    }

    #[test]
    fn previous_restarts_past_threshold() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.previous(Duration::from_secs(30)), Rewind::Restart);
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("B"));
    }

    #[test]
    fn previous_steps_back_within_threshold() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.previous(Duration::from_secs(1)), Rewind::Item(0));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("A"));
    }

    #[test]
    fn previous_with_no_cursor_is_none() {
        let mut queue = three_track_queue();
        assert_eq!(queue.previous(Duration::ZERO), Rewind::None);
    }

    #[test]
    fn shuffle_covers_all_tracks_without_immediate_repeats() {
        let mut queue = three_track_queue();
        queue.set_shuffle(true);
        let mut seen = std::collections::HashSet::new();
        let mut last = usize::MAX;
        // Shuffled picks never exhaust (repeat only gates linear ends).
        (0..30).for_each(|_| {
            let Advance::Item(index) = queue.next() else {
                panic!("shuffled picks always resolve while items exist");
            };
            assert_ne!(index, last, "no immediate repeats while alternatives exist");
            last = index;
            seen.insert(index);
        });
        assert_eq!(seen.len(), 3, "shuffle must cover every track");
    }

    #[test]
    fn peek_matches_next_and_stays_idempotent() {
        let mut queue = three_track_queue();
        queue.set_shuffle(true);
        let first = queue.peek_next_uri();
        assert_eq!(queue.peek_next_uri(), first, "peeks never mutate");
        let Advance::Item(index) = queue.next() else {
            panic!("non-empty queue always advances");
        };
        assert_eq!(Some(queue.items[index].uri.clone()), first);
    }

    #[test]
    fn remove_before_cursor_shifts_it() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.remove(0).map(|item| item.title), Some("A".to_owned()));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("B"));
    }

    #[test]
    fn remove_current_clears_cursor() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.remove(0).map(|item| item.title), Some("A".to_owned()));
        assert_eq!(queue.current(), None);
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("B"));
    }

    #[test]
    fn move_item_keeps_cursor_on_the_same_track() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(queue.next(), Advance::Item(1));
        queue.move_item(1, 0);
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("B"));
        // Order is now [B, A, C]; linear advance continues after B.
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("A"));
    }

    #[test]
    fn push_next_lands_directly_after_cursor() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        let at = queue.push_next(QueueItem::new("file:///x.flac", "X"));
        assert_eq!(at, 1);
        assert_eq!(queue.next(), Advance::Item(1));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("X"));
    }

    #[test]
    fn clear_resets_everything() {
        let mut queue = three_track_queue();
        queue.set_shuffle(true);
        assert_eq!(queue.next(), Advance::Item(0));
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.current(), None);
        assert_eq!(queue.next(), Advance::End);
    }

    #[test]
    fn enrichment_chainers_attach_library_data() {
        let item = QueueItem::new("file:///a.flac", "A")
            .with_track_id(7)
            .with_artist("Nova Rae")
            .with_album("Night Tapes")
            .with_duration(Duration::from_secs(273));
        assert_eq!(item.track_id, Some(7));
        assert_eq!(item.artist.as_deref(), Some("Nova Rae"));
        assert_eq!(item.album.as_deref(), Some("Night Tapes"));
        assert_eq!(item.duration, Some(Duration::from_secs(273)));
        assert_ne!(
            item,
            QueueItem::new("file:///a.flac", "A"),
            "enrichment participates in identity"
        );
    }

    #[test]
    fn jump_moves_cursor_with_history() {
        let mut queue = three_track_queue();
        assert_eq!(queue.next(), Advance::Item(0));
        assert!(queue.jump(2));
        assert_eq!(queue.current_index(), Some(2));
        assert_eq!(queue.current().map(|item| item.title.as_str()), Some("C"));
        assert!(!queue.jump(99), "out-of-range jumps fail loudly");
        assert_eq!(queue.current_index(), Some(2));
        assert_eq!(
            queue.previous(Duration::ZERO),
            Rewind::Item(0),
            "jumped-from track stays reachable via previous"
        );
    }

    #[test]
    fn advance_past_error_steps_forward_under_repeat_one() {
        let mut queue = three_track_queue();
        queue.set_repeat(RepeatMode::One);
        assert_eq!(queue.next(), Advance::Item(0));
        assert_eq!(
            queue.advance_past_error(),
            Advance::Item(1),
            "broken tracks skip forward instead of replaying"
        );
        assert_eq!(queue.repeat(), RepeatMode::One, "mode preserved");
        assert_eq!(
            queue.next(),
            Advance::Item(1),
            "normal advance still replays"
        );
    }

    #[test]
    fn get_reads_entries_by_position() {
        let queue = three_track_queue();
        assert_eq!(queue.get(1).map(|item| item.title.as_str()), Some("B"));
        assert!(queue.get(99).is_none());
    }
}
