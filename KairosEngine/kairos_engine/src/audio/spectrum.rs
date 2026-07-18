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
    let max_mag = bins.iter().map(|b| b.magnitude).fold(0.0f32, f32::max);
    if max_mag > 0.0 {
        for bin in &mut bins {
            bin.magnitude /= max_mag;
        }
    }

    bins
}


