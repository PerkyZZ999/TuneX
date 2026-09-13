//! `StatusNotifierItem` tray presence (Plasma/Wayland).
//!
//! The tray lives on D-Bus, not Qt Widgets: D-002 keeps the UI in Qt Quick,
//! and SNI is the native host protocol on Plasma. A detached thread owns the
//! session-bus connection (same shape as MPRIS/notify) so the Qt poll never
//! blocks. Hosts that expose no tray watcher are a debug log, never an
//! error — the app stays usable headless and offline (R-017).
//!
//! Hover: the host draws the SNI tooltip (title/artist). Activate and
//! context-menu coordinates are forwarded to QML so the compact glass popup
//! can sit next to the icon. Secondary-activate is play/pause.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::mpsc;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedObjectPath;
use zbus::{Connection, interface, proxy::Proxy};

/// Depth of the inbound host→app event channel. The Qt poll drains it
/// every 300 ms; overflow drops the oldest poke rather than wedging D-Bus.
const EVENT_CHANNEL_DEPTH: usize = 32;

/// How often the SNI thread republishes tooltip/title when the snapshot moves.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);

/// Well-known watcher that owns the panel icon list.
const WATCHER_SERVICE: &str = "org.kde.StatusNotifierWatcher";
/// Watcher object path.
const WATCHER_PATH: &str = "/StatusNotifierWatcher";
/// Watcher interface.
const WATCHER_INTERFACE: &str = "org.kde.StatusNotifierWatcher";
/// SNI object path we serve.
const ITEM_PATH: &str = "/StatusNotifierItem";
/// Icon name when the packaged hicolor icon is installed.
const ICON_PACKAGED: &str = "tunex";
/// Icon name that exists on a stock desktop even without our package.
const ICON_FALLBACK: &str = "audio-x-generic";

/// Inbound host actions, drained on the Qt thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayEvent {
    /// Left click / activate. Coordinates are host pixels; `0,0` means unknown.
    Popup {
        /// Horizontal origin from the host, when provided.
        x: i32,
        /// Vertical origin from the host, when provided.
        y: i32,
    },
    /// Right click / context menu at the same coordinates.
    Menu {
        /// Horizontal origin from the host, when provided.
        x: i32,
        /// Vertical origin from the host, when provided.
        y: i32,
    },
    /// Middle click: play/pause without opening a surface.
    PlayPause,
}

/// Now-playing copy published into the SNI tooltip.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TraySnapshot {
    /// Track title; empty when idle.
    pub title: String,
    /// Track artist; empty when untagged or idle.
    pub artist: String,
    /// Whether the engine is currently playing.
    pub playing: bool,
}

/// Process-wide tray hub. The SNI thread writes `available`; the Qt poll
/// publishes snapshots and drains events.
struct TrayHub {
    snapshot: Mutex<TraySnapshot>,
    dirty: AtomicBool,
    available: AtomicBool,
    events: Mutex<Option<mpsc::Receiver<TrayEvent>>>,
    events_tx: mpsc::Sender<TrayEvent>,
}

static INSTALLED: OnceLock<Arc<TrayHub>> = OnceLock::new();

/// Whether a tray host accepted our registration.
#[must_use]
pub fn is_available() -> bool {
    INSTALLED
        .get()
        .is_some_and(|hub| hub.available.load(Ordering::Relaxed))
}

/// Hide-on-close is the setting *and* a live tray host. A missing host must
/// never leave a headless process behind.
#[must_use]
pub fn should_hide_on_close(close_to_tray: bool, tray_available: bool) -> bool {
    close_to_tray && tray_available
}

/// Tooltip title: the track name, or the app name when idle.
#[must_use]
pub fn tooltip_title(title: &str) -> String {
    if title.is_empty() {
        "TuneX".to_owned()
    } else {
        title.to_owned()
    }
}

/// Tooltip body: artist when known, otherwise an honest idle/playing line.
#[must_use]
pub fn tooltip_body(artist: &str, playing: bool, has_track: bool) -> String {
    if !artist.is_empty() {
        return artist.to_owned();
    }
    if !has_track {
        return "Nothing playing".to_owned();
    }
    if playing {
        "Playing".to_owned()
    } else {
        "Paused".to_owned()
    }
}

