//! `tunex-app`: application wiring for `TuneX`.
//!
//! The only crate allowed to touch Qt (via `cxx-qt`, pinned): it owns the
//! bridged `QObject`s, the `QAbstractListModel`s feeding QML, settings/XDG
//! handling, the debounced `notify` watcher, MPRIS/D-Bus, and the fan-out of
//! worker `AppEvent`s onto the Qt thread as queued signals.
//!
//! The `cxx-qt` bridge and Qt event loop land in S1 W-002+; this scaffold
//! intentionally declares no dependencies yet.
