//! Downstream audio bin: equalizer, `ReplayGain`, crossfade, PCM tap.
//!
//! Layout (S15 + S16):
//! `equalizer-10bands` → `ReplayGain` volume → crossfade volume → sink
//!
//! Analysis is a pad probe on the equalizer source — a second sink/tee would
//! participate in FLUSH seeks and stall the playhead. Missing
//! `equalizer-10bands` never fails playback: the bin stays `ReplayGain` →
//! crossfade → sink and [`AudioChain::eq_missing`] is true so Settings can
//! say so.

use std::sync::{Arc, Mutex};

use gstreamer::{self as gst, glib::object::ObjectExt as _, prelude::*};
use tunex_core::{
    EQ_BAND_COUNT, Error, Result, SPECTRUM_BANDS, bands_from_pcm, parse_pcm_f32le, parse_pcm_s16le,
    push_waveform,
};

use super::engine::Inner;

/// Pieces the engine keeps after wrapping a terminal sink.
pub(crate) struct AudioChain {
    pub bin: gst::Bin,
    pub eq: Option<gst::Element>,
    pub rg: gst::Element,
    pub xfade: gst::Element,
    pub eq_missing: bool,
    pub pcm_pad: Option<gst::Pad>,
}

/// `equalizer-10bands ! volume(rg) ! volume(xfade) ! terminal`.
/// Missing equalizer skips that element; playback stays flat.
pub(crate) fn wrap_audio_sink(terminal: &gst::Element) -> Result<AudioChain> {
    let terminal = terminal.clone();
    let bin = gst::Bin::builder().name("tunex-audio").build();
    let rg = volume_element("tunex-rg")?;
    let xfade = volume_element("tunex-xfade")?;
    let (eq, eq_missing) = make_equalizer();
    let head = eq.as_ref().unwrap_or(&rg);
    match &eq {
        Some(filter) => bin
            .add_many([filter, &rg, &xfade, &terminal])
            .map_err(|err| Error::Player(format!("audio bin add failed: {err}")))?,
        None => bin
            .add_many([&rg, &xfade, &terminal])
            .map_err(|err| Error::Player(format!("audio bin add failed: {err}")))?,
    }
    if let Some(filter) = &eq {
        filter
            .link(&rg)
            .map_err(|err| Error::Player(format!("equalizer link failed: {err}")))?;
    }
    rg.link(&xfade)
        .map_err(|err| Error::Player(format!("replaygain link failed: {err}")))?;
    xfade
        .link(&terminal)
        .map_err(|err| Error::Player(format!("crossfade link failed: {err}")))?;
    let sink_pad = head
        .static_pad("sink")
        .ok_or_else(|| Error::Player("audio bin head has no sink pad".to_owned()))?;
    let ghost = gst::GhostPad::builder_with_target(&sink_pad)
        .map_err(|err| Error::Player(format!("audio ghost pad: {err}")))?
        .name("sink")
        .build();
    bin.add_pad(&ghost)
        .map_err(|err| Error::Player(format!("audio ghost pad add: {err}")))?;
    let pcm_pad = eq
        .as_ref()
        .and_then(|filter| filter.static_pad("src"))
        .or_else(|| rg.static_pad("src"));
    Ok(AudioChain {
        bin,
        eq,
        rg,
        xfade,
        eq_missing,
        pcm_pad,
    })
}

fn make_equalizer() -> (Option<gst::Element>, bool) {
    match gst::ElementFactory::make("equalizer-10bands")
        .name("tunex-eq")
        .build()
    {
        Ok(eq) => {
            for band in 0..EQ_BAND_COUNT {
                eq.set_property(&format!("band{band}"), 0.0_f64);
            }
            (Some(eq), false)
        }
        Err(err) => {
            tracing::warn!(
                name: "player.eq.missing",
                error = %err,
                "equalizer-10bands unavailable; playback continues flat"
            );
            (None, true)
        }
    }
}

fn volume_element(name: &str) -> Result<gst::Element> {
    let element = gst::ElementFactory::make("volume")
        .name(name)
        .build()
        .map_err(|err| Error::Player(format!("volume element unavailable: {err}")))?;
    element.set_property("volume", 1.0_f64);
    Ok(element)
}

/// Write stored gains onto the live equalizer (zeros when disabled).
pub(crate) fn apply_equalizer(inner: &Inner) {
    let Some(eq) = &inner.eq else {
        return;
    };
    let bands = if inner.eq_enabled {
        inner.eq_bands
    } else {
        [0.0; EQ_BAND_COUNT]
    };
    for (index, gain) in bands.iter().enumerate() {
        eq.set_property(&format!("band{index}"), f64::from(*gain));
    }
}

/// Copy PCM from the equalizer source into [`Inner::pcm`] and spectrum bars.
pub(crate) fn attach_pcm_probe(pad: &gst::Pad, inner: &Arc<Mutex<Inner>>) {
    let inner = Arc::clone(inner);
    pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
        let Some(buffer) = info.buffer() else {
            return gst::PadProbeReturn::Ok;
        };
        let Ok(map) = buffer.map_readable() else {
            return gst::PadProbeReturn::Ok;
        };
        let samples = samples_from_pad(pad, map.as_slice());
        if samples.is_empty() {
            return gst::PadProbeReturn::Ok;
        }
        if let Ok(mut guard) = inner.lock() {
            push_waveform(&mut guard.pcm, &samples);
            guard.spectrum = bands_from_pcm(&guard.pcm);
        }
        gst::PadProbeReturn::Ok
    });
}

fn samples_from_pad(pad: &gst::Pad, bytes: &[u8]) -> Vec<f32> {
    let caps = pad.current_caps();
    let structure = caps.as_ref().and_then(|caps| caps.structure(0));
    let format = structure
        .and_then(|item| item.get::<String>("format").ok())
        .unwrap_or_else(|| "S16LE".to_owned());
    let channels = structure
        .and_then(|item| item.get::<i32>("channels").ok())
        .and_then(|count| usize::try_from(count.max(1)).ok())
        .unwrap_or(1);
    if format.contains("F32") {
        parse_pcm_f32le(bytes)
    } else {
        parse_pcm_s16le(bytes, channels)
    }
}

/// Flatten spectrum bars into a CSV the QML well can split.
#[must_use]
pub(crate) fn spectrum_csv(bands: &[f32; SPECTRUM_BANDS]) -> String {
    bands
        .iter()
        .map(|value| format!("{value:.4}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Flatten the waveform ring into a CSV.
#[must_use]
pub(crate) fn waveform_csv(samples: &[f32]) -> String {
    samples
        .iter()
        .map(|value| format!("{value:.4}"))
        .collect::<Vec<_>>()
        .join(",")
}
