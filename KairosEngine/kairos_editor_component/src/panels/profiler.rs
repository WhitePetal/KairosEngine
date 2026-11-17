//! Profiler panel: Unity's Profiler window over the structured capture.
//!
//! The panel owns no capture data. It reads the
//! [`kairos_profile::ProfileSession`] through [`crate::HostView::profiler`], keeps
//! only view state (recording flag, selected frame, live-follow) and draws three
//! bands, top to bottom, the way Unity lays out its Profiler window:
//!
//! * **toolbar** — record toggle, frame transport, `Frame: n / m`, Clear,
//!   separated into segments by 1 px rules;
//! * **module area** — a CPU row (category legend + a stacked frame-history
//!   chart with Unity's 30/60 FPS reference lines) and a GPU row (header only:
//!   the capture does not record GPU timings);
//! * **timeline** — header (view selector, Live toggle, CPU/GPU summary), a time
//!   ruler, then one sample track per thread.
//!
//! Layout, colours and measured geometry are [`ProfilerStyle`] tokens loaded
//! from `Preferences/Styles/Profiler.ron`.
//!
//! # Categories
//!
//! The chart stacks the engine modules (`kairos_profile::Category`): Rendering,
//! Scripts, Physics, Audio, Editor, VSync and Others. A marker's category comes
//! from `Category::of`, a rule table over the marker's name and source crate —
//! puffin records no category of its own.
//!
//! Core-module systems (physics, audio, rendering, editor) fold into their
//! module's bucket: they never get a legend row of their own. Everything else is
//! `Scripts`, and the legend refines it into one row per ECS system — the
//! executor opens a `Kairos.System` scope per run and names the system in the
//! scope's data. Scheduler wrappers (`Kairos.Stage`, `Kairos.Engine.Frame`)
//! contribute no time of their own: what they measure is their children's work,
//! counted where it ran, and the remainder becomes `Others`.
//!
//! # Not implemented yet (the seams are marked `TODO(profiler)` in the code)
//!
//! * module selection (`Profiler Modules`) and the Hierarchy / Raw / Call Stacks
//!   view selector — only the timeline view is rebuilt;
//! * thread grouping (Unity's JOB / LOADING / SCRIPTING rows), timeline pan/zoom
//!   and the bottom scrollbar;
//! * the full per-system ranking: the legend only has the rows the module area
//!   leaves free, so it shows the top few and `+N more`; a details pane (Unity's
//!   Hierarchy view) is where the whole list belongs;
//! * GPU timings, per-marker colours (the timeline still indexes the palette by
//!   scope id) and sample selection / hover details;
//! * a pinned header: the panel is inside the dock's scroll area, so a long
//!   thread list scrolls the toolbar and module area away with it (Unity keeps
//!   them fixed; that needs an inner scroll area for the tracks only).
//!
//! The data-phase views that predate the rewrite (hierarchy / raw / JSON) are
//! still in this file but are no longer wired into [`Panel::ui`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::scroll_area::ScrollBarVisibility;
use egui::{
    Align2, Context, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetText,
};
use kairos_editor_core::PanelId;
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;
use kairos_profile::profiling::puffin::{self, GlobalFrameView, MergeScope, NanoSecond, ScopeCollection, StreamInfo, ThreadInfo, UnpackedFrameData};
use kairos_profile::{
    CaptureFiles, Category, FrameSnapshot, HierarchyRow, ProfileSession, Profiler, SYSTEM_MARKER,
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
    /// The selected frame's Scripts systems by inclusive time, highest first.
    /// This is the legend's second section: Unity's `Scripts` category,
    /// refined into one row per system.
    systems: Vec<(String, u64)>,
    /// Filled lazily when the JSON tab is first shown; cleared on rebuild.
    json: String,
}

pub struct Streams {
    streams: Vec<Arc<StreamInfo>>,
    merged_scopes: Vec<MergeScope<'static>>,
    max_depth: usize,
}

impl Streams {
    fn new(scope_collection: &ScopeCollection, frames: &[Arc<UnpackedFrameData>], thread_info: &ThreadInfo) -> Self {
        let mut streams = vec![];
        for frame in frames {
            if let Some(stream_info) = frame.thread_streams.get(thread_info) {
                streams.push(stream_info.clone());
            }
        }
        let merges = puffin::merge_scopes_for_thread(scope_collection, frames, thread_info).unwrap();
        let merges = merges.into_iter().map(|ms| ms.into_owned()).collect();

        let mut max_depth = 0;
        for stream_info in &streams {
            max_depth = stream_info.depth.max(max_depth);
        }

        Self {
            streams,
            merged_scopes: merges,
            max_depth,
        }
    }
}

pub struct SelectedFrames {
    pub frames: vec1::Vec1<Arc<UnpackedFrameData>>,
    pub raw_range_ns: (NanoSecond, NanoSecond),
    pub merged_range_ns: (NanoSecond, NanoSecond),
    pub threads: BTreeMap<ThreadInfo, Streams>,
}

impl SelectedFrames {
    fn try_from_iter(scope_collection: &ScopeCollection, frames: impl Iterator<Item = Arc<UnpackedFrameData>>) -> Option<Self> {
        let mut it = frames;
        let first = it.next()?;
        let mut frames = vec1::Vec1::new(first);
        frames.extend(it);

        Some(Self::from_vec1(scope_collection, frames))
    }

    fn from_vec1(scope_collection: &ScopeCollection, mut frames: vec1::Vec1<Arc<UnpackedFrameData>>) -> Self {
        frames.sort_by_key(|f| f.frame_index());
        frames.dedup_by_key(|f| f.frame_index());

        let mut threads = BTreeSet::new();
        for frame in &frames {
            for ti in frame.thread_streams.keys() {
                threads.insert(ti.clone());
            }
        }

        let threads = threads
            .iter()
            .map(|ti| (ti.clone(), Streams::new(scope_collection, &frames, ti)))
            .collect::<BTreeMap<ThreadInfo, Streams>>();

        let mut merged_min_ns = NanoSecond::MAX;
        let mut merged_max_ns = NanoSecond::MIN;
        for stream in threads.values() {
            for scope in &stream.merged_scopes {
                let scope_start = scope.relative_start_ns;
                let scope_end = scope_start + scope.duration_per_frame_ns;
                merged_min_ns = merged_min_ns.min(scope_start);
                merged_max_ns = merged_max_ns.max(scope_end);
            }
        }

        let raw_range_ns = (frames.first().range_ns().0, frames.last().range_ns().1);

        Self {
            frames,
            raw_range_ns,
            merged_range_ns: (merged_min_ns, merged_max_ns),
            threads,
        }
    }
}

struct FlameGraphInfo<'a> {
    ctx: egui::Context,
    canvas: Rect,
    response: Response,
    painter: egui::Painter,
    text_height: f32,
    start_ns: NanoSecond,
    stop_ns: NanoSecond,
    num_frames: usize,
    layer_id: egui::LayerId,
    font_id: FontId,
    scope_collection: &'a ScopeCollection,
}

