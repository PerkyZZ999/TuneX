//! `tunex` binary: boots the Qt event loop and loads the QML shell.
//!
//! Bridge modules live here (not the library): the linker drops unreferenced
//! archive members, so generated C++ and its Rust implementations must sit in
//! the final executable's own object set to survive `--gc-sections`.

mod bridge;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

// Anchor the cxx-qt generated initializers into the final binary.
//
// Background: the build script's `rustc-link-lib` directives reach test
// targets but not the binary target, so module registration, the type
// registrar, and the compiled QML resources never link into `tunex`
// (verified: no TuneX_plugin/qrc symbols in the binary, QML imports
// unresolvable at runtime). Declaring the link explicitly and calling the
// initializers pulls the whole chain through normal symbol references.
#[link(name = "tunex-app-cxxqt-generated", kind = "static")]
unsafe extern "C" {
    fn cxx_qt_init_crate_tunex_app();
    fn cxx_qt_init_qml_module_TuneX();
}

/// Boot Qt, load the `TuneX` module's `App` shell, run until quit.
/// Exits non-zero when Qt or the QML shell fails to start.
fn main() {
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

    let code = if let Some(app) = app.as_mut() {
        app.exec()
    } else {
        1
    };
    std::process::exit(code);
}
