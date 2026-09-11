//! Measure search latency against an index (S6 W-041 profile pass).
//!
//! Reports p50/p95/max per query shape over repeated runs, which is the
//! number the aspirational M6 target is written against (search p95 < 50 ms
//! on a 50k-track library, SPEC §perf). This measures the query itself: the
//! ~150 ms debounce in front of it is a deliberate UX delay, not latency.
//!
//! ```text
//! cargo run --release -p tunex-library --example bench_search -- <db> [runs]
//! ```

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tunex_library::{open_file, search_library, search_track_ids};

/// Query shapes a real user produces: one term, two terms, a prefix, and a
/// miss (the shape that scans the most for the least).
const QUERIES: [(&str, &str); 5] = [
    ("single term", "night"),
    ("two terms", "night signals"),
    ("prefix", "nort"),
    ("common word", "the"),
    ("no match", "zzzzqqq"),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(db_path) = args.next().map(PathBuf::from) else {
        eprintln!("usage: bench_search <db-path> [runs]");
        std::process::exit(2);
    };
    let runs: usize = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50);

    // Opening runs any pending migration, so a first open after an upgrade
    // costs more than a steady-state one — both are worth seeing.
    let opened = Instant::now();
    let db = open_file(&db_path).expect("index opens");
    let open_elapsed = opened.elapsed();
    let tracks = tunex_library::list_tracks_capped(&db, u32::MAX).map_or(0, |rows| rows.len());
    println!(
        "index {} — {tracks} tracks, opened in {open_elapsed:?}, {runs} runs per query",
        db_path.display()
    );

    for (label, query) in QUERIES {
        let mut timings = Vec::with_capacity(runs);
        // The FTS5 MATCH + bm25 ranking on its own, so a slow query says
        // whether the cost is the index or the hydration that follows it.
        let mut matching = Vec::with_capacity(runs);
        let mut hits = 0;
        for _ in 0..runs {
            let match_started = Instant::now();
            search_track_ids(&db, query.split_whitespace().next().unwrap_or(query))
                .expect("match runs");
            matching.push(match_started.elapsed());

            let started = Instant::now();
            let results = search_library(&db, query).expect("query runs");
            timings.push(started.elapsed());
            hits = results.tracks.len();
        }
        timings.sort_unstable();
        matching.sort_unstable();
        println!(
            "  {label:<12} \"{query}\" → {hits} tracks · p50 {:?} · p95 {:?} · max {:?} \
             (match-only p50 {:?})",
            percentile(&timings, 50),
            percentile(&timings, 95),
            timings.last().copied().unwrap_or_default(),
            percentile(&matching, 50),
        );
    }
}

/// Nearest-rank percentile over sorted timings.
fn percentile(sorted: &[Duration], percent: usize) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (percent * sorted.len()).div_ceil(100).max(1) - 1;
    sorted[rank.min(sorted.len() - 1)]
}
