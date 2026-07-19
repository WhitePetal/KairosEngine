use std::{cell::Cell, fs, sync::Arc, time::Instant};

use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::{AssetHandle, AssetsServer, asset::AudioExtAssetsSystem},
    audio::audio_ext::pcm::PcmData,
    kairos_editor::{
        Engine,
        ui::{Message, dialog::Dialog, inspector::Inspector, paths},
    },
    math,
};
use kira::sound::static_sound::StaticSoundHandle;

// ============================================================
// WaveformPeak — Unity-style amplitude bucket
// ============================================================

/// One bucket in the waveform overview: min and max amplitude within a time window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformPeak {
    pub min: f32,
    pub max: f32,
}

/// Compute waveform peaks from mono f32 PCM samples using min/max bucketing
/// (same approach as Unity's Audio Inspector waveform preview).
///
/// - `samples`: mono f32 PCM, normalized to `[-1, 1]`.
/// - `num_buckets`: typically matches the pixel width of the widget.
///
/// Returns one `WaveformPeak` per bucket.
pub fn compute_peaks(samples: &[f32], num_buckets: usize) -> Vec<WaveformPeak> {
    if samples.is_empty() || num_buckets == 0 {
        return Vec::new();
    }

    let bucket_size = (samples.len() / num_buckets).max(1);

    (0..num_buckets)
        .map(|i| {
            let start = i * bucket_size;
            if start >= samples.len() {
                return WaveformPeak { min: 0.0, max: 0.0 };
            }
            let end = ((i + 1) * bucket_size).min(samples.len());
            let slice = &samples[start..end];

            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            for &s in slice {
                if s < min {
                    min = s;
                }
                if s > max {
                    max = s;
                }
            }
            // Guard against empty slice (shouldn't happen, but be safe)
            if min == f32::INFINITY {
                min = 0.0;
                max = 0.0;
            }
            WaveformPeak { min, max }
        })
        .collect()
}

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

// ============================================================
// Style (loaded from TOML, matching project pattern)
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct AudioInspectorStyle {
    /// Height of the waveform panel in logical pixels.
    waveform_height: f32,
    /// Height of the spectrum panel in logical pixels.
    spectrum_height: f32,
    /// Number of vertical buckets for the waveform overview.
    waveform_buckets: usize,
    /// FFT window size (power of 2).
    fft_size: usize,
    /// Background color for the waveform area.
    waveform_bg: math::Color32,
    /// Waveform line color.
    waveform_color: math::Color32,
    /// Color for the played region overlay.
    waveform_played_color: math::Color32,
    /// Color for the playhead cursor.
    playhead_color: math::Color32,
    /// Spectrum bar color.
    spectrum_color: math::Color32,
    /// Grid line color.
    grid_color: math::Color32,
}

impl AudioInspectorStyle {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_str = fs::read_to_string(paths::PATH_AUDIO_INSPECTOR_STYLE).map_err(|e| {
            format!(
                "Load AudioInspector Style Failed, path: {}, error: {}",
                paths::PATH_AUDIO_INSPECTOR_STYLE,
                e
            )
        })?;
        let style: Self = toml::from_str(&style_str)
            .map_err(|e| format!("Deserialize AudioInspector Style Failed, error: {}", e))?;
        Ok(style)
    }
}

// ============================================================
// Model
// ============================================================

struct AudioInspectorModel {
    style: AudioInspectorStyle,
    audio_ext_handle: Arc<AssetHandle<AudioExtAssetsSystem>>,
    audio_handle: Option<StaticSoundHandle>,
    // ---- playback state (all plain fields, mutated via messages) ----
    /// Whether audio preview is currently playing.
    is_playing: bool,
    /// Current playback position in seconds (for UI display).
    playback_position: f32,
    /// Wall-clock instant when the current play segment started.
    play_start_instant: Option<Instant>,
    /// Position (seconds) where this play segment started (0 for fresh play, N for resume).
    play_start_position: f32,
    /// Accumulated playback time from previous play segments (used for pause/resume).
    play_accumulated: f32,
}

// ============================================================
// Inspector
// ============================================================

pub struct AudioInspector {
    model: AudioInspectorModel,
    /// Precomputed waveform peaks.
    peaks: Cell<Option<Vec<WaveformPeak>>>,
    /// Precomputed spectrum bins.
    spectrum_bins: Cell<Option<Vec<FrequencyBin>>>,
}