/// Publish now-playing copy for the tooltip. No-op before [`spawn`].
pub fn publish(snapshot: TraySnapshot) {
    let Some(hub) = INSTALLED.get() else {
        return;
    };
    let mut slot = lock_snapshot(&hub.snapshot);
    if *slot == snapshot {
        return;
    }
    *slot = snapshot;
    hub.dirty.store(true, Ordering::Relaxed);
}

/// Drain host events for the Qt poll. Empty when the tray is not running.
pub fn take_events() -> Vec<TrayEvent> {
    let Some(hub) = INSTALLED.get() else {
        return Vec::new();
    };
    let mut events = Vec::new();
    if let Some(rx) = hub
        .events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_mut()
    {
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
    }
    events
}

/// Serve a `StatusNotifierItem` on a background thread for the process lifetime.
pub fn spawn() {
    let (events_tx, events_rx) = mpsc::channel(EVENT_CHANNEL_DEPTH);
    let hub = Arc::new(TrayHub {
        snapshot: Mutex::new(TraySnapshot::default()),
        dirty: AtomicBool::new(false),
        available: AtomicBool::new(false),
        events: Mutex::new(Some(events_rx)),
        events_tx,
    });
    let _ = INSTALLED.set(Arc::clone(&hub));
    if std::thread::Builder::new()
        .name("tunex-tray".to_owned())
        .spawn(move || run_server_thread(hub))
        .is_err()
    {
        tracing::debug!(name: "app.tray.spawn", "tray thread failed to spawn");
    }
}

/// Icon name the host should look up in the theme.
#[must_use]
pub fn theme_icon_name() -> &'static str {
    if std::path::Path::new("/usr/share/icons/hicolor/256x256/apps/tunex.png").is_file() {
        ICON_PACKAGED
    } else {
        ICON_FALLBACK
    }
}

fn lock_snapshot(slot: &Mutex<TraySnapshot>) -> std::sync::MutexGuard<'_, TraySnapshot> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn run_server_thread(hub: Arc<TrayHub>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::debug!(
                name: "app.tray.runtime",
                error = %err,
                "tray runtime unavailable"
            );
            return;
        }
    };
    runtime.block_on(serve(hub));
}

async fn serve(hub: Arc<TrayHub>) {
    let pid = std::process::id();
    let bus_name = format!("org.kde.StatusNotifierItem-{pid}-1");
    let item = StatusNotifier {
        hub: Arc::clone(&hub),
    };
    let connection = match zbus::connection::Builder::session()
        .and_then(|builder| builder.name(bus_name.clone()))
        .and_then(|builder| builder.serve_at(ITEM_PATH, item))
    {
        Ok(builder) => match builder.build().await {
            Ok(connection) => connection,
            Err(err) => {
                tracing::debug!(
                    name: "app.tray.bus",
                    error = %err,
                    "session bus unavailable"
                );
                return;
            }
        },
        Err(err) => {
            tracing::debug!(
                name: "app.tray.builder",
                error = %err,
                "status notifier not served"
            );
            return;
        }
    };

    if !register_with_watcher(&connection, &bus_name).await {
        return;
    }
    hub.available.store(true, Ordering::Relaxed);
    tracing::info!(name: "app.tray.serving", bus_name = %bus_name, "status notifier registered");

    let iface = match connection
        .object_server()
        .interface::<_, StatusNotifier>(ITEM_PATH)
        .await
    {
        Ok(iface) => iface,
        Err(err) => {
            tracing::debug!(
                name: "app.tray.iface",
                error = %err,
                "status notifier interface missing after serve"
            );
            return;
        }
    };

    loop {
        tokio::time::sleep(WATCH_INTERVAL).await;
        if hub.dirty.swap(false, Ordering::Relaxed) {
            let emitter = iface.signal_emitter();
            if let Err(err) = StatusNotifier::new_tool_tip(emitter).await {
                tracing::debug!(name: "app.tray.signal", error = %err, "NewToolTip failed");
            }
            if let Err(err) = StatusNotifier::new_title(emitter).await {
                tracing::debug!(name: "app.tray.signal", error = %err, "NewTitle failed");
            }
        }
    }
}

