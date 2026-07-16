use std::{fs, path::PathBuf, time::Instant};

use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use serde::{Deserialize, Serialize};

use crate::{
    asset_loader::assets::AssetsServer,
    audio::{
        audio::SerializedAudioAsset,
        spectrum::{FrequencyBin, compute_spectrum},
        waveform::{PcmData, WaveformPeak, compute_peaks},
    },
    kairos_editor::{
        Engine,
        ui::{dialog::Dialog, inspector::Inspector, paths},
    },
    math,
};
use kira::sound::static_sound::StaticSoundHandle;

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
    /// Path to the .audio TOML file.
    asset_path: PathBuf,
    /// Decoded PCM data.
    pcm_data: Option<PcmData>,
    /// Precomputed spectrum bins.
    spectrum_bins: Option<Vec<FrequencyBin>>,
    /// Precomputed waveform peaks.
    peaks: Vec<WaveformPeak>,
    /// Error message if loading failed.
    load_error: Option<String>,

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
    /// Kira handle for the currently playing sound. `None` when not playing.
    /// Stored so we can explicitly `.stop()` on pause.
    play_handle: Option<StaticSoundHandle>,
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
        let style = AudioInspectorStyle::new()?;
        let asset_path = path.to_path_buf();

        let (pcm_data, peaks, spectrum_bins, load_error) = match Self::load_audio_data(&asset_path)
        {
            Ok(pcm) => {
                let mono = pcm.mono_samples();
                let peaks = compute_peaks(&mono, style.waveform_buckets);
                let slice = &mono[..];
                let bins = compute_spectrum(slice, pcm.sample_rate, style.fft_size);
                (Some(pcm), peaks, Some(bins), None)
            }
            Err(e) => (None, Vec::new(), None, Some(e.to_string())),
        };

        let model = AudioInspectorModel {
            style,
            asset_path,
            pcm_data,
            spectrum_bins,
            peaks,
            load_error,
            is_playing: false,
            playback_position: 0.0,
            play_start_instant: None,
            play_start_position: 0.0,
            play_accumulated: 0.0,
            play_handle: None,
        };

        Ok(Self { model })
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        messager: &mut crate::kairos_editor::ui::Messager,
        _assets_server: &AssetsServer,
    ) {
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

        // ---- waveform panel (with play button + playhead) ----
        self.draw_waveform(ui, messager);

        ui.separator();

        // ---- spectrum panel ----
        self.draw_spectrum(ui);
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
        if self.model.pcm_data.is_none() {
            return;
        }

        if self.model.is_playing {
            self.pause();
        } else {
            self.play(engine);
        }
    }

    /// Called when SeekAudioPreview message is received.
    /// Seeks to `position` (seconds) and starts/resumes playback.
    pub fn seek_and_play(&mut self, engine: &mut Engine, position: f32) {
        if self.model.pcm_data.is_none() {
            return;
        }
        // Clamp to valid range
        let duration = self.model.pcm_data.as_ref().unwrap().duration.as_secs_f32();
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
    pub fn tick_playback(&mut self) {
        if !self.model.is_playing {
            return;
        }

        let Some(instant) = self.model.play_start_instant else {
            return;
        };

        let elapsed = instant.elapsed().as_secs_f32();
        self.model.playback_position =
            self.model.play_start_position + self.model.play_accumulated + elapsed;

        // Check if past duration
        if let Some(ref pcm) = self.model.pcm_data {
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
    fn play(&mut self, engine: &mut Engine) {
        if self.model.pcm_data.is_none() {
            return;
        };

        // Load the audio asset and play via kira
        let toml_str = match fs::read_to_string(&self.model.asset_path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let serialized: SerializedAudioAsset = match toml::from_str(&toml_str) {
            Ok(a) => a,
            Err(_) => return,
        };

        let bytes = match std::fs::read(&serialized.source_path) {
            Ok(b) => b,
            Err(_) => return,
        };

        let sound_data = match kira::sound::static_sound::StaticSoundData::from_cursor(
            std::io::Cursor::new(bytes),
        ) {
            Ok(d) => d,
            Err(_) => return,
        };

        // If resuming, set start_position to the current position
        let mut sound_data = sound_data;
        let resume_pos = self.model.play_accumulated;
        if resume_pos > 0.0 {
            use kira::sound::PlaybackPosition;
            let mut settings = sound_data.settings;
            settings.start_position = PlaybackPosition::Seconds(resume_pos as f64);
            sound_data.settings = settings;
        }

        // Stop any existing playback before starting a new one
        self.stop_kira_handle();

        // Start playback
        let handle = engine.audio_engine.play_sound(sound_data);
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return,
        };
        self.model.play_handle = Some(handle);

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
        if let Some(ref mut handle) = self.model.play_handle {
            handle.stop(kira::Tween::default());
        }
        self.model.play_handle = None;
    }
}

// ============================================================
// Private — rendering
// ============================================================

impl AudioInspector {
    /// Read the `.audio` TOML, get the source audio path, decode to PcmData.
    fn load_audio_data(asset_path: &PathBuf) -> Result<PcmData, Box<dyn std::error::Error>> {
        let toml_str = fs::read_to_string(asset_path)?;
        let serialized: SerializedAudioAsset = toml::from_str(&toml_str)?;
        PcmData::from_path(&serialized.source_path)
    }

    // ----------------------------------------------------------
    // Waveform rendering (with play button + playhead)
    // ----------------------------------------------------------

    fn draw_waveform(&self, ui: &mut egui::Ui, messager: &mut crate::kairos_editor::ui::Messager) {
        let style = &self.model.style;

        // ---- header: label + play/pause button ----
        ui.horizontal(|ui| {
            ui.label(RichText::new("Waveform").strong());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn_text = if self.model.is_playing { "⏸" } else { "▶" };
                if ui.button(btn_text).clicked() {
                    messager.send(crate::kairos_editor::ui::Message::ToggleAudioPreview);
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
        let duration = self
            .model
            .pcm_data
            .as_ref()
            .map(|p| p.duration.as_secs_f32())
            .unwrap_or(0.0);

        if duration > 0.0 && self.model.playback_position > 0.0 {
            let played_x = rect.left() + (self.model.playback_position / duration) * rect.width();
            let played_rect = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top()),
                Pos2::new(played_x, rect.bottom()),
            );
            painter.rect_filled(played_rect, 0.0, style.waveform_played_color);
        }

        // ---- waveform peaks ----
        if !self.model.peaks.is_empty() {
            let bar_width = rect.width() / self.model.peaks.len() as f32;
            let half_h = rect.height() * 0.45;

            for (i, peak) in self.model.peaks.iter().enumerate() {
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
                messager.send(crate::kairos_editor::ui::Message::SeekAudioPreview(
                    seek_time,
                ));
            }
        }
    }

    // ----------------------------------------------------------
    // Spectrum rendering
    // ----------------------------------------------------------

    fn draw_spectrum(&self, ui: &mut egui::Ui) {
        let Some(bins) = &self.model.spectrum_bins else {
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
            return;
        }

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
}