impl Inspector for AudioInspector {
    fn create(
        path: &std::path::Path,
        assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = AudioInspectorStyle::new()?;
        let asset_path = path.to_path_buf();
        let audio_ext_handle = assets_server.load(&asset_path);

        let model = AudioInspectorModel {
            style,
            audio_ext_handle,
            audio_handle: None,
            is_playing: false,
            playback_position: 0.0,
            play_start_instant: None,
            play_start_position: 0.0,
            play_accumulated: 0.0,
        };

        Ok(Self {
            model,
            peaks: Cell::new(None),
            spectrum_bins: Cell::new(None),
        })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        messager: &mut crate::kairos_editor::ui::Messager,
        assets_server: &AssetsServer,
    ) {
        let Some(pcm) = self.get_pcm(assets_server) else {
            ui.label("Audio is Loading...");
            return;
        };

        let mut peaks = self.peaks.take();
        if peaks.is_none() {
            let mono = pcm.mono_samples();
            peaks = Some(compute_peaks(&mono, self.model.style.waveform_buckets));
            let bins = Some(compute_spectrum(
                &mono,
                pcm.sample_rate,
                self.model.style.fft_size,
            ));
            self.spectrum_bins.set(bins);
        }
        self.peaks.set(peaks);

        // ---- info header ----
        ui.label(format!(
            "Sample Rate: {} Hz  |  Channels: {}  |  Duration: {:.2}s  |  Samples: {}",
            pcm.sample_rate,
            pcm.num_channels,
            pcm.duration.as_secs_f32(),
            pcm.num_samples
        ));

        ui.separator();

        // ---- waveform panel (with play button + playhead) ----
        self.draw_waveform(ui, pcm, messager);

        ui.separator();

        // ---- spectrum panel ----
        self.draw_spectrum(ui);

        if self.model.is_playing {
            messager.send(Message::AudioInspectorTick);
        }
    }

    fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
        self.stop_playback();
        None
    }
}

// ============================================================
// Public API — called from InspectorWindow / Context::handle()
// ============================================================

impl AudioInspector {
    /// Called when ToggleAudioPreview message is received.
    /// Has `&mut self` + `&mut Engine` → can start/stop kira playback.
    pub fn toggle_playback(&mut self, engine: &mut Engine) {
        if self.model.is_playing {
            self.pause();
        } else {
            self.play(engine);
        }
    }

    /// Called when SeekAudioPreview message is received.
    /// Seeks to `position` (seconds) and starts/resumes playback.
    pub fn seek_and_play(&mut self, engine: &mut Engine, position: f32) {
        // Clamp to valid range
        let Some(pcm) = self.get_pcm(&engine.assets_server) else {
            return;
        };
        let duration = pcm.duration.as_secs_f32();
        let position = position.clamp(0.0, duration);

        // Stop current playback if any
        self.stop_kira_handle();

        // Set accumulated to the seek target so play() resumes from there
        self.model.play_accumulated = position;
        self.model.playback_position = position;
        self.model.is_playing = false;

        // Start playback from the new position
        self.play(engine);
    }

    /// Stop playback and reset all state. Called when switching assets.
    pub fn stop_playback(&mut self) {
        self.stop_kira_handle();
        self.model.is_playing = false;
        self.model.playback_position = 0.0;
        self.model.play_start_instant = None;
        self.model.play_start_position = 0.0;
        self.model.play_accumulated = 0.0;
    }

    /// Called every frame by Context::handle() to update playback position.
    pub fn tick_playback(&mut self, assets_server: &mut AssetsServer) {
        let Some(instant) = self.model.play_start_instant else {
            return;
        };

        let elapsed = instant.elapsed().as_secs_f32();
        self.model.playback_position =
            self.model.play_start_position + self.model.play_accumulated + elapsed;

        // Check if past duration
        if let Some(pcm) = self.get_pcm(assets_server) {
            let duration = pcm.duration.as_secs_f32();
            if self.model.playback_position >= duration {
                self.stop_kira_handle();
                self.model.playback_position = 0.0;
                self.model.is_playing = false;
                self.model.play_start_instant = None;
                self.model.play_start_position = 0.0;
                self.model.play_accumulated = 0.0;
            }
        }
    }
}

