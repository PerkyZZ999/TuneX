//! S1 end-to-end (W-007): scan the committed fixtures into a fresh index,
//! then play the first decodable track to `Playing`.
//!
//! Lives in `tunex-app` deliberately: the library must never depend on the
//! player (D-008) — only the wiring crate may join them, even in tests.

use std::{path::PathBuf, time::Duration};

use tokio::sync::mpsc;
use tunex_core::{PlaybackState, PlayerEvent};
use tunex_player::PlayerEngine;

#[tokio::test]
async fn scan_fixtures_then_play_first() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let db = tunex_library::open_memory().expect("index opens");
    let stats = tunex_library::scan_folder(&db, &fixtures).expect("scan works");
    assert_eq!(
        stats.tracks_added, 7,
        "six tones plus corrupt.mp3, which is scannable but undecodable"
    );
    let tracks = tunex_library::list_tracks(&db).expect("list works");
    let wav = tracks
        .iter()
        .find(|track| track.path.ends_with("sine.wav"))
        .expect("wav fixture is indexed");
    assert!(
        !wav.stable_key.is_empty(),
        "every row carries a stable identity"
    );

    let (events, mut receiver) = mpsc::channel(PlayerEngine::event_buffer());
    let engine = PlayerEngine::new(events).expect("engine builds");
    let sink = gstreamer::ElementFactory::make("fakesink")
        .build()
        .expect("fakesink exists");
    engine.set_audio_sink(&sink);
    engine
        .load_path(&PathBuf::from(&wav.path))
        .expect("fixture loads");
    engine.play().expect("playback starts");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            tokio::time::Instant::now() < deadline,
            "scanned track never reached Playing"
        );
        let event = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .expect("event arrives in time")
            .expect("sender lives as long as the engine");
        match event {
            PlayerEvent::StateChanged(PlaybackState::Playing) => break,
            PlayerEvent::PlaybackError(message) => {
                panic!("indexed fixture must play, got error: {message}");
            }
            _ => {}
        }
    }
    engine.stop().expect("stop works");
}