pub(crate) struct ProfilerPanel {
    style: ProfilerStyle,
    global_frame_view: GlobalFrameView,
    recording: bool,
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
            global_frame_view: GlobalFrameView::default(),
            recording: false,
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

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
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
        self.recording = profiler.is_recording();
        self.follow_displayed_frame(&session);
        if session.frames.is_empty() {
            self.cached = None;
        } else {
            self.ensure_cache(&session);
        }

        // The panel is measured chrome: Unity's rows are flush and the gaps
        // between sections are rules, not item spacing.
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

        // Unity's window, top to bottom: toolbar, module area, timeline.
        self.main_toolbar(ui, profiler, &session);
        self.module_area(ui, &session);
        self.timeline(ui, &session);

        // profiler.set_recording(true);

        // let mut frame_view = self.global_frame_view.lock();
        // let latest = frame_view
        //     .latest_frames(1)
        //     .map(|frame| frame.unpacked())
        //     .filter_map(|unpacked| unpacked.ok());

        // let scope_collection = frame_view.scope_collection();
        // let frames = SelectedFrames::try_from_iter(scope_collection, latest);

        // let Some(frames) = frames else {
        //     return;
        // };

        // egui::containers::Frame::canvas(ui.style()).show(ui, |ui| {
        //     ScrollArea::vertical().show(ui, |ui| {
        //         let mut canvas = ui.available_rect_before_wrap();
        //         canvas.max.y = f32::INFINITY;
        //         let response = ui.interact(canvas, ui.id().with("canvas"), Sense::click_and_drag());

        //         let (min_ns, max_ns) = frames.raw_range_ns;

        //         let info = FlameGraphInfo {
        //             ctx: ui.ctx().clone(),
        //             canvas,
        //             response,
        //             painter: ui.painter_at(canvas),
        //             text_height: 15.0,
        //             start_ns: min_ns,
        //             stop_ns: max_ns,
        //             num_frames: frames.frames.len(),
        //             layer_id: ui.layer_id(),
        //             font_id: egui::TextStyle::Body.resolve(ui.style()),
        //             scope_collection
        //         };

        //         let mut cursor_y = info.canvas.top();
        //         let threads = frames.threads.keys().cloned().collect::<Vec<ThreadInfo>>();
        //         for thread_info in threads {
        //             let galley = info.ctx.fonts_mut(|f| {
        //                 f.layout_no_wrap(
        //                     thread_info.name.clone(), info.font_id.clone(), egui::Color32::PLACEHOLDER)
        //             });

        //             cursor_y += 4.0;
        //             let text_pos = pos2(info.canvas.min.x, cursor_y);
        //             let rect = Rect::from_min_size(text_pos, galley.size());
        //             info.painter.rect_filled(rect.expand(2.0), 0.0, egui::Color32::BLACK);
        //             info.painter.galley(rect.min, galley, egui::Color32::WHITE);
        //         }
        //     });
        // });
    }
}

impl ProfilerPanel {
    /// Point the panel at the newest frame while the header's Live toggle is on
    /// (and keep the index inside the capture after a clear).
    fn follow_displayed_frame(&mut self, session: &ProfileSession) {
        let last = session.frames.len().saturating_sub(1);
        if self.follow_latest || self.selected_frame > last {
            self.selected_frame = last;
        }
    }

    /// Unity's Profiler toolbar: a row of chrome segments closed by 1 px rules,
    /// with one full-width rule under the row.
    ///
    /// Segments, left to right: record toggle, frame transport (previous / next /
    /// newest), the `Frame: n / m` counter and Clear.
    ///
    /// The reference's other segments are not here yet because they have no
    /// behaviour to model:
    /// * before the record toggle — `Profiler Modules` (picks which module rows
    ///   the module area draws) and `Play Mode` (when the capture starts), both
    ///   text buttons with a caret;
    /// * after Clear, pushed right — `Clear on Play`, `Deep Profile`,
    ///   `Call Stacks` with a caret, then the save / help / overflow icon
    ///   cluster.
    fn main_toolbar(&mut self, ui: &mut Ui, profiler: &Profiler, session: &ProfileSession) {
        let fill = if self.recording {
            self.style.recording_bg.to_egui()
        } else {
            egui::Color32::TRANSPARENT
        };
        let hover = self.style.hover_bg.to_egui();
        let base_style = ButtonStyle::new(self.toolbar_text())
            .height(self.style.toolbar_height)
            .padding(egui::vec2(self.style.toolbar_item_padding, 0.0));
        // Only the record toggle paints a state fill; every other segment is
        // transparent until hovered.
        let record_style = base_style.clone().fills(fill, hover, hover);
        let button_style = base_style.fills(egui::Color32::TRANSPARENT, hover, self.style.click_bg.to_egui());
        let rule = self.style.separator.to_egui();
        // Every icon toggle is the same square; the rules match the row it forms.
        let row_height = self
            .style
            .toolbar_height
            .max(self.style.toolbar_icon_size + 2.0 * button_style.padding.y);
        let last = session.frames.len().saturating_sub(1);
        let has_frames = !session.frames.is_empty();

        ui.scope(|ui| {
            // Unity's toolbar segments sit edge to edge: the rules are the
            // borders, so no item spacing is added.
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            ui.horizontal(|ui| {
                let icon = if self.recording {
                    &self.style.icons.record_on
                } else {
                    &self.style.icons.record_off
                };
                let record = ui.add(
                    Button::icon_only(icon, self.style.toolbar_icon_size, &record_style)
                        .height(row_height)
                        .tooltip("Record profiling information"),
                );
                if record.clicked() {
                    self.recording = !self.recording;
                    profiler.set_recording(self.recording);
                }
                self.toolbar_rule(ui, rule, row_height);

                let previous = ui.add(
                    Button::icon_only(
                        &self.style.icons.prev_frame,
                        self.style.toolbar_icon_size,
                        &button_style,
                    )
                    .height(row_height)
                    .enabled(has_frames && self.selected_frame > 0)
                    .tooltip("Previous frame"),
                );
                if previous.clicked() {
                    self.selected_frame = self.selected_frame.saturating_sub(1);
                    self.follow_latest = false;
                }
                self.toolbar_rule(ui, rule, row_height);

                let next = ui.add(
                    Button::icon_only(
                        &self.style.icons.next_frame,
                        self.style.toolbar_icon_size,
                        &button_style,
                    )
                    .height(row_height)
                    .enabled(has_frames && self.selected_frame < last)
                    .tooltip("Next frame"),
                );
                if next.clicked() {
                    self.selected_frame = (self.selected_frame + 1).min(last);
                    self.follow_latest = self.selected_frame >= last;
                }
                self.toolbar_rule(ui, rule, row_height);

                let newest = ui.add(
                    Button::icon_only(
                        &self.style.icons.current_frame,
                        self.style.toolbar_icon_size,
                        &button_style,
                    )
                    .height(row_height)
                    .enabled(has_frames && !self.follow_latest)
                    .tooltip("Newest frame"),
                );
                if newest.clicked() {
                    self.selected_frame = last;
                    self.follow_latest = true;
                }
                self.toolbar_rule(ui, rule, row_height);

                let counter = if has_frames {
                    format!(
                        "Frame: {} / {}",
                        self.selected_frame + 1,
                        session.frames.len()
                    )
                } else {
                    "Frame: 0 / 0".to_owned()
                };
                self.toolbar_label(ui, &counter, row_height);
                self.toolbar_rule(ui, rule, row_height);

                let clear = ui.add(
                    Button::new("Clear", &button_style)
                        .height(row_height)
                        .enabled(has_frames),
                );
                if clear.clicked() {
                    profiler.clear();
                    self.reset_view();
                }
                self.toolbar_rule(ui, rule, row_height);

                // TODO(profiler): Unity's right-hand group — `Clear on Play`,
                // `Deep Profile`, `Call Stacks` and the save/help/overflow icon
                // cluster. It needs recording options and capture commands that
                // the host does not expose yet.
            });
            ui.add(
                Divider::horizontal()
                    .thickness(self.style.separator_width)
                    .color(rule),
            );
        });
    }