// ============================================================
// Private — play / pause logic
// ============================================================

impl AudioInspector {
    fn get_pcm<'a>(&'a self, assets_server: &'a AssetsServer) -> Option<&'a PcmData> {
        let Some(audio_ext) = assets_server.get(&self.model.audio_ext_handle) else {
            return None;
        };
        let Some(pcm_handle) = &audio_ext.pcm else {
            return None;
        };
        assets_server.get(pcm_handle)
    }

    fn play(&mut self, engine: &mut Engine) {
        let Some(audio_ext) = engine.assets_server.get(&self.model.audio_ext_handle) else {
            return;
        };
        let Some(audio_handle) = &audio_ext.audio else {
            return;
        };
        let Some(audio) = engine.assets_server.get(audio_handle) else {
            return;
        };

        let mut sound_data = audio.sound_data.clone();

        // If resuming, set start_position to the current position
        let resume_pos = self.model.play_accumulated;
        if resume_pos > 0.0 {
            use kira::sound::PlaybackPosition;
            let settings = &mut sound_data.settings;
            settings.start_position = PlaybackPosition::Seconds(resume_pos as f64);
        }

        // Stop any existing playback before starting a new one
        self.stop_kira_handle();

        // Start playback
        let handle = engine.audio_engine.play_sound(sound_data);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return,
        };
        self.model.audio_handle = Some(handle);

        self.model.is_playing = true;
        self.model.play_start_instant = Some(Instant::now());
        self.model.play_start_position = 0.0;
        // play_accumulated keeps its value (non-zero when resuming)
    }

    fn pause(&mut self) {
        // Stop the kira handle
        self.stop_kira_handle();
        // Accumulate elapsed time
        if let Some(instant) = self.model.play_start_instant {
            self.model.play_accumulated += instant.elapsed().as_secs_f32();
        }
        self.model.play_start_instant = None;
        self.model.is_playing = false;
        // playback_position stays at current value for display
    }

    /// Stop the kira handle if one is active and clear it.
    fn stop_kira_handle(&mut self) {
        if let Some(ref mut handle) = self.model.audio_handle {
            handle.stop(kira::Tween::default());
        }
        self.model.audio_handle = None;
    }
}

// ============================================================
// Private — rendering
// ============================================================

impl AudioInspector {
    // ----------------------------------------------------------
    // Waveform rendering (with play button + playhead)
    // ----------------------------------------------------------

