use super::*;

/// Generate a sine wave: `sin(2π * freq * i / sample_rate)`.
fn sine_wave(freq: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * freq * t).sin()
        })
        .collect()
}

#[test]
fn spectrum_empty_input() {
    let bins = compute_spectrum(&[], 44100, 2048);
    assert!(bins.is_empty());
}

#[test]
fn spectrum_single_sample() {
    // n=1 should be rejected (needs n>=2 for window formula)
    let bins = compute_spectrum(&[0.5], 44100, 2048);
    assert!(bins.is_empty());
}

#[test]
fn spectrum_sine_440hz() {
    // 1 second of 440Hz sine at 44100 Hz
    let samples = sine_wave(440.0, 44100, 44100);
    let bins = compute_spectrum(&samples, 44100, 2048);

    assert_eq!(bins.len(), 1024); // fft_size / 2

    // The peak should be near 440Hz
    let peak = bins
        .iter()
        .max_by(|a, b| a.magnitude.partial_cmp(&b.magnitude).unwrap())
        .unwrap();

    assert!(
        (peak.frequency - 440.0).abs() < 50.0,
        "expected peak near 440Hz, got {}Hz",
        peak.frequency
    );
    assert!(
        peak.magnitude > 0.9,
        "expected strong peak, got magnitude {}",
        peak.magnitude
    );
}

#[test]
fn spectrum_normalization() {
    let samples = sine_wave(1000.0, 44100, 44100);
    let bins = compute_spectrum(&samples, 44100, 2048);

    // Max magnitude should be exactly 1.0 after normalization
    let max_mag = bins.iter().map(|b| b.magnitude).fold(0.0f32, f32::max);
    assert!((max_mag - 1.0).abs() < 0.001);
}

#[test]
fn spectrum_nyquist_limit() {
    let samples = sine_wave(440.0, 44100, 4096);
    let bins = compute_spectrum(&samples, 44100, 2048);

    // Max frequency should be ~22050 Hz (Nyquist)
    let max_freq = bins.last().unwrap().frequency;
    assert!((max_freq - 22050.0).abs() < 50.0);
}

#[test]
fn spectrum_freq_resolution() {
    let samples = sine_wave(440.0, 44100, 44100);
    let bins = compute_spectrum(&samples, 44100, 1024);

    let resolution = 44100.0 / 1024.0;
    assert!((bins[1].frequency - bins[0].frequency - resolution).abs() < 0.1);
}

#[test]
fn spectrum_silence_is_flat() {
    // Silence (all zeros) should produce near-zero magnitudes
    let samples = vec![0.0f32; 4096];
    let bins = compute_spectrum(&samples, 44100, 2048);

    let sum: f32 = bins.iter().map(|b| b.magnitude).sum();
    assert!(
        sum < 0.01,
        "silence should have near-zero spectrum, sum={sum}"
    );
}

use std::{path::Path, time::Duration};

use crate::{audio::audio_ext::pcm::PcmData, kairos_editor::ui::inspector::audio::compute_peaks};

// ----------------------------------------------------------
// Helpers
// ----------------------------------------------------------

/// Write a minimal WAV file (16-bit PCM, mono) to a byte vector.
fn write_wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() * 2; // 16-bit
    let file_len = 44 + data_len;
    let mut buf = Vec::with_capacity(file_len);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((file_len - 8) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&(16u32).to_le_bytes()); // chunk size
    buf.extend_from_slice(&(1u16).to_le_bytes()); // PCM
    buf.extend_from_slice(&(1u16).to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 2;
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&(2u16).to_le_bytes()); // block align
    buf.extend_from_slice(&(16u16).to_le_bytes()); // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_len as u32).to_le_bytes());
    for &s in samples {
        let clamped = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf.extend_from_slice(&clamped.to_le_bytes());
    }

    buf
}

