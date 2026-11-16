//! Profiler panel: the data-only first phase of the Unity-style Profiler.
//!
//! The panel owns no capture data. It reads the structured
//! [`kairos_profile::ProfileSession`] through [`crate::HostView::profiler`], keeps
//! only view state (selected tab/frame/thread and a cached aggregate tree), and
//! draws three views over the same frame:
//!
//! * **Hierarchy** — Unity's aggregated view: marker, calls, total/self/max
//!   milliseconds, merged by call path;
//! * **Raw** — pre-order samples per thread with depth indentation;
//! * **JSON** — the serialized frame + marker table, copyable.
//!
//! **Save** writes the whole capture through [`Profiler::save_capture`] and
//! shows the absolute paths it wrote: `.json` (this crate's schema), `.trace.json`
//! (Perfetto / `chrome://tracing`) and `.puffin` (`puffin_viewer`). The panel
//! itself performs no IO and knows no file formats.
//!
//! There is deliberately no chart, flame graph or timeline yet: part 1 is
//! "record code as structured data, and show that data". The drawing phase will
//! reuse [`kairos_profile::hierarchy`] and the same session without changing the
//! capture side.

use std::sync::Arc;

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::ui_image;
use kairos_profile::{
    CaptureFiles, DEFAULT_CAPTURE_DIR, FrameSnapshot, HierarchyRow, ProfileSession, Profiler,
    ScopeInfo, ThreadSnapshot, hierarchy,
};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::panels::profiler::style::ProfilerStyle;

use super::PROFILER;

pub mod style;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Hierarchy,
    Raw,
    Json,
}

/// The cached reduction of one selected frame. Rebuilt when the capture
/// revision, the selected frame or the thread filter changes.
struct CachedFrame {
    frame: Arc<FrameSnapshot>,
    scopes: Vec<ScopeInfo>,
    rows: Vec<HierarchyRow>,
    /// Filled lazily when the JSON tab is first shown; cleared on rebuild.
    json: String,
}

pub(crate) struct ProfilerPanel {
    style: ProfilerStyle,
    tab: Tab,
    follow_latest: bool,
    selected_frame: usize,
    /// `None` aggregates all threads.
    thread: Option<u32>,
    cache_revision: u64,
    cache_frame: usize,
    cache_thread: Option<u32>,
    cached: Option<CachedFrame>,
    /// Result of the last Save click: the exact files written, or the error.
    /// Kept so the panel can show where the capture went.
    save: Option<Result<CaptureFiles, String>>,
}

impl Default for ProfilerPanel {
    fn default() -> Self {
        Self {
            style: ProfilerStyle::dark(),
            tab: Tab::default(),
            follow_latest: true,
            selected_frame: 0,
            thread: None,
            cache_revision: u64::MAX,
            cache_frame: usize::MAX,
            cache_thread: None,
            cached: None,
            save: None,
        }
    }
}

impl Panel for ProfilerPanel {
    fn kind(&self) -> PanelId {
        PROFILER
    }

    fn icon(&self) -> Option<&str> {
        Some(&self.style.icons.tab)
    }

    fn init(&mut self, ctx: &Context) {
        match StyleFile::<ProfilerStyle>::load_or_default() {
            Ok(file) => {
                self.style = if ctx.theme() == egui::Theme::Light {
                    file.light
                } else {
                    file.dark
                };
            }
            Err(error) => {
                kairos_log::warn!(target: "kairos_editor", "profiler style: {error}");
            }
        }

        for path in self.style.icons.all() {
            ui_image::preload(ctx, path);
        }
    }

    fn title(&self, _frame: &Frame) -> WidgetText {
        "Profiler".into()
    }

