//! `TuneX` application entry point (S1 W-001 scaffold).

/// Boot placeholder: prints the build version until W-002 wires the Qt event
/// loop and `cxx-qt` bridge here. Kept in the binary (not library code), so a
/// plain print is acceptable at this stage.
fn main() {
    println!(
        "tunex {} — scaffold; audio engine lands in M1",
        env!("CARGO_PKG_VERSION")
    );
}
