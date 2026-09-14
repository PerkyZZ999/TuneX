//! Ten-band equalizer presets and gain clamps (S15 W-071).
//!
//! Gains are dB on `GStreamer`'s `equalizer-10bands` (`band0`…`band9`).
//! Bypass is all zeros; a user move that no longer matches a named preset
//! becomes `"custom"`.

/// Number of ISO-style bands on `equalizer-10bands`.
pub const EQ_BAND_COUNT: usize = 10;
/// Softest cut the Settings sliders allow.
pub const EQ_GAIN_MIN: f32 = -24.0;
/// Hottest boost the Settings sliders allow.
pub const EQ_GAIN_MAX: f32 = 12.0;

/// Centre frequencies shown next to each slider (`GStreamer` defaults, rounded).
pub const EQ_BAND_LABELS: [&str; EQ_BAND_COUNT] = [
    "29 Hz", "59 Hz", "119 Hz", "237 Hz", "474 Hz", "947 Hz", "1.9 kHz", "3.8 kHz", "7.5 kHz",
    "15 kHz",
];

/// Named presets offered in Settings → Playback. `"custom"` is not in this
/// list; it is derived when the live bands match none of these shapes.
pub const EQ_PRESET_IDS: [&str; 8] = [
    "flat",
    "hip-hop",
    "rock",
    "jazz",
    "classic",
    "vocals",
    "electronic",
    "pop",
];

/// Clamp one band into the slider range.
#[must_use]
pub fn clamp_gain(gain: f32) -> f32 {
    gain.clamp(EQ_GAIN_MIN, EQ_GAIN_MAX)
}

/// Clamp every band.
#[must_use]
pub fn clamp_bands(bands: [f32; EQ_BAND_COUNT]) -> [f32; EQ_BAND_COUNT] {
    let mut out = bands;
    for gain in &mut out {
        *gain = clamp_gain(*gain);
    }
    out
}

/// Gains for a named preset. Unknown names yield [`None`].
#[must_use]
pub fn preset_bands(name: &str) -> Option<[f32; EQ_BAND_COUNT]> {
    Some(match name {
        "flat" => [0.0; EQ_BAND_COUNT],
        "hip-hop" => [6.0, 5.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0],
        "rock" => [4.0, 3.0, 1.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        "jazz" => [3.0, 2.0, 0.0, 1.0, 2.0, 2.0, 1.0, 0.0, 1.0, 2.0],
        "classic" => [4.0, 3.0, 0.0, 0.0, 0.0, 0.0, -1.0, 2.0, 3.0, 4.0],
        "vocals" => [-2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 3.0, 1.0, 0.0, -1.0],
        "electronic" => [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 2.0, 4.0, 5.0],
        "pop" => [2.0, 1.0, 0.0, 1.0, 2.0, 1.0, 0.0, 1.0, 2.0, 2.0],
        _ => return None,
    })
}

/// Id of the preset whose bands match, or `"custom"`.
#[must_use]
pub fn matching_preset(bands: [f32; EQ_BAND_COUNT]) -> &'static str {
    for id in EQ_PRESET_IDS {
        if let Some(preset) = preset_bands(id)
            && preset
                .iter()
                .zip(bands.iter())
                .all(|(left, right)| (left - right).abs() < 0.05)
        {
            return id;
        }
    }
    "custom"
}

/// Spectrum magnitude count posted onto the Qt thread.
pub const SPECTRUM_BANDS: usize = 32;
/// Oscilloscope sample count kept for the waveform polyline.
pub const WAVEFORM_SAMPLES: usize = 128;
/// Floor used when mapping spectrum dB into 0…1 bars.
pub const SPECTRUM_DB_FLOOR: f32 = -60.0;

/// Map spectrum dB magnitudes into 0…1 display bars.
#[must_use]
pub fn normalize_spectrum_db(db: &[f32]) -> [f32; SPECTRUM_BANDS] {
    let mut out = [0.0_f32; SPECTRUM_BANDS];
    for (slot, value) in out.iter_mut().zip(db.iter()) {
        *slot = ((*value) - SPECTRUM_DB_FLOOR) / (0.0 - SPECTRUM_DB_FLOOR);
        *slot = slot.clamp(0.0, 1.0);
    }
    out
}

