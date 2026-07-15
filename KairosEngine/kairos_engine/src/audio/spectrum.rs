use rustfft::{FftPlanner, num_complex::Complex};

/// One frequency bin: frequency in Hz + magnitude (linear, 0.0–1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyBin {
    pub frequency: f32,
    pub magnitude: f32,
}

/// Compute the magnitude spectrum from a slice of mono f32 PCM samples.
///
/// - `samples`: mono PCM, normalized to `[-1, 1]`.
/// - `sample_rate`: sample rate in Hz.
/// - `fft_size`: FFT window size (power of 2, e.g. 2048). Larger = finer resolution.
///
/// Returns frequency bins up to Nyquist (`sample_rate / 2`).
/// Magnitudes are normalized so the max bin = 1.0.
pub fn compute_spectrum(samples: &[f32], sample_rate: u32, fft_size: usize) -> Vec<FrequencyBin> {
    let n = samples.len().min(fft_size);
    if n < 2 {
        // Need at least 2 samples for a meaningful spectrum (and to avoid
        // division-by-zero in the Hann window formula when n=1).
        return Vec::new();
    }

    // Apply Hann window
    let n_minus_1 = (n - 1) as f32;
    let windowed: Vec<f32> = samples[..n]
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / n_minus_1).cos());
            s * w
        })
        .collect();

    // Prepare FFT: zero-pad to fft_size
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);
    let mut buffer: Vec<Complex<f32>> = (0..fft_size)
        .map(|i| {
            if i < n {
                Complex::new(windowed[i], 0.0)
            } else {
                Complex::new(0.0, 0.0)
            }
        })
        .collect();

    fft.process(&mut buffer);

    // Only first half (up to Nyquist)
    let num_bins = fft_size / 2;
    let freq_resolution = sample_rate as f32 / fft_size as f32;

    let mut bins: Vec<FrequencyBin> = (0..num_bins)
        .map(|i| {
            let magnitude = (buffer[i].norm_sqr()).sqrt() / n as f32;
            FrequencyBin {
                frequency: i as f32 * freq_resolution,
                magnitude,
            }
        })
        .collect();

    // Normalize to 0..1
    let max_mag = bins
        .iter()
        .map(|b| b.magnitude)
        .fold(0.0f32, f32::max);
    if max_mag > 0.0 {
        for bin in &mut bins {
            bin.magnitude /= max_mag;
        }
    }

    bins
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

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
        let max_mag = bins
            .iter()
            .map(|b| b.magnitude)
            .fold(0.0f32, f32::max);
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
        assert!(sum < 0.01, "silence should have near-zero spectrum, sum={sum}");
    }
}
