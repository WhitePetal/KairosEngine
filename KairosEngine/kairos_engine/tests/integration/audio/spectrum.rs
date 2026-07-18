use kairos_engine::audio::spectrum::*;

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
