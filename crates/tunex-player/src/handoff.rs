//! Gapless handoff protocol (C19): the whole dance in one place.
//!
//! One concept crosses three modules: hand the next URI over before the
//! current track ends, then commit the cursor exactly once. The pieces:
//!
//! - `about-to-finish` (engine, streaming thread) calls the [`provider`],
//!   which peeks — never moves — and hands the URI to the pipeline. A peek
//!   that already vanished locally is declined: the end-of-track advance
//!   skips missing tracks cleanly, while a doomed preload would error the
//!   pipeline mid-handoff.
//! - `uridecodebin3` swapping input is the commit point. The engine records
//!   the swapped URI; [`drain_swap`] moves the cursor to it when — and only
//!   when — it still matches the live peek, so a teardown that raced the
//!   drain cannot move the cursor under the replacement track.
//! - Manual skips, stops, and explicit plays drain first, then act: a skip
//!   during a preload lands on the preloaded track, and a skip after a
//!   confirmed swap steps past it. Teardowns clear the spent record.
//!
//! Lock ordering: the streaming-thread preload path holds the engine lock
//! while acquiring the queue mutex (inside the provider); control paths
//! must never hold both at once, or a future combined critical section
//! inverts the order into an unjoinable streaming-thread deadlock.
//!
//! Swap detection string-matches the internal `urisourcebin` factory name
//! (tested against `GStreamer` 1.28): if a future `GStreamer` renames that
//! element, swaps silently stop reporting — a run of
//! `player.handoff.unsettled` warnings after an upgrade means this broke,
//! and teardowns degrade to the settle wait.

use std::sync::{Arc, Mutex};

use crate::engine::PlayerEngine;
use crate::playback::{local_path_missing, lock_queue};
use crate::queue::Queue;

/// Invoked on the streaming thread when `about-to-finish` fires, so
/// implementations must be fast and non-blocking (a queue peek, never I/O).
/// [`Queue::peek_next_uri`] is the canonical one.
pub type NextUriProvider = std::sync::Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Where the gapless handoff stands, and whether a teardown is holding it.
///
/// The two questions are one state: a teardown must know whether a swap is
/// still in flight (it has to wait for it), and the preload hook must know
/// whether a teardown is running (it must hand over nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handoff {
    /// Nothing handed over; the next `about-to-finish` may hand a URI.
    Idle,
    /// A URI was handed over and `uridecodebin3` has not swapped to it yet.
    InFlight,
    /// A teardown is running: no handoffs until it finishes.
    Blocked,
    /// A teardown is waiting out the swap it found in flight.
    BlockedInFlight,
}

impl Handoff {
    /// Whether a handed-over URI is still waiting to be swapped in.
    #[must_use]
    pub const fn in_flight(self) -> bool {
        matches!(self, Self::InFlight | Self::BlockedInFlight)
    }

    /// Whether a teardown currently forbids new handoffs.
    #[must_use]
    pub const fn blocked(self) -> bool {
        matches!(self, Self::Blocked | Self::BlockedInFlight)
    }

    /// The state a teardown moves into, remembering a swap already in flight.
    #[must_use]
    pub const fn blocking(self) -> Self {
        if self.in_flight() {
            Self::BlockedInFlight
        } else {
            Self::Blocked
        }
    }

    /// The state after the pipeline reports the swap finished.
    #[must_use]
    pub const fn swapped(self) -> Self {
        match self {
            Self::InFlight => Self::Idle,
            Self::BlockedInFlight => Self::Blocked,
            other => other,
        }
    }
}

/// Gapless lookahead: peek-only. The cursor commits when the engine
/// confirms the swap (see [`drain_swap`]), never speculatively here — a
/// manual skip or a failed preload between peek and swap must not
/// double-account the cursor or poison history with unplayed tracks.
/// Nothing is consumed at the true end (peek is `None`).
pub(crate) fn provider(queue: Arc<Mutex<Queue>>) -> NextUriProvider {
    Arc::new(move || {
        let guard = lock_queue(&queue);
        let uri = guard.peek_next_uri()?;
        if local_path_missing(&uri) {
            return None;
        }
        Some(uri)
    })
}

/// Commit a confirmed gapless swap to the queue cursor. The swap is only
/// honored when it matches the current peek (same deterministic pick the
/// provider previewed): a teardown that raced the drain leaves a stale slot
/// behind, and committing that would move the cursor under the replacement
/// track. Committing through `next()` pushes history and consumes the
/// shuffle pick exactly once.
pub(crate) fn drain_swap(engine: &PlayerEngine, queue: &Mutex<Queue>) {
    let Some(swapped) = engine.take_swapped_uri() else {
        return;
    };
    let mut guard = lock_queue(queue);
    if guard.peek_next_uri().as_deref() == Some(swapped.as_str()) {
        guard.next();
    }
}