    fn draw_waveform(
        &self,
        ui: &mut egui::Ui,
        pcm: &PcmData,
        messager: &mut crate::kairos_editor::ui::Messager,
    ) {
        let style = &self.model.style;

        // ---- header: label + play/pause button ----
        ui.horizontal(|ui| {
            ui.label(RichText::new("Waveform").strong());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn_text = if self.model.is_playing { "⏸" } else { "▶" };
                if ui.button(btn_text).clicked() {
                    messager.send(crate::kairos_editor::ui::Message::AudioInspectorTogglePreview);
                }
            });
        });

        let desired_size = Vec2::new(ui.available_width(), style.waveform_height);
        let (resp, painter) = ui.allocate_painter(desired_size, egui::Sense::click_and_drag());
        let rect = resp.rect;

        // Background
        painter.rect_filled(rect, 0.0, style.waveform_bg);

        // Center line
        let mid_y = rect.center().y;
        painter.line_segment(
            [
                Pos2::new(rect.left(), mid_y),
                Pos2::new(rect.right(), mid_y),
            ],
            Stroke::new(1.0, style.grid_color),
        );

        // ---- played region overlay ----
        let duration = pcm.duration.as_secs_f32();

        if duration > 0.0 && self.model.playback_position > 0.0 {
            let played_x = rect.left() + (self.model.playback_position / duration) * rect.width();
            let played_rect = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(played_x, rect.bottom()),
            );
            painter.rect_filled(played_rect, 0.0, style.waveform_played_color);
        }

        // ---- waveform peaks ----
        let peaks_data = self.peaks.take();
        if let Some(peaks) = &peaks_data
            && !peaks.is_empty()
        {
            let bar_width = rect.width() / peaks.len() as f32;
            let half_h = rect.height() * 0.45;

            for (i, peak) in peaks.iter().enumerate() {
                let x = rect.left() + i as f32 * bar_width + bar_width * 0.5;
                let y_min = mid_y - peak.min * half_h;
                let y_max = mid_y - peak.max * half_h;
                painter.line_segment(
                    [Pos2::new(x, y_min), Pos2::new(x, y_max)],
                    Stroke::new(1.0, style.waveform_color),
                );
            }
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No waveform data",
                egui::FontId::proportional(14.0),
                Color32::GRAY,
            );
        }
        self.peaks.set(peaks_data);

        // ---- playhead cursor ----
        if duration > 0.0 {
            let playhead_x = rect.left()
                + (self.model.playback_position / duration).clamp(0.0, 1.0) * rect.width();

            // Vertical line
            painter.line_segment(
                [
                    Pos2::new(playhead_x, rect.top()),
                    Pos2::new(playhead_x, rect.bottom()),
                ],
                Stroke::new(2.0, style.playhead_color),
            );

            // Small triangle head at top
            let tri_size = 5.0;
            painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(playhead_x, rect.top() + tri_size),
                    Vec2::new(tri_size * 2.0, tri_size * 2.0),
                ),
                2.0,
                style.playhead_color,
            );
        }

        // ---- time axis labels ----
        if duration > 0.0 {
            painter.text(
                Pos2::new(rect.left() + 4.0, rect.bottom() - 14.0),
                egui::Align2::LEFT_BOTTOM,
                "0:00",
                egui::FontId::monospace(10.0),
                Color32::GRAY,
            );
            painter.text(
                Pos2::new(rect.right() - 4.0, rect.bottom() - 14.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("{:.0}:{:02.0}", duration / 60.0, duration % 60.0),
                egui::FontId::monospace(10.0),
                Color32::GRAY,
            );
        }

        // ---- click to seek ----

        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                let seek_time = t * duration;
                messager
                    .send(crate::kairos_editor::ui::Message::AudioInspectorSeekPreview(seek_time));
            }
        }
    }

    // ----------------------------------------------------------
    // Spectrum rendering
    // ----------------------------------------------------------

    fn draw_spectrum(&self, ui: &mut egui::Ui) {
        let bins_data = self.spectrum_bins.take();
        let Some(bins) = &bins_data else {
            return;
        };

        let style = &self.model.style;

        ui.label(RichText::new("Spectrum (Frequency Analysis)").strong());
        let desired_size = Vec2::new(ui.available_width(), style.spectrum_height);

        let (resp, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
        let rect = resp.rect;

        // Background
        painter.rect_filled(rect, 0.0, style.waveform_bg);

        if bins.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No spectrum data",
                egui::FontId::proportional(12.0),
                Color32::GRAY,
            );
        } else {
            // Draw spectrum bars
            let max_display_bins = 512usize;
            let step = (bins.len() / max_display_bins).max(1);
            let bar_width = rect.width() / (bins.len() / step) as f32;

            for (i, chunk) in bins.chunks(step).enumerate() {
                let mag = chunk.iter().map(|b| b.magnitude).fold(0.0f32, f32::max);
                let bar_h = mag * rect.height();
                let x = rect.left() + i as f32 * bar_width;
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(x, rect.bottom() - bar_h),
                        Vec2::new(bar_width.max(1.0), bar_h),
                    ),
                    0.0,
                    style.spectrum_color,
                );
            }

            // Frequency axis labels
            if let Some(first) = bins.first() {
                let max_freq = bins.last().map(|b| b.frequency).unwrap_or(22050.0);
                painter.text(
                    Pos2::new(rect.left() + 4.0, rect.bottom() - 14.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{}Hz", first.frequency as u32),
                    egui::FontId::monospace(10.0),
                    Color32::GRAY,
                );
                painter.text(
                    Pos2::new(rect.right() - 4.0, rect.bottom() - 14.0),
                    egui::Align2::RIGHT_BOTTOM,
                    if max_freq >= 1000.0 {
                        format!("{:.1}kHz", max_freq / 1000.0)
                    } else {
                        format!("{}Hz", max_freq as u32)
                    },
                    egui::FontId::monospace(10.0),
                    Color32::GRAY,
                );
            }
        }
        self.spectrum_bins.set(bins_data);
    }
}