    /// Close one toolbar segment with Unity's 1 px vertical rule.
    fn toolbar_rule(&self, ui: &mut Ui, color: egui::Color32, height: f32) {
        ui.add(
            Divider::vertical()
                .length(height)
                .thickness(self.style.separator_width)
                .color(color),
        );
    }

    /// A non-interactive toolbar segment, e.g. Unity's `Frame: 29 / 53`.
    fn toolbar_label(&self, ui: &mut Ui, text: &str, height: f32) {
        let style = self.toolbar_text();
        let galley = style.layout(ui, text);
        let padding = self.style.toolbar_item_padding;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(galley.size().x + 2.0 * padding, height),
            Sense::hover(),
        );
        ui.painter().galley(
            egui::pos2(
                rect.left() + padding,
                rect.center().y - galley.size().y * 0.5,
            ),
            galley,
            style.text_color(),
        );
    }

    /// Toolbar typography, shared by the toolbar buttons, labels and chips.
    fn toolbar_text(&self) -> TextStyle {
        TextStyle::sized(
            self.style.toolbar_text.to_egui(),
            self.style.toolbar_text_size,
        )
    }

    /// Small caption font (chips, ruler labels, sample names).
    fn small_font(&self) -> FontId {
        FontId::proportional(self.style.small_text_size)
    }

    // -----------------------------------------------------------------------
    // Module area — Unity's "Profiler Modules": CPU Usage and GPU Usage rows,
    // each with a legend column and a frame-history chart.
    // -----------------------------------------------------------------------

    /// The module area above the timeline.
    ///
    /// The CPU row shows the selected frame's most expensive markers (our
    /// stand-in for Unity's category legend) and a right-aligned history chart
    /// of frame durations. The GPU row is header-only until the capture records
    /// GPU timings.
    fn module_area(&mut self, ui: &mut Ui, session: &ProfileSession) {
        egui::ScrollArea::vertical()
            .vscroll(true)
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .id_salt(ui.auto_id_with("module_area"))
            .show(ui, |ui|
        {
            let width = ui.available_width();
            let (area, _) = ui.allocate_exact_size(
                egui::vec2(width, self.style.module_area_height),
                Sense::hover(),
            );

            let rule = self.style.separator.to_egui();
            let rule_width = self.style.separator_width;
            let gpu_height = self.style.module_header_height;
            let cpu_height = (area.height() - gpu_height - rule_width).max(0.0);
            let cpu = Rect::from_min_size(area.min, egui::vec2(width, cpu_height));

            self.paint_cpu_module(ui, cpu, session);
            Divider::horizontal()
                .thickness(rule_width)
                .color(rule)
                .paint_in(
                    ui,
                    Rect::from_min_size(
                        egui::pos2(area.left(), cpu.bottom()),
                        egui::vec2(width, rule_width),
                    ),
                );

            let gpu = Rect::from_min_size(
                egui::pos2(area.left(), cpu.bottom() + rule_width),
                egui::vec2(width, gpu_height),
            );
            self.paint_gpu_module(ui, gpu);

            // Close the module area; the timeline header follows on the next row.
            ui.add(
                Divider::horizontal()
                    .thickness(rule_width)
                    .color(self.style.separator.to_egui()),
            );
        });
    }

    fn paint_cpu_module(&self, ui: &Ui, row: Rect, session: &ProfileSession) {
        let name_width = self.style.module_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        // The chart starts below the module title row, like Unity's graph.
        let chart = Rect::from_min_max(
            egui::pos2(
                name_col.right(),
                row.top() + self.style.module_header_height,
            ),
            row.max,
        );
        let header = Rect::from_min_size(
            row.min,
            egui::vec2(row.width(), self.style.module_header_height),
        );
        let rule = self.style.separator.to_egui();

        // Column rule between the legend and the chart, inside the name column.
        self.paint_vertical_rule(
            ui,
            egui::pos2(name_col.right() - self.style.separator_width, row.top()),
            row.height(),
            rule,
        );

        let value = session
            .frames
            .get(self.selected_frame)
            .map_or_else(|| "no frames".to_owned(), |frame| frame_cost_label(frame));
        self.paint_module_header(ui, header, name_col, "CPU", "CPU Usage", &value);

        // Legend: Unity's category rows with the selected frame's cost, then the
        // frame's Scripts systems by cost. Zero-cost categories keep their row,
        // like Unity: the legend is a fixed vocabulary, not a ranking.
        let frame = session.frames.get(self.selected_frame);
        let visible = ((row.height() - self.style.module_header_height)
            / self.style.legend_row_height.max(1.0))
            .floor()
            .max(0.0) as usize;
        let labels = ui.painter().with_clip_rect(name_col);
        let row_center = |index: usize| {
            egui::pos2(
                name_col.left() + 12.0,
                row.top()
                    + self.style.module_header_height
                    + (index as f32 + 0.5) * self.style.legend_row_height,
            )
        };
        let mut used = 0;
        for category in Category::ALL.iter().take(visible) {
            let center = row_center(used);
            labels.rect_filled(
                Rect::from_center_size(center, egui::vec2(8.0, 8.0)),
                CornerRadius::same(1),
                self.category_color(*category),
            );
            labels.text(
                egui::pos2(name_col.left() + 24.0, center.y),
                Align2::LEFT_CENTER,
                category.label(),
                self.toolbar_font(),
                self.style.timeline_text.to_egui(),
            );
            if let Some(frame) = frame {
                labels.text(
                    egui::pos2(name_col.right() - 8.0, center.y),
                    Align2::RIGHT_CENTER,
                    format!("{:.2}ms", self.category_ms(frame, *category)),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
            }
            used += 1;
        }

        // Systems get whatever rows are left. The full ranking belongs in a
        // details pane (Unity's Hierarchy view); this is the top of it, which is
        // the refinement of Unity's `Scripts` category.
        let systems = self.system_entries();
        let rows_left = visible.saturating_sub(used);
        if rows_left > 0 && !systems.is_empty() {
            let shown = if systems.len() > rows_left {
                rows_left.saturating_sub(1)
            } else {
                systems.len()
            };
            for (name, ns) in systems.iter().take(shown) {
                let center = row_center(used);
                labels.rect_filled(
                    Rect::from_center_size(center, egui::vec2(8.0, 8.0)),
                    CornerRadius::same(1),
                    self.category_color(Category::Scripts),
                );
                labels.text(
                    egui::pos2(name_col.left() + 24.0, center.y),
                    Align2::LEFT_CENTER,
                    name,
                    self.toolbar_font(),
                    self.style.timeline_text.to_egui(),
                );
                labels.text(
                    egui::pos2(name_col.right() - 8.0, center.y),
                    Align2::RIGHT_CENTER,
                    format!("{:.2}ms", *ns as f32 / 1_000_000.0),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
                used += 1;
            }
            if systems.len() > shown {
                let center = row_center(used);
                labels.text(
                    egui::pos2(name_col.left() + 24.0, center.y),
                    Align2::LEFT_CENTER,
                    format!("+{} more", systems.len() - shown),
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
            }
        }

        self.paint_chart(ui, chart, session);
    }

    fn paint_gpu_module(&self, ui: &Ui, row: Rect) {
        let name_width = self.style.module_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        self.paint_module_header(
            ui,
            row,
            name_col,
            "GPU",
            "GPU Usage",
            "timings not captured",
        );

        // Unity's warning triangle: this build cannot report GPU timings.
        let center = egui::pos2(name_col.right() - 14.0, row.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(center.x, center.y - 5.0),
                egui::pos2(center.x + 5.0, center.y + 4.0),
                egui::pos2(center.x - 5.0, center.y + 4.0),
            ],
            self.style.warning.to_egui(),
            Stroke::NONE,
        ));

        // TODO(profiler): give GPU Usage a real chart once the renderer reports
        // GPU frame timings into the capture.
    }

    /// One module title row: badge, name, and the current-value chip.
    fn paint_module_header(
        &self,
        ui: &Ui,
        header: Rect,
        name_col: Rect,
        badge: &str,
        title: &str,
        value: &str,
    ) {
        let painter = ui.painter();
        let badge_rect = Rect::from_center_size(
            egui::pos2(header.left() + 14.0, header.center().y),
            egui::vec2(18.0, 12.0),
        );
        // TODO(profiler): Unity ships a CPU/GPU badge texture per module; paint
        // the letters until that artwork lands.
        painter.rect_filled(
            badge_rect,
            CornerRadius::same(2),
            self.style.hover_bg.to_egui(),
        );
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            badge,
            FontId::proportional(7.0),
            self.style.toolbar_text.to_egui(),
        );
        painter.text(
            egui::pos2(badge_rect.right() + 6.0, header.center().y),
            Align2::LEFT_CENTER,
            title,
            self.toolbar_font(),
            self.style.timeline_text.to_egui(),
        );

        // Value chip at the start of the chart area, like the reference.
        self.paint_chip(
            ui,
            egui::pos2(name_col.right() + 8.0, header.center().y),
            value,
        );
    }

    /// A Unity chip: dark rounded box around small text, left-centre anchored.
    fn paint_chip(&self, ui: &Ui, left_center: egui::Pos2, text: &str) -> Rect {
        let galley = ui.painter().layout_no_wrap(
            text.to_owned(),
            self.small_font(),
            self.style.chip_text.to_egui(),
        );
        let rect = Rect::from_min_size(
            egui::pos2(
                left_center.x,
                left_center.y - self.style.legend_row_height * 0.5,
            ),
            egui::vec2(galley.size().x + 8.0, self.style.legend_row_height),
        );
        ui.painter()
            .rect_filled(rect, CornerRadius::same(2), self.style.chip_bg.to_egui());
        ui.painter().galley(
            egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y * 0.5),
            galley,
            self.style.chip_text.to_egui(),
        );
        rect
    }

    /// The frame-history chart: durations of the last `chart_capacity` frames,
    /// right-aligned (newest at the right edge), with Unity's 30/60 FPS
    /// reference lines and a cursor at the selected frame.
    fn paint_chart(&self, ui: &Ui, plot: Rect, session: &ProfileSession) {
        let painter = ui.painter().with_clip_rect(plot);
        let shown = session.frames.len();

        if shown == 0 {
            painter.text(
                plot.center(),
                Align2::CENTER_CENTER,
                "Waiting for the first captured frame…",
                self.small_font(),
                self.style.ruler_text.to_egui(),
            );
            return;
        }

        let max_ms = session
            .frames
            .iter()
            .map(|frame| self.bar_ms(frame))
            .fold(TARGET_30_FPS_MS, f32::max)
            * 1.05;
        let bar_width = (plot.width() / self.style.window_fram_count.max(1) as f32).max(1.0);
        let bar_left = |from_end: usize| plot.right() - (from_end as f32 + 1.0) * bar_width;

        // One stacked bar per frame. Unity anchors `Others` at the bottom and
        // stacks up the legend, so the first row is the topmost band.
        for (from_end, frame) in session.frames.iter().rev().take(shown).enumerate() {
            let x = bar_left(from_end);
            let mut bottom = plot.bottom();
            for category in Category::ALL.iter().rev().copied() {
                let ms = self.category_ms(frame, category);
                if ms <= 0.0 {
                    continue;
                }
                let height = (ms / max_ms * plot.height()).max(0.5);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(x, bottom - height),
                        egui::pos2(x + bar_width, bottom),
                    ),
                    CornerRadius::ZERO,
                    self.category_color(category),
                );
                bottom -= height;
            }
        }

        // 30/60 FPS budgets, labelled the way Unity labels them. Drawn over the
        // bars so the chips stay readable.
        for (fps, ms) in [(30.0_f32, TARGET_30_FPS_MS), (60.0, TARGET_60_FPS_MS)] {
            if ms > max_ms {
                continue;
            }
            let half_chip = self.style.legend_row_height * 0.5;
            let y = (plot.bottom() - ms / max_ms * plot.height())
                .clamp(plot.top() + half_chip, plot.bottom() - half_chip);
            Divider::horizontal()
                .thickness(1.0)
                .color(self.style.chart_grid.to_egui())
                .paint_in(
                    ui,
                    Rect::from_min_size(egui::pos2(plot.left(), y), egui::vec2(plot.width(), 1.0)),
                );
            self.paint_chip(
                ui,
                egui::pos2(plot.left() + 6.0, y),
                &format!("{ms:.0}ms ({fps:.0}FPS)"),
            );
        }

        // Selected-frame cursor.
        let selected = self.selected_frame.min(session.frames.len() - 1);
        let from_end = session.frames.len() - 1 - selected;
        let center = bar_left(from_end) + bar_width * 0.5;
        Divider::vertical()
            .thickness(2.0)
            .color(self.style.chart_cursor.to_egui())
            .paint_in(
                ui,
                Rect::from_min_size(
                    egui::pos2(center - 1.0, plot.top()),
                    egui::vec2(2.0, plot.height()),
                ),
            );
    }

    /// The selected frame's Scripts systems, highest first.
    fn system_entries(&self) -> &[(String, u64)] {
        self.cached
            .as_ref()
            .map_or(&[], |cached| cached.systems.as_slice())
    }

    /// Category colour from the legend palette (index = enum order).
    fn category_color(&self, category: Category) -> egui::Color32 {
        self.style
            .category_palette
            .get(category as usize)
            .map_or_else(|| self.style.chart_bar.to_egui(), |color| color.to_egui())
    }

    /// Exclusive time in one category for one frame, in milliseconds.
    ///
    /// `Others` also absorbs the part of the frame no scope covered (scheduler
    /// overhead, uninstrumented gaps), so the stacked bar reaches the frame
    /// time the way Unity's does.
    fn category_ms(&self, frame: &FrameSnapshot, category: Category) -> f32 {
        let mut ns = frame.category_ns(category);
        if category == Category::Others {
            let attributed: u64 = Category::ALL
                .iter()
                .filter(|other| **other != Category::Others)
                .map(|other| frame.category_ns(*other))
                .sum();
            ns = ns.saturating_add(frame.duration_ns().saturating_sub(attributed + ns));
        }
        ns as f32 / 1_000_000.0
    }

    /// The drawn height of a frame's bar: its categories stacked. Equal to the
    /// frame time unless parallel work pushes the categories past it.
    fn bar_ms(&self, frame: &FrameSnapshot) -> f32 {
        Category::ALL
            .iter()
            .map(|category| self.category_ms(frame, *category))
            .sum()
    }

    /// Stable marker colour: the palette entry picked by the scope id. The
    /// capture does not record per-marker colours yet (Unity lets the user set
    /// them), so the id is the only stable key.
    fn sample_color(&self, scope: u32) -> egui::Color32 {
        let palette = &self.style.palette;
        let index = scope as usize % palette.len().max(1);
        palette
            .get(index)
            .map_or(egui::Color32::LIGHT_GRAY, |color| color.to_egui())
    }

    fn scope_name(&self, scope: u32) -> Option<&str> {
        let cached = self.cached.as_ref()?;
        scope_info(&cached.scopes, scope).map(|info| info.name.as_str())
    }

    // -----------------------------------------------------------------------
    // Timeline — header, ruler, thread tracks.
    // -----------------------------------------------------------------------

    /// Unity's timeline: a header (view selector, Live, CPU/GPU summary), a
    /// ruler, then one track per thread.
    fn timeline(&mut self, ui: &mut Ui, session: &ProfileSession) {
        egui::ScrollArea::vertical()
            .scroll([true, true])
            .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
            .id_salt(ui.auto_id_with("timeline"))
            .show(ui, |ui|
        {
            let width = ui.available_width();
            let row_height = self.style.toolbar_height;
            let rule = self.style.separator.to_egui();

            let (header, _) = ui.allocate_exact_size(egui::vec2(width, row_height), Sense::hover());
            self.paint_timeline_header(ui, header, session);
            ui.add(
                Divider::horizontal()
                    .thickness(self.style.separator_width)
                    .color(rule),
            );

            let duration_ns = session
                .frames
                .get(self.selected_frame)
                .map_or(0, |frame| frame.duration_ns());
            let track_width = (width - self.style.timeline_name_width).max(0.0);
            let ticks = ruler_ticks(duration_ns, track_width);

            let (ruler, _) =
                ui.allocate_exact_size(egui::vec2(width, self.style.ruler_height), Sense::hover());
            self.paint_ruler(ui, ruler, &ticks);

            let Some(frame) = session.frames.get(self.selected_frame) else {
                self.paint_empty_timeline(ui);
                return;
            };

            // TODO(profiler): Unity groups threads (JOB / LOADING / SCRIPTING …)
            // into collapsible rows with a thread count; the capture reports flat
            // threads, so each one gets its own row here.
            // TODO(profiler): horizontal pan/zoom and the bottom scrollbar: the
            // track currently maps the whole frame onto the available width.
            for thread in &frame.threads {
                let levels = thread.depth().max(1) as f32;
                let height = levels * self.style.level_height + 2.0 * self.style.thread_row_padding;
                let (row, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
                self.paint_thread_row(ui, row, thread, &session.scopes, &ticks, duration_ns);
            }

            // Fill the rest of the panel so the tracks read as one surface.
            let rest = ui.available_rect_before_wrap();
            if rest.height() > 0.0 {
                ui.painter()
                    .rect_filled(rest, CornerRadius::ZERO, self.style.track_bg.to_egui());
            }
        });
    }

    fn paint_timeline_header(&mut self, ui: &mut Ui, row: Rect, session: &ProfileSession) {
        let rule = self.style.separator.to_egui();
        let style = ButtonStyle::new(self.toolbar_text())
            .fills(
                egui::Color32::TRANSPARENT,
                self.style.hover_bg.to_egui(),
                self.style.selected_bg.to_egui(),
            )
            .height(row.height())
            .padding(egui::vec2(self.style.toolbar_item_padding, 0.0));

        // View selector. Unity's menu holds Timeline / Hierarchy / Call Stacks;
        // only the timeline is rebuilt so far.
        let view = Rect::from_min_size(
            row.min,
            egui::vec2(self.style.timeline_view_width, row.height()),
        );
        ui.put(
            view,
            Button::new("Timeline", &style)
                .width(view.width())
                .height(view.height())
                .caret(),
        );
        // TODO(profiler): open the view menu here and draw the Hierarchy / Raw /
        // Call Stacks views through the same selector.
        self.paint_vertical_rule(ui, egui::pos2(view.right(), row.top()), row.height(), rule);

        let live = Rect::from_min_size(
            egui::pos2(view.right() + self.style.separator_width, row.top()),
            egui::vec2(self.style.timeline_live_width, row.height()),
        );
        let response = ui.put(
            live,
            Button::new("Live", &style)
                .width(live.width())
                .height(live.height())
                .align(ButtonAlign::Center)
                .selected(self.follow_latest),
        );
        if response.clicked() {
            self.follow_latest = !self.follow_latest;
            if self.follow_latest {
                self.selected_frame = session.frames.len().saturating_sub(1);
            }
        }

        // CPU/GPU summary, centred over the track column like the reference.
        let summary = match session.frames.get(self.selected_frame) {
            Some(frame) => format!("CPU: {:.2}ms    GPU: --", frame_ms(frame)),
            None => "CPU: --    GPU: --".to_owned(),
        };
        let track_left = row.left() + self.style.timeline_name_width;
        ui.painter().text(
            egui::pos2((track_left + row.right()) * 0.5, row.center().y),
            Align2::CENTER_CENTER,
            summary,
            self.small_font(),
            self.style.timeline_text.to_egui(),
        );
    }

    fn paint_ruler(&self, ui: &Ui, row: Rect, ticks: &[RulerTick]) {
        let painter = ui.painter();
        let track_left = row.left() + self.style.timeline_name_width;
        painter.rect_filled(row, CornerRadius::ZERO, self.style.ruler_bg.to_egui());
        let labels = painter.with_clip_rect(row);

        for tick in ticks {
            let x = track_left + tick.x;
            if x > row.right() {
                continue;
            }
            let (height, color) = match &tick.label {
                Some(_) => (
                    self.style.ruler_height * 0.4,
                    self.style.ruler_text.to_egui(),
                ),
                None => (
                    self.style.ruler_height * 0.25,
                    self.style.ruler_text.to_egui().gamma_multiply(0.7),
                ),
            };
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x, row.bottom() - height),
                    egui::vec2(1.0, height),
                ),
                CornerRadius::ZERO,
                color,
            );
            if let Some(label) = &tick.label {
                labels.text(
                    egui::pos2(x + 3.0, row.top() + 3.0),
                    Align2::LEFT_TOP,
                    label,
                    self.small_font(),
                    self.style.ruler_text.to_egui(),
                );
            }
        }

        Divider::horizontal()
            .thickness(self.style.separator_width)
            .color(self.style.separator.to_egui())
            .paint_in(
                ui,
                Rect::from_min_size(
                    egui::pos2(row.left(), row.bottom() - self.style.separator_width),
                    egui::vec2(row.width(), self.style.separator_width),
                ),
            );
    }

    fn paint_thread_row(
        &self,
        ui: &Ui,
        row: Rect,
        thread: &ThreadSnapshot,
        scopes: &[ScopeInfo],
        ticks: &[RulerTick],
        duration_ns: u64,
    ) {
        let name_width = self.style.timeline_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        let track = Rect::from_min_max(egui::pos2(name_col.right(), row.top()), row.max);
        let painter = ui.painter();

        painter.rect_filled(
            name_col,
            CornerRadius::ZERO,
            self.style.timeline_name_bg.to_egui(),
        );
        painter.rect_filled(track, CornerRadius::ZERO, self.style.track_bg.to_egui());
        // TODO(profiler): elide long thread names with "…" instead of clipping.
        painter.with_clip_rect(name_col).text(
            egui::pos2(
                name_col.left() + 10.0,
                name_col.top() + self.style.thread_row_padding,
            ),
            Align2::LEFT_TOP,
            thread_label(thread),
            self.toolbar_font(),
            self.style.timeline_text.to_egui(),
        );

        // Grid lines aligned with the labelled ruler ticks.
        for tick in ticks.iter().filter(|tick| tick.label.is_some()) {
            let x = track.left() + tick.x;
            if x >= track.right() - 0.5 {
                continue;
            }
            self.paint_vertical_rule(
                ui,
                egui::pos2(x, track.top()),
                track.height(),
                self.style.grid.to_egui(),
            );
        }

        // Column rule between the names and the tracks, inside the name column.
        self.paint_vertical_rule(
            ui,
            egui::pos2(name_col.right() - self.style.separator_width, row.top()),
            row.height(),
            self.style.separator.to_egui(),
        );

        if duration_ns > 0 {
            let mut labels = MAX_SAMPLE_LABELS;
            for sample in &thread.samples {
                let start = sample.start_ns.min(duration_ns) as f64 / duration_ns as f64;
                let end = sample
                    .start_ns
                    .saturating_add(sample.duration_ns)
                    .min(duration_ns) as f64
                    / duration_ns as f64;
                let x0 = track.left() + start as f32 * track.width();
                let x1 = track.left() + end as f32 * track.width();
                let top = track.top()
                    + self.style.thread_row_padding
                    + sample.depth as f32 * self.style.level_height;
                let height = (self.style.level_height - 1.0).max(1.0);
                let bar = Rect::from_min_max(
                    egui::pos2(x0, top),
                    egui::pos2(x1.max(x0 + 1.0).min(track.right()), top + height),
                );
                if bar.width() <= 0.0 {
                    continue;
                }

                let color = self.sample_color(sample.scope);
                painter.rect_filled(bar, CornerRadius::same(1), color);
                painter.rect_stroke(
                    bar,
                    CornerRadius::same(1),
                    Stroke::new(1.0, color.gamma_multiply(0.55)),
                    StrokeKind::Inside,
                );

                // Labels are expensive; only paint them where they fit, and cap
                // them per row so a dense frame stays interactive.
                if labels > 0 && bar.width() >= 34.0 {
                    if let Some(scope) = scope_info(scopes, sample.scope) {
                        labels -= 1;
                        let clip = bar.shrink2(egui::vec2(2.0, 0.0));
                        painter.with_clip_rect(clip).text(
                            egui::pos2(clip.left() + 1.0, bar.center().y),
                            Align2::LEFT_CENTER,
                            &scope.name,
                            self.small_font(),
                            self.style.sample_text.to_egui(),
                        );
                    }
                }
            }
        }

        // Rule under the row.
        Divider::horizontal()
            .thickness(self.style.separator_width)
            .color(self.style.row_rule.to_egui())
            .paint_in(
                ui,
                Rect::from_min_size(
                    egui::pos2(row.left(), row.bottom() - self.style.separator_width),
                    egui::vec2(row.width(), self.style.separator_width),
                ),
            );
    }

    fn paint_empty_timeline(&self, ui: &Ui) {
        let rest = ui.available_rect_before_wrap();
        if rest.height() <= 0.0 {
            return;
        }
        ui.painter()
            .rect_filled(rest, CornerRadius::ZERO, self.style.track_bg.to_egui());
        ui.painter().text(
            rest.center(),
            Align2::CENTER_CENTER,
            "No frames captured — press Record to profile the editor.",
            self.toolbar_font(),
            self.style.ruler_text.to_egui(),
        );
    }

    /// One 1 px vertical rule at `at` (its left edge) with the given height.
    fn paint_vertical_rule(&self, ui: &Ui, at: egui::Pos2, height: f32, color: egui::Color32) {
        Divider::vertical()
            .thickness(self.style.separator_width)
            .color(color)
            .paint_in(
                ui,
                Rect::from_min_size(at, egui::vec2(self.style.separator_width, height)),
            );
    }

    /// The 12 px label font used by names, titles and chips.
    fn toolbar_font(&self) -> FontId {
        FontId::proportional(self.style.toolbar_text_size)
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
        let systems = system_entries(&frame, &session.scopes);
        self.cached = Some(CachedFrame {
            frame,
            scopes: session.scopes.clone(),
            rows,
            systems,
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
                                ui.colored_label(self.style.recording_bg.to_egui(), warning);
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

/// 30 FPS / 60 FPS frame budgets, drawn as chart reference lines and used as
/// the chart's minimum scale so a fast capture still shows them.
const TARGET_30_FPS_MS: f32 = 1000.0 / 30.0;
const TARGET_60_FPS_MS: f32 = 1000.0 / 60.0;

/// Sample labels are the expensive part of a track; cap them per thread row.
const MAX_SAMPLE_LABELS: usize = 64;

/// Tick steps, in milliseconds, that read well at any capture length.
const RULER_STEPS_MS: [f64; 18] = [
    0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
    1000.0, 2000.0,
];

/// Minimum distance between two labelled ruler ticks, in points.
const MIN_TICK_SPACING: f32 = 80.0;

/// One tick on the timeline ruler.
#[derive(Clone, Debug, PartialEq)]
struct RulerTick {
    /// Position from the track column's left edge, in points.
    x: f32,
    /// Label at a major tick; `None` for the four minor ticks between majors.
    label: Option<String>,
}

/// Ruler ticks for a frame lasting `duration_ns` drawn across `width` points.
///
/// The step is the smallest "nice" value that keeps labels at least
/// [`MIN_TICK_SPACING`] apart, so a 2 ms frame and a 2 s frame both read. Each
/// major tick is followed by four unlabelled minors.
fn ruler_ticks(duration_ns: u64, width: f32) -> Vec<RulerTick> {
    let duration_ms = duration_ns as f64 / 1_000_000.0;
    if duration_ms <= 0.0 || width <= 0.0 {
        return Vec::new();
    }

    let step = RULER_STEPS_MS
        .iter()
        .copied()
        .find(|step| step / duration_ms * f64::from(width) >= f64::from(MIN_TICK_SPACING))
        .unwrap_or(RULER_STEPS_MS[RULER_STEPS_MS.len() - 1]);

    let mut ticks = Vec::new();
    let mut major = 0.0;
    while major <= duration_ms + 1e-9 {
        ticks.push(RulerTick {
            x: (major / duration_ms) as f32 * width,
            label: Some(format!("{major:.3}ms")),
        });
        for minor in 1..5 {
            let at = major + step * f64::from(minor) / 5.0;
            if at < duration_ms {
                ticks.push(RulerTick {
                    x: (at / duration_ms) as f32 * width,
                    label: None,
                });
            }
        }
        major += step;
    }
    ticks
}

/// Wall time of one frame, in milliseconds.
fn frame_ms(frame: &FrameSnapshot) -> f32 {
    frame.duration_ns() as f32 / 1_000_000.0
}

/// Unity's module chip: `6.2ms (161FPS)`. Sub-0.1 ms frames are reported as
/// `<0.1ms`: the FPS equivalent would be noise.
fn frame_cost_label(frame: &FrameSnapshot) -> String {
    let ms = frame_ms(frame);
    if ms < 0.1 {
        "<0.1ms".to_owned()
    } else {
        format!("{ms:.1}ms ({:.0}FPS)", 1000.0 / ms)
    }
}

/// The selected frame's Scripts systems, by inclusive time (highest first).
///
/// Core-module systems (physics, audio, rendering, editor) are folded into their
/// category and never appear here: this list is exactly the refinement of
/// Unity's `Scripts` category into one row per system. Inclusive time matches
/// what a system costs from its caller's point of view, and is what Unity's
/// Hierarchy view reports.
fn system_entries(frame: &FrameSnapshot, scopes: &[ScopeInfo]) -> Vec<(String, u64)> {
    let mut totals: BTreeMap<&str, u64> = BTreeMap::new();
    for thread in &frame.threads {
        for sample in &thread.samples {
            let Some(info) = scope_info(scopes, sample.scope) else {
                continue;
            };
            if info.name != SYSTEM_MARKER {
                continue;
            }
            let Some(system) = sample.data.as_deref() else {
                continue;
            };
            if Category::of_system(system) != Category::Scripts {
                continue;
            }
            *totals.entry(system).or_default() = totals
                .get(system)
                .copied()
                .unwrap_or_default()
                .saturating_add(sample.duration_ns);
        }
    }
    let mut rows: Vec<(String, u64)> = totals
        .into_iter()
        .map(|(system, ns)| (short_system_name(system), ns))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

/// `kairos_game::spin_system` → `spin_system`.
fn short_system_name(system: &str) -> String {
    system.rsplit("::").next().unwrap_or(system).to_owned()
}

#[cfg(test)]
mod probe {
    use super::*;
    use crate::frame::HostView;
    use egui::pos2;
    use kairos_editor_core::EditorState;

    struct NoHost;
    impl HostView for NoHost {}

    struct ProfilerHost(Profiler);
    impl HostView for ProfilerHost {
        fn profiler(&self) -> Option<&Profiler> {
            Some(&self.0)
        }
    }

    /// Every filled rect the pass painted, as `(rect, fill)`.
    fn painted(output: &egui::FullOutput) -> Vec<(Rect, egui::Color32)> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) => Some((shape.rect, shape.fill)),
                _ => None,
            })
            .collect()
    }

    /// The profiler sink is process-global, so the tests that create one must
    /// not run concurrently with each other.
    static PROFILER_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_profiler_tests() -> std::sync::MutexGuard<'static, ()> {
        PROFILER_TESTS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Capture one committed frame with two nested scopes on this thread.
    fn captured_profiler() -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!("profiler.test.frame");
            {
                kairos_profile::scope!("profiler.test.work");
            }
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        assert!(
            !profiler.session().is_empty(),
            "the fixture captured one committed frame"
        );
        profiler
    }

    /// One headless pass of the whole panel.
    fn draw_panel(panel: &mut ProfilerPanel, host: &ProfilerHost) -> egui::FullOutput {
        let ctx = Context::default();
        let state = EditorState::new();
        let mut out = UiOut::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            egui::vec2(900.0, 640.0),
        ));
        ctx.run_ui(input, |ui| {
            panel.ui(ui, &Frame::with_host(&state, host), &mut out);
        })
    }

    /// Text painted in one pass, in paint order.
    fn painted_text(output: &egui::FullOutput) -> Vec<String> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// The Unity toolbar contract: a square record toggle, one vertical rule
    /// closing that segment, and one full-width rule under the row.
    #[test]
    fn toolbar_draws_a_square_record_toggle_flanked_by_chrome_rules() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        profiler.set_recording(true);

        let ctx = Context::default();
        let state = EditorState::new();
        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let mut out = UiOut::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            egui::vec2(400.0, 300.0),
        ));
        let output = ctx.run_ui(input, |ui| {
            panel.ui(ui, &Frame::with_host(&state, &host), &mut out);
        });

        let painted = painted(&output);
        let record = painted
            .iter()
            .find(|(_, fill)| *fill == panel.style.recording_bg.to_egui())
            .expect("the recording toggle paints its recording fill")
            .0;
        assert_eq!(
            record.width(),
            record.height(),
            "the record toggle must be square"
        );
        assert_eq!(record.height(), panel.style.toolbar_height);

        let separator = panel.style.separator.to_egui();
        let chrome: Vec<Rect> = painted
            .iter()
            .filter(|(_, fill)| *fill == separator)
            .map(|(rect, _)| *rect)
            .collect();
        let vertical = chrome
            .iter()
            .find(|rect| rect.width() <= 1.0 && rect.height() > 1.0)
            .expect("a vertical rule closes the record segment");
        assert_eq!(vertical.left(), record.right());
        assert_eq!(vertical.height(), record.height());

        let horizontal = chrome
            .iter()
            .find(|rect| rect.height() <= 1.0 && rect.width() > 1.0)
            .expect("a full-width rule sits under the toolbar row");
        assert_eq!(horizontal.top(), record.bottom());
        assert!(
            horizontal.width() > record.width(),
            "the row-closing rule spans the panel, not just the button"
        );
    }

    /// Unity's `Scripts` category, refined: the legend lists the fixed category
    /// vocabulary, game systems get a row each, and a core-module system is
    /// folded into its category instead of getting one.
    #[test]
    fn the_legend_lists_categories_and_refines_scripts_into_systems() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!("Kairos.Stage", "Update");
            {
                kairos_profile::scope!("Kairos.System", "kairos_physics::physics_step_system");
                // Give the sample a measurable duration: puffin reads the clock
                // in nanoseconds, and back-to-back scopes can land on the same
                // tick, which would leave the Physics band at zero height.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            {
                kairos_profile::scope!("Kairos.System", "kairos_game::spin_system");
            }
        }
        profiler.end_frame(true);
        profiler.begin_frame();

        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let text = painted_text(&output);

        for label in [
            "Rendering",
            "Scripts",
            "Physics",
            "Audio",
            "Editor",
            "VSync",
            "Others",
        ] {
            assert!(
                text.iter().any(|line| line == label),
                "legend row {label} missing: {text:?}"
            );
        }
        assert!(
            text.iter().any(|line| line == "spin_system"),
            "a game system is the Scripts refinement, by short name: {text:?}"
        );
        assert!(
            !text.iter().any(|line| line == "physics_step_system"),
            "a core-module system folds into its category: {text:?}"
        );

        // The chart stacks the categories: the Physics colour is painted inside
        // the plot (right of the name column), not only as an 8 px legend swatch.
        let physics = panel.style.category_palette[Category::Physics as usize].to_egui();
        assert!(
            painted(&output).iter().any(
                |(rect, fill)| *fill == physics && rect.left() >= panel.style.module_name_width
            ),
            "the chart stacks a Physics segment"
        );
    }

    /// The module area: CPU/GPU rows, the legend of the selected frame, Unity's
    /// 30/60 FPS reference chips and the selected-frame cursor.
    #[test]
    fn module_area_draws_the_cpu_gpu_rows_and_history_chart() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);

        let text = painted_text(&output);
        assert!(text.iter().any(|line| line == "CPU Usage"), "{text:?}");
        assert!(text.iter().any(|line| line == "GPU Usage"), "{text:?}");
        assert!(
            text.iter().any(|line| line.contains("profiler.test.frame")),
            "the legend lists the selected frame's markers: {text:?}"
        );
        assert!(
            text.iter().any(|line| line.contains("30FPS"))
                && text.iter().any(|line| line.contains("60FPS")),
            "Unity's 30/60 FPS reference chips are drawn: {text:?}"
        );

        let cursor = panel.style.chart_cursor.to_egui();
        assert!(
            painted(&output)
                .iter()
                .any(|(rect, fill)| *fill == cursor && rect.height() > 10.0),
            "the selected frame has a cursor in the chart"
        );
    }

    /// The timeline: header segments, one track per thread, sample bars in the
    /// marker palette to the right of the name column.
    #[test]
    fn timeline_draws_thread_tracks_with_sample_bars() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);

        let text = painted_text(&output);
        for expected in ["Timeline", "Live", "Frame: 1 / 1"] {
            assert!(
                text.iter().any(|line| line == expected),
                "missing header text {expected:?}: {text:?}"
            );
        }
        assert!(
            text.iter().any(|line| line == "profiler.test.frame"),
            "the root sample is labelled: {text:?}"
        );

        let palette: Vec<egui::Color32> =
            panel.style.palette.iter().map(|color| color.to_egui()).collect();
        let track_left = panel.style.timeline_name_width;
        assert!(
            painted(&output).iter().any(|(rect, fill)| {
                rect.left() >= track_left && rect.height() >= 10.0 && palette.contains(fill)
            }),
            "sample bars use the marker palette inside the track column"
        );
    }

    /// The ruler picks a step that keeps labels apart at any capture length.
    #[test]
    fn ruler_ticks_stay_readable_at_every_frame_length() {
        let ticks = ruler_ticks(45_000_000, 900.0);
        let labels: Vec<f32> = ticks
            .iter()
            .filter(|tick| tick.label.is_some())
            .map(|tick| tick.x)
            .collect();
        assert!(labels.len() >= 2, "{labels:?}");
        assert_eq!(labels.first().copied(), Some(0.0));
        for pair in labels.windows(2) {
            assert!(
                pair[1] - pair[0] >= MIN_TICK_SPACING,
                "labels must stay apart: {labels:?}"
            );
        }
        assert!(
            ticks.iter().any(|tick| tick.label.is_none()),
            "minor ticks sit between the labelled ones"
        );

        assert!(!ruler_ticks(2_000_000, 900.0).is_empty());
        assert!(!ruler_ticks(2_000_000_000, 900.0).is_empty());
        assert!(ruler_ticks(0, 900.0).is_empty());
    }

    /// Nothing captured: the panel explains how to start instead of drawing an
    /// empty track.
    #[test]
    fn an_empty_capture_asks_the_user_to_record() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(Profiler::new());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);

        let text = painted_text(&output);
        assert!(
            text.iter().any(|line| line.contains("No frames captured")),
            "{text:?}"
        );
        assert!(text.iter().any(|line| line == "Frame: 0 / 0"), "{text:?}");
        assert!(
            text.iter()
                .any(|line| line.contains("Waiting for the first captured frame")),
            "the module chart has an empty state: {text:?}"
        );
    }


    fn draw(panel: &mut ProfilerPanel) -> (usize, usize) {
        let ctx = Context::default();
        let state = EditorState::new();
        let host = NoHost;
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(800.0, 600.0)));
        let mut out = UiOut::new();
        let output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.ui(ui, &Frame::with_host(&state, &host), &mut out);
            });
        });
        let texts = output
            .shapes
            .iter()
            .filter(|clipped| matches!(clipped.shape, egui::Shape::Text(_)))
            .count();
        (output.shapes.len(), texts)
    }

    fn latest(panel: &ProfilerPanel) -> usize {
        panel.global_frame_view.lock().latest_frames(16).count()
    }

    #[test]
    fn probe_where_frames_come_from() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        let mut panel = ProfilerPanel::default();

        profiler.begin_frame();
        {
            kairos_profile::scope!("probe.frame");
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        eprintln!(
            "PROBE[1] recording-off latest_frames={} draw={:?}",
            latest(&panel),
            draw(&mut panel)
        );

        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!("probe.frame");
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        eprintln!(
            "PROBE[2] recording-on  latest_frames={} draw={:?}",
            latest(&panel),
            draw(&mut panel)
        );

        let late = GlobalFrameView::default();
        eprintln!(
            "PROBE[3] late-view     latest_frames={}",
            late.lock().latest_frames(16).count()
        );
        drop(late);

        profiler.set_recording(false);
        eprintln!(
            "PROBE[4] stopped       latest_frames={}",
            latest(&panel)
        );
    }
}
