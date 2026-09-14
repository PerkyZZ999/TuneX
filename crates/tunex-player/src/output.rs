//! `PipeWire` / `GStreamer` audio output enumeration (L-013).
//!
//! Listing runs off the UI thread. The empty id is the system default; a
//! stored id that no longer exists falls back to that default instead of
//! dropping the queue.

use gstreamer::{self as gst, prelude::*};
use tunex_core::{Error, Result};

/// One selectable sink. `id` is empty for the system default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioOutput {
    /// Persistent device id (`node.name` / `device.name`, or empty).
    pub id: String,
    /// Human-readable label for Settings.
    pub label: String,
}

/// System default row, always first in the picker.
#[must_use]
pub fn system_output() -> AudioOutput {
    AudioOutput {
        id: String::new(),
        label: "System".to_owned(),
    }
}

/// Enumerate `Audio/Sink` devices. Always includes [`system_output`].
#[must_use]
pub fn list_audio_outputs() -> Vec<AudioOutput> {
    if gst::init().is_err() {
        return vec![system_output()];
    }
    let mut out = vec![system_output()];
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Audio/Sink"), None);
    let _ = monitor.start();
    for device in monitor.devices() {
        let label = device.display_name().to_string();
        if label.is_empty() {
            continue;
        }
        let id = device_id(&device);
        if id.is_empty() || out.iter().any(|row| row.id == id) {
            continue;
        }
        out.push(AudioOutput { id, label });
    }
    monitor.stop();
    out
}

fn device_id(device: &gst::Device) -> String {
    let Some(props) = device.properties() else {
        return device.display_name().to_string();
    };
    props
        .get::<String>("node.name")
        .or_else(|_| props.get::<String>("device.name"))
        .or_else(|_| props.get::<String>("name"))
        .unwrap_or_else(|_| device.display_name().to_string())
}

/// Build the terminal sink for `id`. Empty id uses `autoaudiosink`.
///
/// # Errors
///
/// Returns [`Error::Player`] when no usable sink element exists.
pub fn sink_for_device(id: &str) -> Result<gst::Element> {
    if gst::init().is_err() {
        return Err(Error::Player("gstreamer init failed".to_owned()));
    }
    if id.is_empty() {
        return gst::ElementFactory::make("autoaudiosink")
            .build()
            .or_else(|_| gst::ElementFactory::make("fakesink").build())
            .map_err(|err| Error::Player(format!("no audio sink: {err}")));
    }
    if let Some(element) = sink_from_monitor(id) {
        return Ok(element);
    }
    // PipeWire / Pulse fallback: target the named node if the monitor missed it.
    if let Ok(sink) = gst::ElementFactory::make("pipewiresink").build() {
        sink.set_property("target-object", id);
        return Ok(sink);
    }
    if let Ok(sink) = gst::ElementFactory::make("pulsesink").build() {
        sink.set_property("device", id);
        return Ok(sink);
    }
    Err(Error::Player(format!(
        "output device {id} is not available"
    )))
}

fn sink_from_monitor(id: &str) -> Option<gst::Element> {
    let monitor = gst::DeviceMonitor::new();
    monitor.add_filter(Some("Audio/Sink"), None);
    let _ = monitor.start();
    let found = monitor
        .devices()
        .into_iter()
        .find(|device| device_id(device) == id);
    monitor.stop();
    found.and_then(|device| device.create_element(None).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_starts_with_system() {
        let list = list_audio_outputs();
        assert_eq!(list[0], system_output());
        assert_eq!(list.iter().filter(|row| row.id.is_empty()).count(), 1);
    }

    #[test]
    fn system_sink_builds() {
        let sink = sink_for_device("").expect("system sink");
        let factory = sink.factory().map(|factory| factory.name().to_string());
        assert!(
            factory
                .as_deref()
                .is_some_and(|name| name == "autoaudiosink" || name == "fakesink"),
            "unexpected factory {factory:?}"
        );
    }
}
