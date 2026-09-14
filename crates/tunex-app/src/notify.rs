//! Desktop notifications over D-Bus (S5 W-034).
//!
//! Track-change + playback-error toasts via `org.freedesktop.Notifications`
//! (`Notify`). Best-effort by design: no server on the session bus means a
//! debug log, never an error — the app must work identically with networking
//! disabled and without a notification daemon (D-014, R-017). Delivery runs
//! on a detached thread so the Qt poll never blocks on D-Bus (R-NFR-01).
//!
//! Only `tunex-app` touches this module (D-007/D-008: platform surface).

use std::collections::HashMap;

/// Well-known notification service (session bus, local only — offline-safe).
const SERVICE: &str = "org.freedesktop.Notifications";
/// Well-known notification object path.
const PATH: &str = "/org/freedesktop/Notifications";
/// Notification interface carrying `Notify`.
const INTERFACE: &str = "org.freedesktop.Notifications";
/// Sender name shown by the notification server.
const APP_NAME: &str = "TuneX";
/// Freedesktop icon name (no file lookup, always offline-safe).
const APP_ICON: &str = "audio-x-generic";
/// Explicit auto-expiry so toasts never linger (minimal, non-intrusive).
const EXPIRE_TIMEOUT_MS: i32 = 5_000;

/// Body line for a track toast: known parts joined, unknown parts omitted
/// (never fabricated — missing tags stay missing).
#[must_use]
pub fn track_body(artist: Option<&str>, album: Option<&str>) -> String {
    match (artist, album) {
        (Some(artist), Some(album)) => format!("{artist} — {album}"),
        (Some(artist), None) => artist.to_owned(),
        (None, Some(album)) => album.to_owned(),
        (None, None) => String::new(),
    }
}

/// Whether the current track deserves a toast: a loaded track whose URI
/// differs from the last toasted one, notifications are on, and the window
/// is not focused. Stops and repeats never notify.
#[must_use]
pub fn should_notify(
    last_uri: Option<&str>,
    current_uri: Option<&str>,
    window_active: bool,
    enabled: bool,
    track_change: bool,
) -> bool {
    if !enabled || !track_change || window_active {
        return false;
    }
    match (last_uri, current_uri) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(last), Some(current)) => last != current,
    }
}

/// Whether a playback error should toast (independent of window focus).
#[must_use]
pub fn should_notify_error(enabled: bool, playback_errors: bool) -> bool {
    enabled && playback_errors
}

/// Queue a toast for delivery on a detached thread. Returns immediately;
/// every failure below (no bus, no server, rejected message) is a debug log.
pub fn post(summary: String, body: String) {
    if summary.is_empty() && body.is_empty() {
        return;
    }
    if std::thread::Builder::new()
        .name("tunex-notify".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match runtime {
                Ok(runtime) => runtime.block_on(send_notify(&summary, &body)),
                Err(err) => tracing::debug!(
                    name: "app.notify.runtime",
                    error = %err,
                    "notification runtime unavailable"
                ),
            }
        })
        .is_err()
    {
        tracing::debug!(name: "app.notify.spawn", "notification thread failed to spawn");
    }
}

/// One `Notify` call: transient hint (servers may skip persistence) and an
/// explicit timeout keep it minimal and non-intrusive.
async fn send_notify(summary: &str, body: &str) {
    use mpris_server::zbus::{Connection, proxy::Proxy, zvariant::Value};

    let connection = match Connection::session().await {
        Ok(connection) => connection,
        Err(err) => {
            tracing::debug!(
                name: "app.notify.no_bus",
                error = %err,
                "session bus unavailable"
            );
            return;
        }
    };
    let proxy = match Proxy::new(&connection, SERVICE, PATH, INTERFACE).await {
        Ok(proxy) => proxy,
        Err(err) => {
            tracing::debug!(
                name: "app.notify.no_proxy",
                error = %err,
                "notification proxy unavailable"
            );
            return;
        }
    };
    let mut hints = HashMap::new();
    hints.insert("transient", Value::Bool(true));
    let result: Result<u32, _> = proxy
        .call(
            "Notify",
            &(
                APP_NAME,
                0_u32,
                APP_ICON,
                summary,
                body,
                Vec::<&str>::new(),
                hints,
                EXPIRE_TIMEOUT_MS,
            ),
        )
        .await;
    if let Err(err) = result {
        tracing::debug!(
            name: "app.notify.send_failed",
            error = %err,
            "notification server declined the toast"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_body_joins_known_parts_only() {
        assert_eq!(
            track_body(Some("Nova Rae"), Some("Night Tapes")),
            "Nova Rae — Night Tapes"
        );
        assert_eq!(track_body(Some("Nova Rae"), None), "Nova Rae");
        assert_eq!(track_body(None, Some("Night Tapes")), "Night Tapes");
        assert_eq!(track_body(None, None), "");
    }

    #[test]
    fn should_notify_fires_only_on_new_tracks() {
        assert!(
            !should_notify(None, None, false, true, true),
            "nothing loaded, no toast"
        );
        assert!(
            should_notify(None, Some("file:///a.flac"), false, true, true),
            "first track while unfocused"
        );
        assert!(
            !should_notify(
                Some("file:///a.flac"),
                Some("file:///a.flac"),
                false,
                true,
                true
            ),
            "repeat is not a change"
        );
        assert!(
            should_notify(
                Some("file:///a.flac"),
                Some("file:///b.flac"),
                false,
                true,
                true
            ),
            "advance notifies when unfocused"
        );
        assert!(
            !should_notify(
                Some("file:///a.flac"),
                Some("file:///b.flac"),
                true,
                true,
                true
            ),
            "focused skip stays silent"
        );
        assert!(
            !should_notify(Some("file:///a.flac"), None, false, true, true),
            "stops never notify"
        );
        assert!(
            !should_notify(
                Some("file:///a.flac"),
                Some("file:///b.flac"),
                false,
                false,
                true
            ),
            "master off"
        );
        assert!(
            !should_notify(
                Some("file:///a.flac"),
                Some("file:///b.flac"),
                false,
                true,
                false
            ),
            "track-change off"
        );
    }

    #[test]
    fn should_notify_error_ignores_focus() {
        assert!(should_notify_error(true, true));
        assert!(!should_notify_error(false, true));
        assert!(!should_notify_error(true, false));
    }

    #[test]
    fn post_without_content_sends_nothing() {
        // Returns immediately; nothing to deliver.
        post(String::new(), String::new());
    }

    #[test]
    fn post_returns_immediately_without_a_server() {
        // Headless CI has a session bus but no notification daemon: the
        // detached send fails gracefully while `post` itself never blocks.
        let start = std::time::Instant::now();
        post("TuneX test".to_owned(), "body".to_owned());
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "post must not block the caller"
        );
    }
}
