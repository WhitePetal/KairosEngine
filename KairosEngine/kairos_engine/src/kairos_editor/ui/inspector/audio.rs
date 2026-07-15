use std::{cell::RefCell, fs, path::PathBuf};

use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::AssetsServer,
    audio::{
        audio::SerializedAudioAsset,
        spectrum::{FrequencyBin, compute_spectrum},
        waveform::{PcmData, WaveformPeak, compute_peaks},
    },
    kairos_editor::ui::{
        dialog::Dialog,
        inspector::Inspector,
        paths,
    },
};

// ============================================================
// Style (loaded from TOML, matching project pattern)
// ============================================================

#[derive(Debug, Serialize, Deserialize)]
struct AudioInspectorStyle {
    /// Height of the waveform panel in logical pixels.
    waveform_height: f32, // 120.0
    /// Height of the spectrum panel in logical pixels.
    spectrum_height: f32, // 100.0
    /// Number of vertical buckets for the waveform overview.
    waveform_buckets: usize, // 1024
    /// FFT window size (power of 2).
    fft_size: usize, // 2048
    /// Background color for the waveform area (RGBA hex).
    waveform_bg: String, // "#1E1E1E"
    /// Waveform line color (RGBA hex).
    waveform_color: String, // "#4FC3F7"
    /// Spectrum bar color (RGBA hex).
    spectrum_color: String, // "#81C784"
    /// Grid line color (RGBA hex).
    grid_color: String, // "#3A3A3A"
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
        let style: Self = toml::from_str(&style_str).map_err(|e| {
            format!("Deserialize AudioInspector Style Failed, error: {}", e)
        })?;
        Ok(style)
    }
}

impl Default for AudioInspectorStyle {
    fn default() -> Self {
        Self {
            waveform_height: 120.0,
            spectrum_height: 100.0,
            waveform_buckets: 1024,
            fft_size: 2048,
            waveform_bg: "#1E1E1E".into(),
            waveform_color: "#4FC3F7".into(),
            spectrum_color: "#81C784".into(),
            grid_color: "#3A3A3A".into(),
        }
    }
}

fn parse_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x1E);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x1E);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0x1E);
    Color32::from_rgb(r, g, b)
}

// ============================================================
// Model
// ============================================================

struct AudioInspectorModel {
    style: AudioInspectorStyle,
    /// Path to the .audio TOML file.
    #[allow(dead_code)]
    asset_path: PathBuf,
    /// Decoded PCM data.
    pcm_data: Option<PcmData>,
    /// Precomputed waveform peaks.
    peaks: Vec<WaveformPeak>,
    /// Error message if loading failed.
    load_error: Option<String>,
    /// Selected time range for FFT view: (start_seconds, end_seconds).
    /// None = show full spectrum.
    selected_range: Option<(f32, f32)>,
    /// Cached spectrum for the selected range.
    cached_spectrum: RefCell<Option<Vec<FrequencyBin>>>,
}

// ============================================================
// Inspector
// ============================================================

pub struct AudioInspector {
    model: AudioInspectorModel,
}

impl Inspector for AudioInspector {
    fn create(
        path: &std::path::Path,
        _assets_server: &mut AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let style = AudioInspectorStyle::new().unwrap_or_default();
        let asset_path = path.to_path_buf();

        let (pcm_data, peaks, load_error) = match Self::load_audio_data(&asset_path) {
            Ok(data) => {
                let mono = data.mono_samples();
                let peaks = compute_peaks(&mono, style.waveform_buckets);
                (Some(data), peaks, None)
            }
            Err(e) => (None, Vec::new(), Some(e.to_string())),
        };

        let model = AudioInspectorModel {
            style,
            asset_path,
            pcm_data,
            peaks,
            load_error,
            selected_range: None,
            cached_spectrum: RefCell::new(None),
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        _messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &AssetsServer,
    ) {
        ui.separator();

        // ---- info header ----
        if let Some(ref pcm) = self.model.pcm_data {
            ui.label(format!(
                "Sample Rate: {} Hz  |  Channels: {}  |  Duration: {:.2}s  |  Samples: {}",
                pcm.sample_rate,
                pcm.num_channels,
                pcm.duration.as_secs_f32(),
                pcm.num_samples
            ));
        }

        if let Some(ref err) = self.model.load_error {
            ui.colored_label(ui.visuals().error_fg_color, format!("Error: {err}"));
            return;
        }

        ui.separator();

        // ---- waveform panel ----
        self.draw_waveform(ui);

        ui.separator();

        // ---- spectrum panel ----
        self.draw_spectrum(ui);
    }

    fn on_exit(
        &self,
        _ctx: &egui::Context,
        _assets_server: &AssetsServer,
    ) -> Option<Box<dyn Dialog>> {
        None
    }
}

// ============================================================
// Private impl — loading & rendering
// ============================================================

impl AudioInspector {
    /// Read the `.audio` TOML, get the source audio path, decode to PcmData.
    fn load_audio_data(asset_path: &PathBuf) -> Result<PcmData, Box<dyn std::error::Error>> {
        let toml_str = fs::read_to_string(asset_path)?;
        let serialized: SerializedAudioAsset = toml::from_str(&toml_str)?;
        PcmData::from_path(&serialized.source_path)
    }

