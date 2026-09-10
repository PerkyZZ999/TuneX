//! `tunex-app`: application wiring for `TuneX`.
//!
//! The only crate allowed to touch Qt (via `cxx-qt`, pinned): bridged
//! `QObject`s, settings/XDG handling, the debounced `notify` watcher,
//! MPRIS/D-Bus, and the fan-out of worker `AppEvent`s onto the Qt thread as
//! queued signals land here slice by slice.
//!
//! Layout rule (load-bearing): bridge modules live in this library, and the
//! `tunex` binary calls [`run`], so every target (binary, lib tests,
//! integration tests) links the generated C++ together with its Rust
//! implementations. A binary that references nothing from here silently drops
//! the whole Qt world at link time (`--gc-sections`).

pub mod bridge;

/// Library orchestration (folders, scan worker, watcher batches).
pub mod library;

/// MPRIS D-Bus presence and transport surface (engine wiring lands in S5).
pub mod mpris;

/// Library-to-queue bridging (index rows become playable queue items).
pub mod playback;

/// Debounced search orchestration (keystrokes in, grouped results out).
pub mod search;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

// Anchor the cxx-qt generated initializers into every final link.
//
// Background: the build script's `rustc-link-lib` directives reach test
// targets but not the binary target, so module registration, the type
// registrar, and the compiled QML resources never link into `tunex`
// (verified: no `TuneX_plugin`/qrc symbols in the binary, QML imports
// unresolvable at runtime). Declaring the link explicitly and calling the
// initializers pulls the whole chain through normal symbol references.
#[link(name = "tunex-app-cxxqt-generated", kind = "static")]
unsafe extern "C" {
    fn cxx_qt_init_crate_tunex_app();
    fn cxx_qt_init_qml_module_TuneX();
}

/// Boot Qt, load the `TuneX` module's `App` shell, run until quit.
/// Returns the process exit code; never returns normally.
pub fn run() -> i32 {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tunex=info")),
        )
        .init();

    let config = match tunex_core::load_from(&tunex_core::config_file()) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(
                name: "app.config.fallback",
                error = %err,
                "settings unreadable, starting with defaults"
            );
            tunex_core::TunexConfig::default()
        }
    };
    tracing::info!(
        name: "app.start",
        version = env!("CARGO_PKG_VERSION"),
        library_roots = config.library_roots.len(),
        "tunex starting"
    );
    // MPRIS presence starts with the process (own thread; Qt keeps main).
    mpris::spawn();
    // SAFETY: generated, idempotent initializers; called once on the main
    // thread before any Qt object exists. (`let ()` form satisfies both
    // semicolon lints at once.)
    let () = unsafe {
        cxx_qt_init_crate_tunex_app();
        cxx_qt_init_qml_module_TuneX();
    };

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();

    if let Some(mut engine) = engine.as_mut() {
        // Fail fast with a clear message instead of running windowless: a
        // null object here means the QML shell or its imports are broken.
        engine
            .as_mut()
            .on_object_created(|_, object, _| {
                if object.is_null() {
                    eprintln!("tunex: failed to load QML shell (see Qt output above)");
                    std::process::exit(2);
                }
            })
            .release();
        // Resource path mirrors the crate-relative QML path declared in
        // build.rs (`qml/TuneX/App.qml` under module URI `TuneX`).
        engine.load(&QUrl::from("qrc:/qt/qml/TuneX/qml/TuneX/App.qml"));
    }

    if let Some(app) = app.as_mut() {
        app.exec()
    } else {
        1
    }
}