    fn ui(&mut self, ui: &mut Ui, frame: &Frame, _out: &mut UiOut) {
        let Some(profiler) = frame.host.profiler() else {
            ui.weak("The host does not expose a profiler.");
            return;
        };

        let session = profiler.session();
        // A Record/Clear click invalidates the session snapshot above; skip the
        // rest of this pass rather than drawing one frame of stale data.
        if self.toolbar(ui, profiler, &session) {
            return;
        }

        if session.frames.is_empty() {
            self.empty_state(ui, profiler);
            return;
        }

        self.frame_selector(ui, &session);
        self.ensure_cache(&session);
        ui.separator();

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Hierarchy, "Hierarchy");
            ui.selectable_value(&mut self.tab, Tab::Raw, "Raw");
            ui.selectable_value(&mut self.tab, Tab::Json, "JSON");
            ui.separator();
            if let Some(cached) = &self.cached {
                ui.weak(format!(
                    "Frame {} · {} samples · {} threads",
                    self.selected_frame + 1,
                    cached.frame.sample_count(),
                    cached.frame.threads.len(),
                ));
            }
        });
        ui.add_space(2.0);

        match self.tab {
            Tab::Hierarchy => self.hierarchy_view(ui),
            Tab::Raw => self.raw_view(ui),
            Tab::Json => self.json_view(ui, profiler),
        }
    }
}

impl ProfilerPanel {
    /// Draw capture controls. Returns `true` when the session changed underneath
    /// this pass and the caller should stop drawing the previous snapshot.
    fn toolbar(&mut self, ui: &mut Ui, profiler: &Profiler, session: &ProfileSession) -> bool {
        let mut refresh = false;

        ui.horizontal_wrapped(|ui| {
            let recording = profiler.is_recording();
            let label = if recording { "■ Stop" } else { "● Record" };
            if ui.selectable_label(recording, label).clicked() {
                profiler.set_recording(!recording);
                self.reset_view();
                refresh = true;
            }
            if ui.button("Clear").clicked() {
                profiler.clear();
                self.reset_view();
                refresh = true;
            }
            if ui
                .add_enabled(!session.frames.is_empty(), egui::Button::new("Save"))
                .on_hover_text(format!(
                    "Write the whole capture to {DEFAULT_CAPTURE_DIR}/ as JSON, Chrome trace and .puffin"
                ))
                .on_disabled_hover_text("Nothing captured yet")
                .clicked()
            {
                // The panel never touches the filesystem itself: `kairos_profile`
                // owns the formats and the paths, and hands back exactly what it
                // wrote so the panel can show it.
                self.save = Some(
                    profiler
                        .save_capture(DEFAULT_CAPTURE_DIR)
                        .map_err(|error| error.to_string()),
                );
            }

            ui.separator();
            if recording {
                ui.colored_label(self.style.recording_color.to_egui(), "REC");
            }
            ui.label(format!("{} frames", session.frames.len()));
            if session.dropped_frames > 0 {
                ui.weak(format!("{} dropped", session.dropped_frames));
            }
            if session.discarded_passes > 0 {
                ui.weak(format!("{} discarded passes", session.discarded_passes));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(format!("Captures → {DEFAULT_CAPTURE_DIR}/"));
            });
        });

