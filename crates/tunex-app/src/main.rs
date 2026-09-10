//! `tunex` binary: process entry point. All application logic lives in the
//! `tunex_app` library so every target links one shared implementation;
//! calling [`tunex_app::run`] here is what anchors the Qt world in the final
//! binary (see the layout rule in `lib.rs`).

fn main() {
    std::process::exit(tunex_app::run());
}