/// Decode little-endian f32 PCM (analysis tap). Odd trailing bytes are dropped.
#[must_use]
pub fn parse_pcm_f32le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            f32::from_bits(bits)
        })
        .collect()
}

/// Decode interleaved S16LE PCM, keeping the first channel.
#[must_use]
pub fn parse_pcm_s16le(bytes: &[u8], channels: usize) -> Vec<f32> {
    let channels = channels.max(1);
    let stride = 2 * channels;
    bytes
        .chunks_exact(stride)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            f32::from(sample) / 32768.0
        })
        .collect()
}

/// Split recent PCM into [`SPECTRUM_BANDS`] RMS bars (0…1).
#[must_use]
pub fn bands_from_pcm(samples: &[f32]) -> [f32; SPECTRUM_BANDS] {
    let mut out = [0.0_f32; SPECTRUM_BANDS];
    if samples.is_empty() {
        return out;
    }
    let chunk = (samples.len() / SPECTRUM_BANDS).max(1);
    for (index, slot) in out.iter_mut().enumerate() {
        let start = index * chunk;
        if start >= samples.len() {
            break;
        }
        let end = (start + chunk).min(samples.len());
        let window = &samples[start..end];
        let count = u16::try_from(window.len()).unwrap_or(1).max(1);
        let mean_sq = window.iter().map(|sample| sample * sample).sum::<f32>() / f32::from(count);
        *slot = mean_sq.sqrt().clamp(0.0, 1.0);
    }
    out
}

/// Keep the newest [`WAVEFORM_SAMPLES`] samples, dropping the oldest.
pub fn push_waveform(buffer: &mut Vec<f32>, incoming: &[f32]) {
    buffer.extend_from_slice(incoming);
    if buffer.len() > WAVEFORM_SAMPLES {
        let extra = buffer.len() - WAVEFORM_SAMPLES;
        buffer.drain(..extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_round_trip_matches_named_ids() {
        for id in EQ_PRESET_IDS {
            let bands = preset_bands(id).expect("named preset exists");
            assert_eq!(matching_preset(bands), id);
        }
    }

    #[test]
    fn moved_band_becomes_custom() {
        let mut bands = preset_bands("flat").expect("flat");
        bands[0] = 3.0;
        assert_eq!(matching_preset(bands), "custom");
    }

    #[test]
    fn clamp_keeps_gains_inside_slider_range() {
        let bands = clamp_bands([80.0, -80.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((bands[0] - EQ_GAIN_MAX).abs() < f32::EPSILON);
        assert!((bands[1] - EQ_GAIN_MIN).abs() < f32::EPSILON);
    }

    #[test]
    fn spectrum_db_maps_into_unit_interval() {
        let bars = normalize_spectrum_db(&[-60.0, -30.0, 0.0, 12.0]);
        assert!((bars[0] - 0.0).abs() < f32::EPSILON);
        assert!((bars[1] - 0.5).abs() < 0.01);
        assert!((bars[2] - 1.0).abs() < f32::EPSILON);
        assert!((bars[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pcm_parse_and_waveform_window() {
        let sample = 0.5_f32.to_le_bytes();
        let parsed = parse_pcm_f32le(&sample);
        assert_eq!(parsed, vec![0.5]);
        let stereo = parse_pcm_s16le(&[0x00, 0x40, 0x00, 0x00], 2);
        assert_eq!(stereo.len(), 1);
        let mut buf = vec![0.0; WAVEFORM_SAMPLES];
        push_waveform(&mut buf, &[1.0, 2.0]);
        assert_eq!(buf.len(), WAVEFORM_SAMPLES);
        assert!((buf[WAVEFORM_SAMPLES - 1] - 2.0).abs() < f32::EPSILON);
        assert!((buf[WAVEFORM_SAMPLES - 2] - 1.0).abs() < f32::EPSILON);
        let bars = bands_from_pcm(&[0.0, 1.0, 0.0, 1.0]);
        assert_eq!(bars.len(), SPECTRUM_BANDS);
        assert!(bars.iter().any(|value| *value > 0.0));
    }
}