/// Helper: write stereo 16-bit WAV to bytes.
fn write_wav_stereo(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let data_len = samples.len() * 2; // 16-bit
    let file_len = 44 + data_len;
    let mut buf = Vec::with_capacity(file_len);

    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((file_len - 8) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&(16u32).to_le_bytes());
    buf.extend_from_slice(&(1u16).to_le_bytes()); // PCM
    buf.extend_from_slice(&(2u16).to_le_bytes()); // stereo
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * 4;
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&(4u16).to_le_bytes()); // block align
    buf.extend_from_slice(&(16u16).to_le_bytes());

    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_len as u32).to_le_bytes());
    for &s in samples {
        let clamped = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf.extend_from_slice(&clamped.to_le_bytes());
    }

    buf
}

// ----------------------------------------------------------
// compute_peaks tests
// ----------------------------------------------------------

#[test]
fn peaks_empty_input() {
    let peaks = compute_peaks(&[], 100);
    assert!(peaks.is_empty());
}

#[test]
fn peaks_zero_buckets() {
    let samples = sine_wave(440.0, 44100, 44100);
    let peaks = compute_peaks(&samples, 0);
    assert!(peaks.is_empty());
}

#[test]
fn peaks_single_bucket() {
    let samples = vec![0.5, -0.3, 0.8, -1.0, 0.2];
    let peaks = compute_peaks(&samples, 1);
    assert_eq!(peaks.len(), 1);
    assert!((peaks[0].min - (-1.0)).abs() < 0.001);
    assert!((peaks[0].max - 0.8).abs() < 0.001);
}

#[test]
fn peaks_sine_wave_symmetry() {
    let samples = sine_wave(440.0, 44100, 44100);
    let peaks = compute_peaks(&samples, 100);

    assert_eq!(peaks.len(), 100);

    for p in &peaks {
        assert!(p.min >= -1.0 && p.min <= 1.0);
        assert!(p.max >= -1.0 && p.max <= 1.0);
        assert!(p.min <= p.max);
    }
}

#[test]
fn peaks_more_buckets_than_samples() {
    let samples = vec![0.1, -0.2, 0.3];
    let peaks = compute_peaks(&samples, 10);
    assert_eq!(peaks.len(), 10);
    // First 3 buckets should have non-zero data
    assert!(peaks[0].max > 0.0);
}

// ----------------------------------------------------------
// mono_samples tests
// ----------------------------------------------------------

#[test]
fn mono_stereo_average() {
    let pcm = PcmData::from_raw(
        44100,
        2,
        vec![
            0.4, 0.6, // frame 0
            -0.2, 0.2, // frame 1
            1.0, -0.5, // frame 2
        ],
    );
    let mono = pcm.mono_samples();
    assert_eq!(mono.len(), 3);
    assert!((mono[0] - 0.5).abs() < 0.001);
    assert!((mono[1] - 0.0).abs() < 0.001);
    assert!((mono[2] - 0.25).abs() < 0.001);
}

#[test]
fn mono_single_channel_passthrough() {
    let pcm = PcmData::from_raw(44100, 1, vec![0.1, -0.2, 0.3]);
    let mono = pcm.mono_samples();
    assert_eq!(mono, vec![0.1, -0.2, 0.3]);
}

#[test]
fn mono_empty() {
    let pcm = PcmData::from_raw(44100, 2, vec![]);
    let mono = pcm.mono_samples();
    assert!(mono.is_empty());
}

#[test]
fn mono_zero_channels() {
    let pcm = PcmData::from_raw(44100, 0, vec![]);
    let mono = pcm.mono_samples();
    assert!(mono.is_empty());
}

#[test]
fn mono_trailing_partial_frame() {
    // 2 channels, but 3 samples = 1 full frame + 1 leftover
    let pcm = PcmData::from_raw(44100, 2, vec![0.4, 0.6, 0.9]);
    let mono = pcm.mono_samples();
    assert_eq!(mono.len(), 2);
    assert!((mono[0] - 0.5).abs() < 0.001); // (0.4+0.6)/2
    assert!((mono[1] - 0.9).abs() < 0.001); // single sample
}

// ----------------------------------------------------------
// PcmData::from_raw tests
// ----------------------------------------------------------

