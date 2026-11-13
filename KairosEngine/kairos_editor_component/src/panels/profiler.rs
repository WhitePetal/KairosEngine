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
//! There is deliberately no chart, flame graph or timeline yet: part 1 is
//! "record code as structured data, and show that data". The drawing phase will
//! reuse [`kairos_profile::hierarchy`] and the same session without changing the
//! capture side.

use std::sync::Arc;

use egui::{Context, Ui, WidgetText};
use kairos_editor_core::PanelId;
use kairos_editor_widgets::ui_image;
use kairos_profile::{
    FrameSnapshot, HierarchyRow, ProfileSession, Profiler, ScopeInfo, ThreadSnapshot, hierarchy,
};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

use super::PROFILER;

/// Tab-header artwork, loaded as a runtime file in [`Panel::init`].
const TAB_ICON: &str = "Preferences/Textures/tab_icons/profiler.png";

/// Hard cap on rendered rows. The underlying tree is still complete; this only
/// keeps a pathological capture from making one frame of UI expensive.
const MAX_TREE_ROWS: usize = 4_000;
const MAX_RAW_ROWS: usize = 4_000;

const RECORDING_COLOR: egui::Color32 = egui::Color32::from_rgb(224, 82, 82);

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
    tab: Tab,
    follow_latest: bool,
    selected_frame: usize,
    /// `None` aggregates all threads.
    thread: Option<u32>,
    cache_revision: u64,
    cache_frame: usize,
    cache_thread: Option<u32>,
    cached: Option<CachedFrame>,
}

impl Default for ProfilerPanel {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            follow_latest: true,
            selected_frame: 0,
            thread: None,
            cache_revision: u64::MAX,
            cache_frame: usize::MAX,
            cache_thread: None,
            cached: None,
        }
    }
}

impl Panel for ProfilerPanel {
    fn kind(&self) -> PanelId {
        PROFILER
    }

    fn icon(&self) -> Option<&str> {
        Some(TAB_ICON)
    }

    fn init(&mut self, ctx: &Context) {
        ui_image::preload(ctx, TAB_ICON);
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

            ui.separator();
            if recording {
                ui.colored_label(RECORDING_COLOR, "REC");
            }
            ui.label(format!("{} frames", session.frames.len()));
            if session.dropped_frames > 0 {
                ui.weak(format!("{} dropped", session.dropped_frames));
            }
            if session.discarded_passes > 0 {
                ui.weak(format!("{} discarded passes", session.discarded_passes));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak("Data view · charts next");
            });
        });

        refresh
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
                ui.label("Press Record, then interact with the editor.");
            }
            ui.add_space(8.0);
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

                        for row in cached.rows.iter().take(MAX_TREE_ROWS) {
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

                if cached.rows.len() > MAX_TREE_ROWS {
                    ui.weak(format!(
                        "Showing the first {MAX_TREE_ROWS} of {} rows.",
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
                                ui.colored_label(RECORDING_COLOR, warning);
                            }

                            egui::Grid::new(("profiler-raw-grid", thread.id))
                                .striped(true)
                                .num_columns(3)
                                .show(ui, |ui| {
                                    ui.strong("Sample");
                                    ui.strong("Start ms");
                                    ui.strong("Time ms");
                                    ui.end_row();

                                    for sample in thread.samples.iter().take(MAX_RAW_ROWS) {
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

                            if thread.samples.len() > MAX_RAW_ROWS {
                                ui.weak(format!(
                                    "Showing the first {MAX_RAW_ROWS} of {} samples.",
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
