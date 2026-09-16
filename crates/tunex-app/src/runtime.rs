//! Detached single-thread runtimes for presence threads (C18).
//!
//! MPRIS, tray, and notification delivery each run on their own thread with
//! a current-thread `tokio` runtime. The builder chain is shared here;
//! failure logging stays per service (a dead MPRIS runtime is an error, a
//! dead toast runtime is a debug).

/// Build a current-thread runtime for a detached presence thread.
///
/// # Errors
///
/// Returns the builder's I/O error when the runtime cannot start; callers
/// log at their own level and degrade (never abort).
pub fn current_thread_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}