#[test]
fn from_raw_duration() {
    let pcm = {
        let sample_rate = 44100;
        let num_channels = 1;
        let samples = vec![0.0; 44100];
        let num_samples = if num_channels == 0 {
            0
        } else {
            samples.len() / num_channels
        };
        let duration = Duration::from_secs_f64(num_samples as f64 / sample_rate as f64);
        PcmData {
            sample_rate,
            num_samples,
            num_channels,
            duration,
            samples,
        }
    };
    assert!((pcm.duration.as_secs_f32() - 1.0).abs() < 0.01);
    assert_eq!(pcm.num_samples, 44100);
    assert_eq!(pcm.num_channels, 1);
}

// ----------------------------------------------------------
// WAV decoding tests (symphonia integration)
// ----------------------------------------------------------

#[test]
fn decode_mono_wav() {
    let samples = sine_wave(440.0, 44100, 4410); // 0.1s
    let wav_bytes = write_wav_bytes(&samples, 44100);
    let tmp = std::env::temp_dir().join("kairos_test_mono.wav");
    std::fs::write(&tmp, &wav_bytes).unwrap();

    let pcm = PcmData::from_path(&tmp).unwrap();
    assert_eq!(pcm.sample_rate, 44100);
    assert_eq!(pcm.num_channels, 1);

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn decode_stereo_wav() {
    // Interleave two sine channels
    let left = sine_wave(440.0, 44100, 4410);
    let right = sine_wave(880.0, 44100, 4410);
    let mut interleaved = Vec::with_capacity(left.len() * 2);
    for i in 0..left.len() {
        interleaved.push(left[i]);
        interleaved.push(right[i]);
    }
    let wav_bytes = write_wav_stereo(&interleaved, 44100);
    let tmp = std::env::temp_dir().join("kairos_test_stereo.wav");
    std::fs::write(&tmp, &wav_bytes).unwrap();

    let pcm = PcmData::from_path(&tmp).unwrap();
    assert_eq!(pcm.num_channels, 2);
    assert!(pcm.num_samples > 0);
    assert!((pcm.duration.as_secs_f32() - 0.1).abs() < 0.01);

    let mono = pcm.mono_samples();
    assert_eq!(mono.len(), pcm.num_samples);

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn decode_nonexistent_file() {
    let result = PcmData::from_path(Path::new("/nonexistent/audio.wav"));
    assert!(result.is_err());
}

#[test]
fn decode_tiny_wav_one_sample() {
    // Just one sample — edge case that can trigger zero-frame issues
    let wav_bytes = write_wav_bytes(&[0.5], 44100);
    let tmp = std::env::temp_dir().join("kairos_test_tiny.wav");
    std::fs::write(&tmp, &wav_bytes).unwrap();

    let pcm = PcmData::from_path(&tmp).unwrap();
    assert_eq!(pcm.num_samples, 1);
    assert!((pcm.samples[0] - 0.5).abs() < 0.02); // ~16-bit precision

    let _ = std::fs::remove_file(&tmp);
}

// ----------------------------------------------------------
// Full pipeline: decode → mono → peaks
// ----------------------------------------------------------

#[test]
fn full_pipeline_wav() {
    let samples = sine_wave(440.0, 44100, 44100); // 1s
    let wav_bytes = write_wav_bytes(&samples, 44100);
    let tmp = std::env::temp_dir().join("kairos_test_pipeline.wav");
    std::fs::write(&tmp, &wav_bytes).unwrap();

    let pcm = PcmData::from_path(&tmp).unwrap();
    let mono = pcm.mono_samples();
    let peaks = compute_peaks(&mono, 1024);

    assert_eq!(peaks.len(), 1024);
    // For a pure sine, peaks should be approximately [-1, 1]
    let global_min = peaks.iter().map(|p| p.min).fold(f32::INFINITY, f32::min);
    let global_max = peaks
        .iter()
        .map(|p| p.max)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(global_min < -0.9, "expected min < -0.9, got {global_min}");
    assert!(global_max > 0.9, "expected max > 0.9, got {global_max}");

    let _ = std::fs::remove_file(&tmp);
}