async fn register_with_watcher(connection: &Connection, bus_name: &str) -> bool {
    let proxy = match Proxy::new(connection, WATCHER_SERVICE, WATCHER_PATH, WATCHER_INTERFACE).await
    {
        Ok(proxy) => proxy,
        Err(err) => {
            tracing::debug!(
                name: "app.tray.no_watcher",
                error = %err,
                "status notifier watcher absent"
            );
            return false;
        }
    };
    let result: zbus::Result<()> = proxy.call("RegisterStatusNotifierItem", &(bus_name,)).await;
    if let Err(err) = result {
        tracing::debug!(
            name: "app.tray.register",
            error = %err,
            "status notifier registration declined"
        );
        return false;
    }
    true
}

/// D-Bus `StatusNotifierItem`. Snapshot reads are non-blocking mutex takes.
struct StatusNotifier {
    hub: Arc<TrayHub>,
}

impl StatusNotifier {
    fn snapshot(&self) -> TraySnapshot {
        lock_snapshot(&self.hub.snapshot).clone()
    }

    fn push(&self, event: TrayEvent) {
        match self.hub.events_tx.try_send(event) {
            Err(mpsc::error::TrySendError::Full(dropped)) => {
                tracing::debug!(
                    name: "app.tray.event_overflow",
                    ?dropped,
                    "tray event dropped"
                );
            }
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

/// Host tooltip payload: icon name, pixmaps, title, body.
type SniToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

#[interface(name = "org.kde.StatusNotifierItem")]
#[expect(
    clippy::unused_self,
    reason = "SNI D-Bus methods are instance-shaped by spec"
)]
impl StatusNotifier {
    fn context_menu(&self, x: i32, y: i32) {
        self.push(TrayEvent::Menu { x, y });
    }

    fn activate(&self, x: i32, y: i32) {
        self.push(TrayEvent::Popup { x, y });
    }

    fn secondary_activate(&self, x: i32, y: i32) {
        let _ = (x, y);
        self.push(TrayEvent::PlayPause);
    }

    fn scroll(&self, delta: i32, orientation: &str) {
        let _ = (self, delta, orientation);
    }

    #[zbus(property)]
    fn category(&self) -> String {
        let _ = self;
        "ApplicationStatus".to_owned()
    }

    #[zbus(property)]
    fn id(&self) -> String {
        let _ = self;
        "tunex".to_owned()
    }

    #[zbus(property)]
    fn title(&self) -> String {
        tooltip_title(&self.snapshot().title)
    }

    #[zbus(property)]
    fn status(&self) -> String {
        let _ = self;
        "Active".to_owned()
    }

    #[zbus(property)]
    fn window_id(&self) -> u32 {
        let _ = self;
        0
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        theme_icon_name().to_owned()
    }

    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        let _ = self;
        OwnedObjectPath::try_from("/NO_DBUSMENU").expect("SNI sentinel is a valid object path")
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        let _ = self;
        false
    }

    #[zbus(property)]
    fn tool_tip(&self) -> SniToolTip {
        let snap = self.snapshot();
        (
            theme_icon_name().to_owned(),
            Vec::new(),
            tooltip_title(&snap.title),
            tooltip_body(&snap.artist, snap.playing, !snap.title.is_empty()),
        )
    }

    #[zbus(signal)]
    async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_on_close_needs_both_setting_and_host() {
        assert!(should_hide_on_close(true, true), "setting on + host → hide");
        assert!(
            !should_hide_on_close(true, false),
            "no host must still quit"
        );
        assert!(
            !should_hide_on_close(false, true),
            "setting off always quits"
        );
        assert!(!should_hide_on_close(false, false));
    }

    #[test]
    fn tooltip_uses_track_when_present() {
        assert_eq!(tooltip_title(""), "TuneX");
        assert_eq!(tooltip_title("Blue Hour"), "Blue Hour");
        assert_eq!(tooltip_body("Nova Rae", true, true), "Nova Rae");
        assert_eq!(tooltip_body("", false, false), "Nothing playing");
        assert_eq!(tooltip_body("", true, true), "Playing");
        assert_eq!(tooltip_body("", false, true), "Paused");
    }

    #[test]
    fn take_events_is_empty_before_spawn() {
        assert!(take_events().is_empty());
        assert!(!is_available());
    }
}