        self.save_status(ui);
        refresh
    }

    /// Show where the last Save went (or why it failed).
    fn save_status(&self, ui: &mut Ui) {
        let Some(result) = &self.save else {
            return;
        };

        match result {
            Ok(files) => {
                ui.horizontal_wrapped(|ui| {
                    ui.weak("Saved");
                    if ui.small_button("Copy paths").clicked() {
                        ui.ctx().copy_text(files_summary(files));
                    }
                    ui.weak("json · trace.json · puffin");
                });
                ui.add(
                    egui::Label::new(egui::RichText::new(files_summary(files)).monospace()).wrap(),
                );
            }
            Err(error) => {
                let color = ui.visuals().error_fg_color;
                ui.colored_label(color, format!("Save failed: {error}"));
            }
        }
    }

    fn empty_state(&self, ui: &mut Ui, profiler: &Profiler) {
        ui.add_space(32.0);
        ui.vertical_centered(|ui| {
            if profiler.is_recording() {
                ui.spinner();
                ui.label("Recording — waiting for the first frame…");
                ui.weak("The first frame appears after the next UI pass is flushed.");
            } else {
                ui.heading("Profiler");
                ui.label("No frames captured.");
                ui.label("Press Record; the host keeps ticking, so frames arrive even when you leave the window alone.");
            }
            ui.add_space(8.0);
            ui.weak(format!(
                "Save writes JSON, Chrome trace and .puffin under {DEFAULT_CAPTURE_DIR}/."
            ));
            ui.weak("Part 1 exposes the structured capture: hierarchy, raw samples and JSON.");
        });
    }

    fn frame_selector(&mut self, ui: &mut Ui, session: &ProfileSession) {
        let last = session.frames.len().saturating_sub(1);
        if self.follow_latest || self.selected_frame > last {
            self.selected_frame = last;
        }

        ui.horizontal_wrapped(|ui| {
            let mut latest = self.follow_latest;
            if ui.checkbox(&mut latest, "Latest").changed() {
                self.follow_latest = latest;
                if latest {
                    self.selected_frame = last;
                }
            }

            let mut selected = self.selected_frame;
            let response = ui.add(
                egui::DragValue::new(&mut selected)
                    .range(0..=last)
                    .speed(0.25),
            );
            if response.changed() {
                self.selected_frame = selected.min(last);
                self.follow_latest = false;
            }

            let frame = &session.frames[self.selected_frame];
            ui.label(format!(
                "Frame {}/{} · {:.3} ms",
                self.selected_frame + 1,
                session.frames.len(),
                frame.duration_ns() as f64 / 1_000_000.0,
            ));

            ui.separator();

            if self.thread.is_some_and(|id| frame.thread(id).is_none()) {
                self.thread = None;
            }
            let selected_text = self
                .thread
                .and_then(|id| frame.thread(id))
                .map(thread_label)
                .unwrap_or_else(|| "All threads".to_owned());
            egui::ComboBox::from_id_salt("profiler-thread")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.thread, None, "All threads");
                    for thread in &frame.threads {
                        ui.selectable_value(
                            &mut self.thread,
                            Some(thread.id),
                            thread_label(thread),
                        );
                    }
                });
        });
    }

    fn ensure_cache(&mut self, session: &ProfileSession) {
        let index = self
            .selected_frame
            .min(session.frames.len().saturating_sub(1));
        let stale = self.cached.is_none()
            || self.cache_revision != session.revision
            || self.cache_frame != index
            || self.cache_thread != self.thread;
        if !stale {
            return;
        }

        let frame = Arc::clone(&session.frames[index]);
        let rows = hierarchy(&frame, self.thread);
        self.cached = Some(CachedFrame {
            frame,
            scopes: session.scopes.clone(),
            rows,
            json: String::new(),
        });
        self.cache_revision = session.revision;
        self.cache_frame = index;
        self.cache_thread = self.thread;
    }

    fn reset_view(&mut self) {
        self.cached = None;
        self.cache_revision = u64::MAX;
        self.cache_frame = usize::MAX;
        self.cache_thread = None;
        self.selected_frame = 0;
        self.follow_latest = true;
        self.thread = None;
        // A new capture invalidates the previously written files' contents.
        self.save = None;
    }

    fn hierarchy_view(&self, ui: &mut Ui) {
        let Some(cached) = &self.cached else {
            return;
        };
        if cached.rows.is_empty() {
            ui.weak("No samples in this frame.");
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("profiler-hierarchy")
                    .striped(true)
                    .num_columns(5)
                    .min_col_width(52.0)
                    .show(ui, |ui| {
                        ui.strong("Marker");
                        ui.strong("Calls");
                        ui.strong("Total ms");
                        ui.strong("Self ms");
                        ui.strong("Max ms");
                        ui.end_row();

                        for row in cached.rows.iter().take(self.style.max_tree_rows) {
                            let scope = scope_info(&cached.scopes, row.scope);
                            let name = scope.map_or("<unknown scope>", |scope| scope.name.as_str());
                            let label = match &row.data {
                                Some(data) => format!("{name}  [{data}]"),
                                None => name.to_owned(),
                            };

                            ui.horizontal(|ui| {
                                ui.add_space(row.depth as f32 * 14.0);
                                let response = ui.label(label);
                                if let Some(scope) =
                                    scope.filter(|scope| !scope.location.is_empty())
                                {
                                    response.on_hover_text(scope.location.clone());
                                }
                            });
                            ui.monospace(row.calls.to_string());
                            ui.monospace(format_ms(row.total_ns));
                            ui.monospace(format_ms(row.self_ns));
                            ui.monospace(format_ms(row.max_ns));
                            ui.end_row();
                        }
                    });

                if cached.rows.len() > self.style.max_tree_rows {
                    let max_tree_rows = self.style.max_tree_rows;
                    ui.weak(format!(
                        "Showing the first {max_tree_rows} of {} rows.",
                        cached.rows.len()
                    ));
                }
            });
    }

    fn raw_view(&self, ui: &mut Ui) {
        let Some(cached) = &self.cached else {
            return;
        };

        let mut rendered = 0usize;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for thread in &cached.frame.threads {
                    if self.thread.is_some_and(|id| id != thread.id) {
                        continue;
                    }

                    let title = format!(
                        "{} · {} samples{}",
                        thread_label(thread),
                        thread.samples.len(),
                        if thread.warning.is_some() {
                            " · warning"
                        } else {
                            ""
                        },
                    );
                    egui::CollapsingHeader::new(title)
                        .id_salt(("profiler-raw-thread", thread.id))
                        .default_open(true)
                        .show(ui, |ui| {
                            if let Some(warning) = &thread.warning {
                                ui.colored_label(self.style.recording_color.to_egui(), warning);
                            }

                            egui::Grid::new(("profiler-raw-grid", thread.id))
                                .striped(true)
                                .num_columns(3)
                                .show(ui, |ui| {
                                    ui.strong("Sample");
                                    ui.strong("Start ms");
                                    ui.strong("Time ms");
                                    ui.end_row();

                                    for sample in
                                        thread.samples.iter().take(self.style.max_raw_rows)
                                    {
                                        rendered += 1;
                                        let scope = scope_info(&cached.scopes, sample.scope);
                                        let name = scope
                                            .map_or("<unknown scope>", |scope| scope.name.as_str());
                                        let mut label = format!(
                                            "{}{}",
                                            "  ".repeat(sample.depth as usize),
                                            name
                                        );
                                        if let Some(data) = &sample.data {
                                            label.push_str("  [");
                                            label.push_str(data);
                                            label.push(']');
                                        }

                                        let response = ui.monospace(label);
                                        if let Some(scope) =
                                            scope.filter(|scope| !scope.location.is_empty())
                                        {
                                            response.on_hover_text(scope.location.clone());
                                        }
                                        ui.monospace(format_ms(sample.start_ns));
                                        ui.monospace(format_ms(sample.duration_ns));
                                        ui.end_row();
                                    }
                                });

                            if thread.samples.len() > self.style.max_raw_rows {
                                let max_raw_rows = self.style.max_raw_rows;
                                ui.weak(format!(
                                    "Showing the first {max_raw_rows} of {} samples.",
                                    thread.samples.len()
                                ));
                            }
                        });
                }
            });

        if rendered == 0 {
            ui.weak("No samples for this thread.");
        }
    }

    fn json_view(&mut self, ui: &mut Ui, profiler: &Profiler) {
        let Some(cached) = &mut self.cached else {
            return;
        };
        if cached.json.is_empty() {
            cached.json = profiler
                .frame_json(self.cache_frame)
                .unwrap_or_else(|| "{}".to_owned());
        }

        ui.horizontal(|ui| {
            if ui.button("Copy JSON").clicked() {
                ui.ctx().copy_text(cached.json.clone());
            }
            ui.weak(format!("{} bytes", cached.json.len()));
            ui.weak("schema v1");
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.monospace(&cached.json);
            });
    }
}

fn scope_info(scopes: &[ScopeInfo], id: u32) -> Option<&ScopeInfo> {
    scopes
        .binary_search_by_key(&id, |scope| scope.id)
        .ok()
        .map(|index| &scopes[index])
}

fn format_ms(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1_000_000.0)
}

fn thread_label(thread: &ThreadSnapshot) -> String {
    if thread.name.is_empty() {
        format!("Thread {}", thread.id)
    } else {
        format!("{} (#{})", thread.name, thread.id)
    }
}

/// The three saved paths, one per line, made absolute for display.
///
/// `save_capture` returns the paths it was given (relative to the process
/// working directory), which is correct for IO but not for "where did my
/// capture go". Canonicalising here answers that question with a path the user
/// can paste into a file manager; if the file vanished in between, the joined
/// path is still better than nothing.
fn files_summary(files: &CaptureFiles) -> String {
    let path = |path: &std::path::Path| {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string()
    };
    format!(
        "{}\n{}\n{}",
        path(&files.json),
        path(&files.chrome_trace),
        path(&files.puffin)
    )
}