    // ----------------------------------------------------------
    // Waveform rendering
    // ----------------------------------------------------------

    fn draw_waveform(&self, ui: &mut egui::Ui) {
        let style = &self.model.style;

        ui.label(RichText::new("Waveform").strong());
        let desired_size = Vec2::new(ui.available_width(), style.waveform_height);

        let (resp, painter) = ui.allocate_painter(desired_size, egui::Sense::click_and_drag());
        let rect = resp.rect;

        // Background
        let bg = parse_color(&style.waveform_bg);
        painter.rect_filled(rect, 0.0, bg);

        // Center line
        let mid_y = rect.center().y;
        let grid = parse_color(&style.grid_color);
        painter.line_segment(
            [Pos2::new(rect.left(), mid_y), Pos2::new(rect.right(), mid_y)],
            Stroke::new(1.0, grid),
        );

        // Draw peaks
        if !self.model.peaks.is_empty() {
            let bar_width = rect.width() / self.model.peaks.len() as f32;
            let wave_color = parse_color(&style.waveform_color);
            let half_h = rect.height() * 0.45;

            for (i, peak) in self.model.peaks.iter().enumerate() {
                let x = rect.left() + i as f32 * bar_width + bar_width * 0.5;
                let y_min = mid_y - peak.min * half_h;
                let y_max = mid_y - peak.max * half_h;
                painter.line_segment(
                    [Pos2::new(x, y_min), Pos2::new(x, y_max)],
                    Stroke::new(1.0, wave_color),
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

        // Selection interaction (click & drag to select FFT range)
        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.hover_pos() {
                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                if let Some(ref pcm) = self.model.pcm_data {
                    let time = t * pcm.duration.as_secs_f32();
                    // Simple selection: 0.5s window centered on click
                    let half_window = 0.25;
                    let _start = (time - half_window).max(0.0);
                    let _end = (time + half_window).min(pcm.duration.as_secs_f32());
                    // We can't mutate &self in draw; selection is currently display-only
                    // In a production version, you'd send a Message or use Cell/RefCell
                }
            }
        }

        // Time axis labels
        if let Some(ref pcm) = self.model.pcm_data {
            let dur = pcm.duration.as_secs_f32();
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
                format!("{:.0}:{:02.0}", dur / 60.0, dur % 60.0),
                egui::FontId::monospace(10.0),
                Color32::GRAY,
            );
        }
    }

    // ----------------------------------------------------------
    // Spectrum rendering
    // ----------------------------------------------------------

    fn draw_spectrum(&self, ui: &mut egui::Ui) {
        let style = &self.model.style;

        ui.label(RichText::new("Spectrum (Frequency Analysis)").strong());
        let desired_size = Vec2::new(ui.available_width(), style.spectrum_height);

        let (resp, painter) = ui.allocate_painter(desired_size, egui::Sense::hover());
        let rect = resp.rect;

        // Background
        let bg = parse_color(&style.waveform_bg);
        painter.rect_filled(rect, 0.0, bg);

        // Compute or retrieve cached spectrum
        let bins = self.get_or_compute_spectrum();

        if bins.is_empty() {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Select a region in the waveform to view spectrum",
                egui::FontId::proportional(12.0),
                Color32::GRAY,
            );
            return;
        }

        // Draw spectrum bars (show up to ~512 bins for visual clarity)
        let max_display_bins = 512usize;
        let step = (bins.len() / max_display_bins).max(1);
        let bar_width = rect.width() / (bins.len() / step) as f32;
        let spec_color = parse_color(&style.spectrum_color);

        for (i, chunk) in bins.chunks(step).enumerate() {
            // Max magnitude within this chunk
            let mag = chunk.iter().map(|b| b.magnitude).fold(0.0f32, f32::max);
            let bar_h = mag * rect.height();
            let x = rect.left() + i as f32 * bar_width;
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x, rect.bottom() - bar_h),
                    Vec2::new(bar_width.max(1.0), bar_h),
                ),
                0.0,
                spec_color,
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

    fn get_or_compute_spectrum(&self) -> Vec<FrequencyBin> {
        let mut cache = self.model.cached_spectrum.borrow_mut();
        if let Some(ref cached) = *cache {
            return cached.clone();
        }

        let pcm = match &self.model.pcm_data {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mono = pcm.mono_samples();

        // Use selected range or full audio
        let (start_sample, end_sample) = if let Some((start_sec, end_sec)) = self.model.selected_range
        {
            let s = (start_sec * pcm.sample_rate as f32) as usize;
            let e = (end_sec * pcm.sample_rate as f32) as usize;
            (s.min(mono.len()), e.min(mono.len()).max(s + 1))
        } else {
            (0, mono.len())
        };

        if end_sample <= start_sample || start_sample >= mono.len() {
            return Vec::new();
        }

        let slice = &mono[start_sample..end_sample];
        let bins = compute_spectrum(slice, pcm.sample_rate, self.model.style.fft_size);
        *cache = Some(bins.clone());
        bins
    }
}
