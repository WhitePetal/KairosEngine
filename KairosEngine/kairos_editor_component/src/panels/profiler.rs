//! Profiler panel: Unity's Profiler window over the structured capture.
//!
//! The panel owns no capture data. It reads the
//! [`kairos_profile::ProfileSession`] through [`crate::HostView::profiler`], keeps
//! only view state (recording flag, selected frame, live-follow, the selected
//! block, the module area's height, the timeline's zoom and scroll) and draws
//! Unity's bands, top to bottom:
//!
//! * **toolbar** — record toggle, frame transport, `Frame: n / m`, Clear,
//!   separated into segments by 1 px rules;
//! * **module area** — Unity's modules pane: fixed-height module cards over a
//!   vertical scroll bar. CPU Usage has the category legend and a stacked
//!   frame-history chart (Unity's 30/60 FPS reference lines and a 120 FPS
//!   baseline); GPU Usage is a fixed-height empty card until the capture
//!   records GPU timings;
//! * **splitter** — the draggable rule between the module area and the
//!   timeline, Unity's split view handle;
//! * **timeline** — a pinned header (view selector, Live toggle, CPU/GPU
//!   summary) and time ruler over one track per registered thread lane, a
//!   zoomable time axis and Unity's two scroll bars.
//!
//! Layout, colours and measured geometry are [`ProfilerStyle`] tokens loaded
//! from `Preferences/Styles/Profiler.ron`.
//!
//! # Categories
//!
//! The chart stacks the engine modules ([`Category`]): Rendering, Scripts,
//! Physics, Audio, Editor, VSync and Others. A marker declares its category at
//! the call site — `kairos_profile::scope!(Category::Physics, ...)` appends a
//! `Physics/` tag to the marker name — and the executor tags the sample data of
//! its shared `Kairos.System` scope the same way. Classification is therefore
//! part of the code that emits the marker; only third-party scopes (wgpu,
//! rapier, epaint) are still read from their source crate.
//!
//! Core-module systems (physics, audio, rendering, editor) fold into their
//! module's bucket: they never get a legend row of their own. Everything else is
//! `Scripts` (a system's default when it declares nothing), and the legend
//! refines it into one row per ECS system. Legend swatches are toggles, like
//! Unity's: a hidden category keeps its row and its time and leaves the chart.
//! Scheduler wrappers (`Kairos.Stage`, `Kairos.Engine.Frame`) contribute no time
//! of their own: what they measure is their children's work, counted where it
//! ran, and the remainder becomes `Others`.
//!
//! The timeline paints every block in its category's colour and labels it
//! `Category · marker (duration)` (a system block uses the short system name
//! from its data, and the duration is the block's inclusive time — the number
//! Unity prints after the name). That tag is a property of each block, not a
//! contiguous region: with ECS parallelism one module's systems can run on
//! several workers or interleave with other work on a lane. The block still
//! reads as its module — for example a third-party
//! `Surface::get_current_texture` block reads as
//! `VSync · Surface::get_current_texture (0.021ms)`.
//!
//! # Module area and timeline split
//!
//! The two bands share the space under the toolbar and are separated by a
//! draggable splitter — Unity's `TwoPaneSplitView` in `ProfilerWindow`, which
//! persists the fixed pane's height as `MainSplitView.FixedPaneSize`. The module
//! pane is the fixed pane: it keeps the height the user last dragged to and the
//! timeline takes whatever is left, so resizing the panel grows the timeline.
//!
//! The module pane is a **window onto a stack of fixed-height module cards**
//! (CPU Usage, then the empty GPU Usage card), like Unity's modules list. The
//! cards keep their heights, so dragging the splitter down reveals the modules
//! below instead of stretching the CPU chart, and the wheel or the pane's scroll
//! bar moves a stack taller than the pane. The splitter cannot grow the pane past
//! the whole stack, so there is never dead space under the last card. A
//! double-click restores the skin's `module_area_height`.
//!
//! Both panes have a minimum (the module pane keeps its first title row and two
//! legend rows; the timeline needs its header, ruler, one thread row and the
//! horizontal bar). When the panel is shorter than both, the module pane keeps
//! its minimum and the timeline gives way first, which is what Unity's
//! `UpdateLayout` does when the flexed pane no longer fits.
//!
//! The rule is [`kairos_editor_widgets::splitter::Splitter`], the shared strip
//! widget; it reports the drag and the panel keeps the size, because the panel
//! paints measured chrome rather than nesting layouts. Its idle line has its own
//! token (`splitter_idle`), unlike the section rules, so the handle reads as
//! draggable chrome at a glance.
//!
//! # Timeline lanes
//!
//! Rows come from the capture's registered thread lanes — the frame thread plus
//! every task-pool worker — not from the selected frame's threads, so a worker
//! that ran nothing keeps its row instead of the tracks shifting between frames.
//! Only the tracks scroll: the header, the ruler and the horizontal scroll bar
//! sit outside the row viewport, and the thread names never pan.
//!
//! # Timeline navigation
//!
//! The tracks show a window into the selected frame, Unity's `ZoomableArea`:
//! [`TimelineView`] keeps it as a fraction of the frame, so stepping frames or
//! resizing the panel keeps the zoom. The gestures are Unity's CPU-module ones:
//!
//! * the wheel over the time area zooms horizontally around the pointer, down
//!   to Unity's `minWidth` of 0.05 ms and out to the whole frame (the capture
//!   has no neighbouring frames to reveal past the edges);
//! * the wheel over the thread names or the vertical bar scrolls the rows;
//! * the wheel over the horizontal bar pans;
//! * middle-drag, or alt-drag, pans the tracks; alt+right-drag zooms;
//! * `A` fits the whole frame again, Unity's FrameAll.
//!
//! The two bars are [`kairos_editor_widgets::min_max_scroll::MinMaxScroll`],
//! Unity's min-max scroller: the horizontal one's thumb is the shown window, so
//! dragging it pans and dragging either end resizes it — the timeline's second
//! zoom gesture. egui's `ScrollArea` bar is a plain thumb that cannot resize a
//! window, which is why the timeline lays out its own bars.
//!
//! # Timeline selection
//!
//! Clicking a block selects it, like Unity: the block keeps a 1 px outline and a
//! details box opens under it with `Copy` / `Show` buttons. The box reports the
//! block's own time and, only when that adds information, the marker's
//! accumulated time in the selected frame — like Unity's
//! `ProfilerTimelineGUI.DoSelectionTooltip`: the section needs more than one
//! instance in the frame, the thread line more than one instance on the block's
//! thread, and the frame line instances on other threads too. A block that ran
//! once on one thread therefore shows nothing but its name and time. `Copy` puts
//! that text on the clipboard. Clicking the same block again, or empty track,
//! dismisses it. A click also turns Live off: the box describes one frame, and a
//! capture that is still recording would replace that frame every tick.
//!
//! # Not implemented yet (the seams are marked `TODO(profiler)` in the code)
//!
//! * module selection (`Profiler Modules`) and the Hierarchy / Raw / Call Stacks
//!   view selector — only the timeline view is rebuilt;
//! * thread grouping (Unity's JOB / LOADING / SCRIPTING rows) and resizable
//!   thread rows: the vertical bar scrolls, but the rows have no splitter and no
//!   vertical zoom (Unity locks the timeline's vertical scale);
//! * the full per-system ranking: the legend only has the rows the module area
//!   leaves free, so it shows the top few and `+N more`; a details pane (Unity's
//!   Hierarchy view) is where the whole list belongs;
//! * GPU timings, per-marker colour overrides (the timeline uses the category
//!   palette), hover details (a block reports after a click, not on hover) and
//!   the details box's Show menu (Unity lists the block in the Hierarchy / Raw
//!   views, opens its call stack and jumps to the source line);
//! * the tooltip's cross-frame line (Unity's `0.53ms total over 3 frames on
//!   thread '…'`): it needs samples that span frames to be linked, and the
//!   capture currently parses each frame on its own;
//! * Unity's range selection (drag across the tracks to measure a window), the
//!   `F` shortcut that frames the selected block, and the ghost frames Unity
//!   draws before and after the selected one while the timeline is panned.
//!
//! The data-phase views that predate the rewrite (hierarchy / raw / JSON) are
//! still in this file but are no longer wired into [`Panel::ui`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::{
    Align2, Context, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetText,
};
use kairos_editor_core::PanelId;
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::min_max_scroll::{MinMaxScroll, MinMaxScrollStyle, ScrollWindow};
use kairos_editor_widgets::paint;
use kairos_editor_widgets::splitter::Splitter;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::ui_image;
use kairos_profile::profiling::puffin::{
    self, GlobalFrameView, MergeScope, NanoSecond, ScopeCollection, StreamInfo, ThreadInfo,
    UnpackedFrameData,
};
use kairos_profile::{
    CaptureFiles, Category, FrameSnapshot, HierarchyRow, ProfileSession, Profiler, SYSTEM_MARKER,
    SampleSnapshot, ScopeInfo, ThreadSnapshot, classify_system, hierarchy, strip_tag,
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
    /// The timeline's rows: every registered thread lane (whether or not it ran
    /// in the selected frame) followed by threads that only appeared in the
    /// frames. Fixed per capture, so rows do not jump between frames.
    lanes: Vec<Lane>,
    /// Filled lazily when the JSON tab is first shown; cleared on rebuild.
    json: String,
}

/// One timeline row: a thread, whether or not it reported work this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Lane {
    /// Session thread id, once the thread has reported a frame.
    id: Option<u32>,
    /// Thread name, or the empty string for an unnamed thread.
    name: String,
}

impl Lane {
    fn label(&self) -> String {
        match (self.name.is_empty(), self.id) {
            (false, Some(id)) => format!("{} (#{id})", self.name),
            // Registered but never reported: the row is dimmed, the label is
            // just the thread's name.
            (false, None) => self.name.clone(),
            (true, Some(id)) => format!("Thread {id}"),
            (true, None) => "Unknown thread".to_owned(),
        }
    }
}

/// The timeline block the user selected.
///
/// Samples carry no stable id, so the selection addresses one by position: the
/// frame it was clicked in, the timeline row, and the index in that thread's
/// pre-order sample list. Stepping to another frame makes it stale, so
/// [`ProfilerPanel::timeline`] clears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    /// Frame ordinal the block belongs to.
    frame: usize,
    /// Index into the timeline's lanes.
    lane: usize,
    /// Index into the lane's thread samples.
    sample: usize,
}

/// The slice of a frame the timeline's tracks show.
///
/// Unity's timeline is a `ZoomableArea`: a window into the frame time that the
/// wheel, the scroll bar's ends and the `A` key move and resize. The window is
/// stored as fractions of the frame duration, so stepping frames, resizing the
/// panel or recording again keeps the zoom instead of jumping to an absolute
/// time range.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TimelineView {
    /// Left edge of the window, as a fraction of the frame (`0..=1`).
    pan: f64,
    /// Width of the window, as a fraction of the frame (`0..=1`); 1 fits the
    /// whole frame.
    span: f64,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            pan: 0.0,
            span: 1.0,
        }
    }
}

impl TimelineView {
    /// The narrowest window the timeline zooms to: Unity's
    /// `ZoomableArea.minWidth` (0.05 ms, the shortest slice its timeline reads).
    const MIN_VISIBLE_NS: f64 = 50_000.0;

    /// The narrowest `span` a frame of `duration_ns` may be shown at.
    fn min_span(duration_ns: u64) -> f64 {
        if duration_ns == 0 {
            1.0
        } else {
            (Self::MIN_VISIBLE_NS / duration_ns as f64).clamp(1e-9, 1.0)
        }
    }

    /// Keep the window inside the frame.
    fn clamp(&mut self, duration_ns: u64) {
        let min_span = Self::min_span(duration_ns);
        self.span = self.span.clamp(min_span, 1.0);
        self.pan = self.pan.clamp(0.0, 1.0 - self.span);
    }

    /// Zoom by `factor` (> 1 zooms in) around `focus`, a position across the
    /// track from 0 to 1: the time under the pointer stays under it, like
    /// Unity's `ZoomableArea.SetScaleFocused`.
    fn zoom_around(&mut self, factor: f64, focus: f64, duration_ns: u64) {
        let min_span = Self::min_span(duration_ns);
        let focus = focus.clamp(0.0, 1.0);
        let focus_time = self.pan + focus * self.span;
        let span = (self.span / factor).clamp(min_span, 1.0);
        self.span = span;
        self.pan = (focus_time - focus * span).clamp(0.0, 1.0 - span);
    }

    /// Move the window by `delta` of the frame.
    fn pan_by(&mut self, delta: f64, duration_ns: u64) {
        self.pan += delta;
        self.clamp(duration_ns);
    }

    /// The window in milliseconds: `(start, width)`.
    fn window_ms(&self, duration_ns: u64) -> (f64, f64) {
        let duration_ms = duration_ns as f64 / 1_000_000.0;
        (duration_ms * self.pan, duration_ms * self.span)
    }
}

/// The rectangles the timeline lays out for one pass.
///
/// The header and ruler are pinned; only `body` scrolls. `track_left` and
/// `track_right` bound the zoomable time area: the thread name column ends at
/// `track_left` (it does not pan) and the vertical scroll bar starts at
/// `track_right`.
#[derive(Clone, Copy, Debug)]
struct TimelineLayout {
    header: Rect,
    ruler: Rect,
    body: Rect,
    horizontal_bar: Rect,
    vertical_bar: Rect,
    track_left: f32,
    track_right: f32,
}

impl TimelineLayout {
    fn track_width(&self) -> f32 {
        (self.track_right - self.track_left).max(0.0)
    }

    /// Whether `pos` is over the zoomable time area (ruler or tracks).
    fn time_area_contains(&self, pos: egui::Pos2) -> bool {
        (self.ruler.contains(pos) || self.body.contains(pos))
            && pos.x >= self.track_left
            && pos.x < self.track_right
    }

    /// Whether `pos` is over any part of the timeline.
    fn contains(&self, pos: egui::Pos2) -> bool {
        self.header.contains(pos)
            || self.ruler.contains(pos)
            || self.body.contains(pos)
            || self.horizontal_bar.contains(pos)
            || self.vertical_bar.contains(pos)
    }
}

pub struct Streams {
    streams: Vec<Arc<StreamInfo>>,
    merged_scopes: Vec<MergeScope<'static>>,
    max_depth: usize,
}

impl Streams {
    fn new(
        scope_collection: &ScopeCollection,
        frames: &[Arc<UnpackedFrameData>],
        thread_info: &ThreadInfo,
    ) -> Self {
        let mut streams = vec![];
        for frame in frames {
            if let Some(stream_info) = frame.thread_streams.get(thread_info) {
                streams.push(stream_info.clone());
            }
        }
        let merges =
            puffin::merge_scopes_for_thread(scope_collection, frames, thread_info).unwrap();
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
    fn try_from_iter(
        scope_collection: &ScopeCollection,
        frames: impl Iterator<Item = Arc<UnpackedFrameData>>,
    ) -> Option<Self> {
        let mut it = frames;
        let first = it.next()?;
        let mut frames = vec1::Vec1::new(first);
        frames.extend(it);

        Some(Self::from_vec1(scope_collection, frames))
    }

    fn from_vec1(
        scope_collection: &ScopeCollection,
        mut frames: vec1::Vec1<Arc<UnpackedFrameData>>,
    ) -> Self {
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
    /// Categories the user switched off in the legend, indexed by
    /// [`Category`]. Unity's swatches are toggles: a hidden category keeps its
    /// row and its time, and leaves the chart's stack.
    hidden: [bool; Category::COUNT],
    /// The timeline block the user clicked: it keeps an outline, and the
    /// details box describes it until another block or empty track is clicked.
    selection: Option<Selection>,
    /// Height of the module area in points. The splitter between it and the
    /// timeline keeps this, so the timeline gets the rest of the panel; a drag
    /// replaces it for the session. Unity persists the same value as
    /// `ProfilerWindow.MainSplitView.FixedPaneSize`; the editor has no per-panel
    /// layout settings file to write it to yet.
    module_height: f32,
    /// Vertical offset of the module pane's card stack, in points.
    module_scroll: f32,
    /// The slice of the selected frame the tracks show.
    view: TimelineView,
    /// Vertical offset of the timeline's rows, in points.
    track_scroll: f32,
}

impl Default for ProfilerPanel {
    fn default() -> Self {
        let style = ProfilerStyle::dark();
        Self {
            module_height: style.module_area_height,
            module_scroll: 0.0,
            style,
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
            hidden: [false; Category::COUNT],
            selection: None,
            view: TimelineView::default(),
            track_scroll: 0.0,
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

        // A skin may size the module band differently; the splitter starts at
        // the skin's height and a drag replaces it for the session.
        self.module_height = self.style.module_area_height;

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

        // Unity's window, top to bottom: toolbar, module area, splitter,
        // timeline.
        self.main_toolbar(ui, profiler, &session);
        self.bands(ui, &session);

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
        let button_style = base_style.fills(
            egui::Color32::TRANSPARENT,
            hover,
            self.style.click_bg.to_egui(),
        );
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

    fn timeline_block_font(&self) -> FontId {
        FontId::proportional(self.style.timeline_block_text_size)
    }

    // -----------------------------------------------------------------------
    // Bands: the module pane and the timeline share the space under the
    // toolbar, and the splitter between them keeps the module pane's height.
    // -----------------------------------------------------------------------

    /// The module pane, the splitter and the timeline.
    ///
    /// The module pane is Unity's *fixed pane*: it keeps the height the user
    /// dragged the splitter to and the timeline takes what is left. The
    /// splitter applies its drag to `module_height` on this pass, but the pane
    /// above it was already allocated, so the two panes move on the next pass —
    /// the same one-pass lag a dock separator has. The pane stops growing at the
    /// card stack's height: beyond it there is nothing more to reveal.
    fn bands(&mut self, ui: &mut Ui, session: &ProfileSession) {
        let width = ui.available_width();
        let total = ui.available_height().max(0.0);
        let min_module = self.min_module_height();
        let min_timeline = self.min_timeline_height();
        let cards = self.module_content_height();
        let splitter = self.splitter();
        let max_module = (total - splitter.thickness() - min_timeline)
            .min(cards)
            .max(min_module);
        self.module_height = self.module_height.clamp(min_module, max_module);

        let (module, _) =
            ui.allocate_exact_size(egui::vec2(width, self.module_height), Sense::hover());
        self.module_area(ui, module, session);

        // The widget's own clamp must see the card stack as the far end too,
        // or a fast drag could park the pane under the last module for one pass.
        let splitter_total = total.min(cards + splitter.thickness() + min_timeline);
        let response = splitter
            .limits(min_module, min_timeline)
            .reset_to(self.style.module_area_height)
            .show(
                ui,
                ui.id().with("profiler.splitter"),
                &mut self.module_height,
                splitter_total,
            );
        if response.changed() {
            ui.ctx().request_repaint();
        }

        self.timeline(ui, session);
    }

    /// The splitter between the module area and the timeline, in the panel skin.
    fn splitter(&self) -> Splitter {
        Splitter::horizontal()
            .line(self.style.separator_width)
            .colors(
                self.style.splitter_idle.to_egui(),
                self.style.splitter_hover.to_egui(),
                self.style.splitter_active.to_egui(),
            )
    }

    /// Smallest module pane: its first card's title row and two legend rows.
    fn min_module_height(&self) -> f32 {
        self.style.module_header_height + 2.0 * self.style.legend_row_height
    }

    /// The module card stack's height: the two fixed-height cards and the rule
    /// between them. The pane is a window onto this, and the splitter stops
    /// here so there is no dead space under the last card.
    fn module_content_height(&self) -> f32 {
        self.style.module_cpu_height + self.style.separator_width + self.style.module_gpu_height
    }

    /// Smallest timeline: header, ruler, one thread row, the horizontal bar and
    /// the rules around the ruler.
    fn min_timeline_height(&self) -> f32 {
        self.style.toolbar_height
            + self.style.ruler_height
            + self.style.scrollbar_width
            + self.style.level_height
            + 2.0 * self.style.thread_row_padding
            + 2.0 * self.style.separator_width
    }

    /// The module pane: a window onto the fixed-height module cards, with the
    /// pane's scroll bar on the right edge.
    ///
    /// Unity's modules pane scrolls rather than stretching one chart, so dragging
    /// the splitter down reveals the card below (GPU Usage) and the pane stops at
    /// the whole stack ([`module_content_height`](Self::module_content_height)).
    /// The cards keep painting at their fixed heights and are clipped to the
    /// pane; the wheel over the pane and the pane's bar move the stack.
    fn module_area(&mut self, ui: &mut Ui, area: Rect, session: &ProfileSession) {
        if area.width() <= 0.0 || area.height() <= 0.0 {
            return;
        }
        let bar_width = self.style.scrollbar_width.min(area.width());
        let viewport = Rect::from_min_max(
            area.min,
            egui::pos2(area.right() - bar_width, area.bottom()),
        );
        let bar = Rect::from_min_max(egui::pos2(viewport.right(), area.top()), area.max);

        let rule_width = self.style.separator_width;
        let cpu_height = self.style.module_cpu_height.max(0.0);
        let gpu_height = self.style.module_gpu_height.max(0.0);
        let content_height = self.module_content_height();
        let max_scroll = (content_height - viewport.height()).max(0.0);
        self.module_scroll = self.module_scroll.clamp(0.0, max_scroll);

        // Unity's modules pane scrolls as a list; consume the wheel so nothing
        // behind the pane scrolls as well.
        if ui.rect_contains_pointer(area) {
            let wheel = ui.input(|input| input.smooth_scroll_delta.y);
            if wheel != 0.0 {
                self.module_scroll = (self.module_scroll - wheel).clamp(0.0, max_scroll);
                ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
                ui.ctx().request_repaint();
            }
        }

        // The bar moves the stack on this pass, so the cards below it already
        // paint at the new offset (the timeline paints its rows after its bars
        // for the same reason).
        self.paint_module_scroll_bar(ui, bar, viewport.height(), content_height);

        let content = Rect::from_min_size(
            egui::pos2(viewport.left(), viewport.top() - self.module_scroll),
            egui::vec2(viewport.width(), content_height),
        );
        // The pane is the clip: a card scrolled out of it is neither drawn nor
        // clickable. `scope_builder` moves the parent cursor to the child's used
        // rect, and the cards are painted at explicit rects rather than laid
        // out, so reserve the pane in the child first. A child UI inherits its
        // parent's painter clip, so shrink it to the pane explicitly — otherwise
        // a card taller than the pane paints over the splitter and the timeline
        // header below it.
        let _ = ui.scope_builder(egui::UiBuilder::new().max_rect(viewport), |ui| {
            ui.advance_cursor_after_rect(viewport);
            ui.shrink_clip_rect(viewport);
            let cpu = Rect::from_min_size(content.min, egui::vec2(content.width(), cpu_height));
            self.paint_cpu_module(ui, cpu, session);
            Divider::horizontal()
                .thickness(rule_width)
                .color(self.style.separator.to_egui())
                .paint_in(
                    ui,
                    Rect::from_min_size(
                        egui::pos2(content.left(), cpu.bottom()),
                        egui::vec2(content.width(), rule_width),
                    ),
                );
            let gpu = Rect::from_min_size(
                egui::pos2(content.left(), cpu.bottom() + rule_width),
                egui::vec2(content.width(), gpu_height),
            );
            self.paint_gpu_module(ui, gpu);
        });
    }

    /// The module pane's vertical scroll bar: the shared non-resizable
    /// [`MinMaxScroll`], so it looks and behaves like the timeline's bars.
    fn paint_module_scroll_bar(
        &mut self,
        ui: &mut Ui,
        bar: Rect,
        viewport_height: f32,
        content_height: f32,
    ) {
        if bar.width() <= 0.0 || bar.height() <= 0.0 {
            return;
        }
        let span = f64::from(viewport_height.min(content_height).max(0.0));
        let mut window = ScrollWindow::new(
            f64::from(self.module_scroll),
            span,
            (0.0, f64::from(content_height.max(0.0))),
        );
        let _ = MinMaxScroll::vertical(&self.scroll_style()).show(
            ui,
            ui.id().with("profiler.modules.scroll"),
            bar,
            &mut window,
        );
        self.module_scroll = window.value as f32;
    }

    fn paint_cpu_module(&mut self, ui: &mut Ui, row: Rect, session: &ProfileSession) {
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
            let row_rect = Rect::from_min_max(
                egui::pos2(
                    name_col.left(),
                    center.y - self.style.legend_row_height * 0.5,
                ),
                egui::pos2(
                    name_col.right(),
                    center.y + self.style.legend_row_height * 0.5,
                ),
            );
            // Unity's legend swatches are toggles: a click takes the category
            // out of the chart and leaves the row behind, dimmed.
            let response = ui.interact(
                row_rect,
                ui.id().with(("profiler.category", *category as usize)),
                Sense::click(),
            );
            if response.hovered() {
                ui.painter().rect_filled(
                    row_rect,
                    CornerRadius::ZERO,
                    self.style.hover_bg.to_egui(),
                );
            }
            if response.clicked() {
                let hidden = &mut self.hidden[*category as usize];
                *hidden = !*hidden;
            }
            let hidden = self.hidden[*category as usize];
            let swatch = self.category_color(*category);
            labels.rect_filled(
                Rect::from_center_size(center, egui::vec2(8.0, 8.0)),
                CornerRadius::same(1),
                if hidden {
                    swatch.gamma_multiply(0.3)
                } else {
                    swatch
                },
            );
            labels.text(
                egui::pos2(name_col.left() + 24.0, center.y),
                Align2::LEFT_CENTER,
                category.label(),
                self.toolbar_font(),
                if hidden {
                    self.style.ruler_text.to_egui()
                } else {
                    self.style.timeline_text.to_egui()
                },
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
        if rows_left > 0 && !systems.is_empty() && !self.hidden[Category::Scripts as usize] {
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

    /// The GPU Usage card: a title row with Unity's warning and an empty body
    /// until the capture records GPU timings.
    ///
    /// The card keeps its fixed height so the module pane can reveal it by
    /// scrolling; the body is the slot a graph will occupy once the renderer
    /// reports GPU frame timings into the capture.
    fn paint_gpu_module(&self, ui: &Ui, card: Rect) {
        let name_width = self.style.module_name_width.min(card.width());
        let header = Rect::from_min_size(
            card.min,
            egui::vec2(card.width(), self.style.module_header_height),
        );
        let name_col = Rect::from_min_max(
            card.min,
            egui::pos2(card.left() + name_width, header.bottom()),
        );
        self.paint_module_header(
            ui,
            header,
            name_col,
            "GPU",
            "GPU Usage",
            "timings not captured",
        );

        // Unity's warning triangle: this build cannot report GPU timings.
        let center = egui::pos2(name_col.right() - 14.0, header.center().y);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                egui::pos2(center.x, center.y - 5.0),
                egui::pos2(center.x + 5.0, center.y + 4.0),
                egui::pos2(center.x - 5.0, center.y + 4.0),
            ],
            self.style.warning.to_egui(),
            Stroke::NONE,
        ));

        let body = Rect::from_min_max(egui::pos2(card.left(), header.bottom()), card.max);
        if body.height() > 0.0 {
            ui.painter().text(
                body.center(),
                Align2::CENTER_CENTER,
                "No GPU timing data",
                self.small_font(),
                self.style.ruler_text.to_egui(),
            );
        }
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
    /// reference lines, the 120 FPS baseline and a cursor at the selected frame.
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
                if self.hidden[category as usize] {
                    continue;
                }
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

        // Frame budgets, labelled the way Unity labels them: its 30/60 FPS
        // reference lines plus the engine's 120 FPS baseline. Drawn over the
        // bars so the chips stay readable.
        for (fps, ms) in [
            (30.0_f32, TARGET_30_FPS_MS),
            (60.0, TARGET_60_FPS_MS),
            (120.0, TARGET_120_FPS_MS),
        ] {
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

    /// The timeline rows: registered lanes plus threads seen in the frames.
    fn lanes(&self) -> &[Lane] {
        self.cached
            .as_ref()
            .map_or(&[], |cached| cached.lanes.as_slice())
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
            .filter(|category| !self.hidden[**category as usize])
            .map(|category| self.category_ms(frame, *category))
            .sum()
    }

    // -----------------------------------------------------------------------
    // Timeline — pinned header and ruler, scrollable tracks, zoomable time axis.
    // -----------------------------------------------------------------------

    /// Unity's timeline: a pinned header (view selector, Live, CPU/GPU summary)
    /// and ruler above the thread tracks, and a zoomable time axis between them.
    ///
    /// # Navigation
    ///
    /// The gestures are the CPU-usage module's in Unity's Profiler: the wheel
    /// zooms the time area around the pointer, the wheel over the thread names or
    /// the vertical bar scrolls the rows, the wheel over the horizontal bar pans,
    /// dragging the bar pans, dragging either end of its thumb zooms,
    /// middle-drag (or alt-drag) pans the tracks, and `A` fits the whole frame
    /// again. [`TimelineView`] holds the window and [`MinMaxScroll`] draws the
    /// bars.
    fn timeline(&mut self, ui: &mut Ui, session: &ProfileSession) {
        let width = ui.available_width();
        let rule = self.style.separator.to_egui();
        let rule_width = self.style.separator_width;

        // The header and the ruler are pinned: only the tracks scroll. Unity
        // keeps its toolbar, ruler and thread names outside the zoomable area.
        let (header, _) =
            ui.allocate_exact_size(egui::vec2(width, self.style.toolbar_height), Sense::hover());
        self.paint_timeline_header(ui, header, session);
        ui.add(Divider::horizontal().thickness(rule_width).color(rule));

        // Columns: the thread name column (it does not pan), the zoomable time
        // area, and the vertical scroll bar on the right edge.
        let name_width = self.style.timeline_name_width.min(width);
        let scrollbar_width = self
            .style
            .scrollbar_width
            .min((width - name_width).max(0.0));
        let track_left = header.left() + name_width;
        let track_right = (header.right() - scrollbar_width).max(track_left);

        let (ruler, _) =
            ui.allocate_exact_size(egui::vec2(width, self.style.ruler_height), Sense::hover());
        let remaining = ui.available_height().max(0.0);
        let (area, _) = ui.allocate_exact_size(egui::vec2(width, remaining), Sense::hover());
        let bottom_bar = self.style.scrollbar_width.min(area.height()).max(0.0);
        let body = Rect::from_min_max(
            area.min,
            egui::pos2(track_right, area.bottom() - bottom_bar),
        );
        let layout = TimelineLayout {
            header,
            ruler,
            body,
            horizontal_bar: Rect::from_min_max(
                egui::pos2(track_left, body.bottom()),
                egui::pos2(track_right, area.bottom()),
            ),
            vertical_bar: Rect::from_min_max(
                egui::pos2(track_right, area.top()),
                egui::pos2(area.right(), body.bottom()),
            ),
            track_left,
            track_right,
        };

        let frame = session.frames.get(self.selected_frame).map(Arc::as_ref);
        let duration_ns = frame.map_or(0, |frame| frame.duration_ns());
        self.view.clamp(duration_ns);

        // Row heights come first: the vertical offset is clamped before painting,
        // and the wheel or the vertical bar can move it below.
        let row_heights = self.row_heights(frame);
        let content_height: f32 = row_heights.iter().sum();
        let max_scroll = (content_height - layout.body.height()).max(0.0);
        self.track_scroll = self.track_scroll.clamp(0.0, max_scroll);

        let body_response = ui.interact(
            layout.body,
            ui.id().with("profiler.timeline.tracks"),
            Sense::click_and_drag(),
        );
        self.navigate(ui, &layout, &body_response, duration_ns);
        self.paint_scroll_bars(ui, &layout, content_height, duration_ns);
        self.view.clamp(duration_ns);
        self.track_scroll = self.track_scroll.clamp(0.0, max_scroll);

        let view = self.view;
        let ticks = ruler_ticks(view, duration_ns, layout.track_width());
        let ruler_track = Rect::from_min_max(
            egui::pos2(layout.track_left, layout.ruler.top()),
            egui::pos2(layout.track_right, layout.ruler.bottom()),
        );
        self.paint_ruler(ui, layout.ruler, ruler_track, &ticks);

        let Some(frame) = frame else {
            self.selection = None;
            self.paint_empty_timeline(ui, layout.body);
            return;
        };
        let frame_index = self.selected_frame;

        // The selection addresses one frame's samples: stepping the transport or
        // a new capture makes it stale.
        let stale = self.selection.is_some_and(|selection| {
            selection.frame != frame_index || selection.lane >= self.lanes().len()
        });
        if stale {
            self.selection = None;
        }

        // The name column and the tracks read as one surface; the rows paint
        // themselves over the background.
        let name_column = Rect::from_min_max(
            layout.body.min,
            egui::pos2(layout.track_left, layout.body.bottom()),
        );
        let track_column = Rect::from_min_max(
            egui::pos2(layout.track_left, layout.body.top()),
            layout.body.max,
        );
        ui.painter().rect_filled(
            name_column,
            CornerRadius::ZERO,
            self.style.timeline_name_bg.to_egui(),
        );
        ui.painter().rect_filled(
            track_column,
            CornerRadius::ZERO,
            self.style.track_bg.to_egui(),
        );

        // A click picks the block under the pointer; a second click on the same
        // block — or a click on empty track — dismisses the details box.
        let clicked_at = body_response
            .clicked_by(egui::PointerButton::Primary)
            .then(|| body_response.interact_pointer_pos())
            .flatten();
        let mut clicked: Option<(usize, usize, Rect)> = None;
        let mut selected_bar = None;
        let mut y = layout.body.top() - self.track_scroll;
        for (lane_index, lane) in self.lanes().iter().enumerate() {
            let height = row_heights.get(lane_index).copied().unwrap_or(0.0);
            let row = Rect::from_min_size(
                egui::pos2(layout.body.left(), y),
                egui::vec2(layout.body.width(), height),
            );
            y += height;
            if !row.intersects(layout.body) {
                continue;
            }
            let thread = lane
                .id
                .and_then(|id| frame.threads.iter().find(|thread| thread.id == id));
            let selected = self
                .selection
                .and_then(|selection| (selection.lane == lane_index).then_some(selection.sample));
            let outcome = self.paint_thread_row(
                ui,
                clicked_at,
                row,
                layout.body,
                lane,
                thread,
                &session.scopes,
                view,
                &ticks,
                duration_ns,
                selected,
            );
            if let Some((sample, bar)) = outcome.clicked {
                clicked = Some((lane_index, sample, bar));
            }
            if let Some(bar) = outcome.selected {
                selected_bar = Some(bar);
            }
        }

        if let Some((lane, sample, bar)) = clicked {
            let hit = Selection {
                frame: frame_index,
                lane,
                sample,
            };
            self.selection = (self.selection != Some(hit)).then_some(hit);
            selected_bar = self.selection.is_some().then_some(bar);
            if self.selection.is_some() {
                // The box describes one frame; pin the capture so the next
                // recorded frame cannot swap it out from under the click.
                self.follow_latest = false;
                // This pass painted the row before the click was known, so paint
                // the outline on the newly selected block now; the next pass
                // paints it from the selection.
                ui.painter().rect_stroke(
                    bar,
                    CornerRadius::same(1),
                    Stroke::new(1.0, self.style.selection_outline.to_egui()),
                    StrokeKind::Inside,
                );
            }
            ui.ctx().request_repaint();
        } else if body_response.clicked_by(egui::PointerButton::Primary) {
            self.selection = None;
            ui.ctx().request_repaint();
        }

        // The details box floats over the tracks, after the fill so the fill
        // cannot cover it.
        let Some(bar) = selected_bar else {
            return;
        };
        let Some(selection) = self.selection else {
            return;
        };
        let Some(details) = selection_details(frame, &session.scopes, self.lanes(), selection)
        else {
            return;
        };
        self.paint_selection_details(ui, layout.body, bar, &details);
    }

    /// Unity's timeline navigation, applied before the pass paints so a wheel
    /// event or a bar drag shows in the same frame.
    fn navigate(
        &mut self,
        ui: &mut Ui,
        layout: &TimelineLayout,
        body: &Response,
        duration_ns: u64,
    ) {
        let track_width = layout.track_width();
        let pointer = ui.input(|i| i.pointer.hover_pos());

        // The wheel: over the time area it zooms around the pointer (Unity's
        // ZoomableArea), over the names or the vertical bar it scrolls the rows,
        // over the horizontal bar it pans.
        let wheel = ui.input(|i| i.smooth_scroll_delta);
        if wheel != egui::Vec2::ZERO && track_width > 0.0 {
            if let Some(pos) = pointer {
                let used = if layout.time_area_contains(pos) {
                    let focus =
                        f64::from(((pos.x - layout.track_left) / track_width).clamp(0.0, 1.0));
                    if wheel.y != 0.0 {
                        let factor = ZOOM_PER_NOTCH.powf(f64::from(wheel.y) / WHEEL_NOTCH_POINTS);
                        self.view.zoom_around(factor, focus, duration_ns);
                    }
                    if wheel.x != 0.0 {
                        let delta = -f64::from(wheel.x) / f64::from(track_width) * self.view.span;
                        self.view.pan_by(delta, duration_ns);
                    }
                    true
                } else if layout.body.contains(pos) || layout.vertical_bar.contains(pos) {
                    self.track_scroll -= wheel.y;
                    true
                } else if layout.horizontal_bar.contains(pos) {
                    let step = f64::from(wheel.y) / WHEEL_NOTCH_POINTS * self.view.span * 0.1;
                    self.view.pan_by(-step, duration_ns);
                    true
                } else {
                    false
                };
                if used {
                    // Consume the delta the way a ScrollArea does, so nothing
                    // above the timeline scrolls as well.
                    ui.input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
                    ui.ctx().request_repaint();
                }
            }
        }

        // `A` frames the whole frame again: Unity's FrameAll shortcut.
        let over_timeline = pointer.is_some_and(|pos| layout.contains(pos));
        if over_timeline
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::A))
        {
            self.view = TimelineView::default();
            ui.ctx().request_repaint();
        }

        // Middle-drag pans the time area; alt-drag does the same for mice without
        // a middle button. Alt+right-drag zooms around the press point.
        let alt = ui.input(|i| i.modifiers.alt);
        if alt && body.dragged_by(egui::PointerButton::Secondary) {
            let focus = ui
                .input(|i| i.pointer.press_origin())
                .map_or(0.5, |origin| {
                    f64::from(((origin.x - layout.track_left) / track_width).clamp(0.0, 1.0))
                });
            let delta = body.drag_delta();
            let factor = 1.0 + f64::from(delta.x + delta.y) * 0.01;
            if factor > 0.01 {
                self.view.zoom_around(factor, focus, duration_ns);
            }
            ui.ctx().request_repaint();
        } else if body.dragged_by(egui::PointerButton::Middle)
            || (alt && body.dragged_by(egui::PointerButton::Primary))
        {
            let dx = body.drag_delta().x;
            self.view.pan_by(
                -f64::from(dx) / f64::from(track_width) * self.view.span,
                duration_ns,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            ui.ctx().request_repaint();
        }
    }

    /// The timeline's scroll bars and the two side-pane corners Unity fills next
    /// to them.
    ///
    /// The vertical bar is a plain scrollbar over the row heights. The horizontal
    /// one is Unity's min-max scroller over the frame: its thumb is the shown
    /// window, so dragging it pans and dragging either end zooms.
    fn paint_scroll_bars(
        &mut self,
        ui: &mut Ui,
        layout: &TimelineLayout,
        content_height: f32,
        duration_ns: u64,
    ) {
        let style = self.scroll_style();
        let chrome = self.style.timeline_name_bg.to_egui();
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(layout.header.left(), layout.body.bottom()),
                egui::pos2(layout.track_left, layout.horizontal_bar.bottom()),
            ),
            CornerRadius::ZERO,
            chrome,
        );
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(layout.track_right, layout.body.bottom()),
                egui::pos2(layout.header.right(), layout.horizontal_bar.bottom()),
            ),
            CornerRadius::ZERO,
            chrome,
        );

        // The rows: a plain scrollbar, because the timeline's vertical scale is
        // locked (Unity's `vZoomLockedByDefault`).
        let viewport = layout.body.height();
        let content = content_height.max(viewport).max(1.0);
        let mut window = ScrollWindow::new(
            f64::from(self.track_scroll),
            f64::from(viewport.min(content)),
            (0.0, f64::from(content)),
        )
        .with_min_span(f64::from(viewport.min(content)));
        let scrolled = MinMaxScroll::vertical(&style)
            .show(
                ui,
                ui.id().with("profiler.timeline.vscroll"),
                layout.vertical_bar,
                &mut window,
            )
            .changed();
        if scrolled {
            self.track_scroll = window.value as f32;
            ui.ctx().request_repaint();
        }

        // The frame time: Unity's min-max scroller. Its thumb pans, its two ends
        // resize the window — which is how the timeline zooms.
        let mut window = ScrollWindow::new(self.view.pan, self.view.span, (0.0, 1.0))
            .with_min_span(TimelineView::min_span(duration_ns));
        let zoomed = MinMaxScroll::horizontal(&style)
            .resizable(true)
            .show(
                ui,
                ui.id().with("profiler.timeline.hscroll"),
                layout.horizontal_bar,
                &mut window,
            )
            .changed();
        if zoomed {
            self.view.pan = window.value;
            self.view.span = window.span;
            ui.ctx().request_repaint();
        }
    }

    /// The scroll bars' colours, from the panel's style tokens.
    fn scroll_style(&self) -> MinMaxScrollStyle {
        MinMaxScrollStyle {
            trough: self.style.scrollbar_bg.to_egui(),
            thumb: self.style.scrollbar_thumb.to_egui(),
            thumb_hover: self.style.scrollbar_thumb_hover.to_egui(),
            thumb_active: self.style.scrollbar_thumb_active.to_egui(),
            grip: self.style.scrollbar_grip.to_egui(),
            handle_width: self.style.scrollbar_handle_width,
        }
    }

    /// The painted height of each timeline row, in lane order.
    fn row_heights(&self, frame: Option<&FrameSnapshot>) -> Vec<f32> {
        self.lanes()
            .iter()
            .map(|lane| {
                let depth = frame
                    .and_then(|frame| {
                        lane.id
                            .and_then(|id| frame.threads.iter().find(|thread| thread.id == id))
                    })
                    .map_or(1, |thread| thread.depth().max(1));
                depth as f32 * self.style.level_height + 2.0 * self.style.thread_row_padding
            })
            .collect()
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
        let track_left = (row.left() + self.style.timeline_name_width).min(row.right());
        let track_right = (row.right() - self.style.scrollbar_width).max(track_left);
        ui.painter().text(
            egui::pos2((track_left + track_right) * 0.5, row.center().y),
            Align2::CENTER_CENTER,
            summary,
            self.small_font(),
            self.style.timeline_text.to_egui(),
        );
    }

    /// The time ruler over the track column, and the name column's chrome beside
    /// it.
    ///
    /// Ticks are absolute frame times from `ticks` (see [`ruler_ticks`]), so the
    /// grid lines in the tracks line up with them at every zoom.
    fn paint_ruler(&self, ui: &Ui, row: Rect, track: Rect, ticks: &[RulerTick]) {
        let painter = ui.painter();
        painter.rect_filled(
            row,
            CornerRadius::ZERO,
            self.style.timeline_name_bg.to_egui(),
        );
        painter.rect_filled(track, CornerRadius::ZERO, self.style.ruler_bg.to_egui());
        let labels = painter.with_clip_rect(track);

        for tick in ticks {
            let x = track.left() + tick.x;
            if x > track.right() {
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

        // Rule between the name column and the tracks, like the thread rows have.
        self.paint_vertical_rule(
            ui,
            egui::pos2(track.left() - self.style.separator_width, row.top()),
            row.height(),
            self.style.separator.to_egui(),
        );

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

    /// One thread's track: the name column, the grid and the sample blocks.
    ///
    /// `clicked_at` is where the pass's click landed, when the track viewport took
    /// one. `clip` is the row viewport: a row scrolled partly out of it is cut,
    /// not drawn over the ruler or the horizontal bar. Returns what the pass
    /// produced for the timeline's selection: the block under the click, and the
    /// selected block's rect so the details box can anchor to it.
    #[allow(clippy::too_many_arguments)]
    fn paint_thread_row(
        &self,
        ui: &Ui,
        clicked_at: Option<egui::Pos2>,
        row: Rect,
        clip: Rect,
        lane: &Lane,
        thread: Option<&ThreadSnapshot>,
        scopes: &[ScopeInfo],
        view: TimelineView,
        ticks: &[RulerTick],
        duration_ns: u64,
        selected: Option<usize>,
    ) -> RowOutcome {
        let mut outcome = RowOutcome::default();
        let name_width = self.style.timeline_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        let track = Rect::from_min_max(egui::pos2(name_col.right(), row.top()), row.max);
        let painter = ui.painter().with_clip_rect(clip);

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
            lane.label(),
            self.toolbar_font(),
            // A lane that has not reported yet reads as idle, not active.
            if lane.id.is_some() {
                self.style.timeline_text.to_egui()
            } else {
                self.style.ruler_text.to_egui()
            },
        );

        // Grid lines aligned with the labelled ruler ticks.
        for tick in ticks.iter().filter(|tick| tick.label.is_some()) {
            let x = track.left() + tick.x;
            if x >= track.right() - 0.5 {
                continue;
            }
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x, track.top()),
                    egui::vec2(self.style.separator_width, track.height()),
                ),
                CornerRadius::ZERO,
                self.style.grid.to_egui(),
            );
        }

        // Column rule between the names and the tracks, inside the name column.
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(name_col.right() - self.style.separator_width, row.top()),
                egui::vec2(self.style.separator_width, row.height()),
            ),
            CornerRadius::ZERO,
            self.style.separator.to_egui(),
        );

        if duration_ns > 0 {
            let mut labels = MAX_SAMPLE_LABELS;
            let samples = thread.map_or(&[][..], |thread| thread.samples.as_slice());
            for (index, sample) in samples.iter().enumerate() {
                let Some((x0, x1)) = block_span(sample, track, view, duration_ns) else {
                    continue;
                };
                let top = track.top()
                    + self.style.thread_row_padding
                    + sample.depth as f32 * self.style.level_height;
                let height = (self.style.level_height - 1.0).max(1.0);
                let bar = Rect::from_min_max(
                    egui::pos2(x0, top),
                    egui::pos2(x1.max(x0 + 1.0), top + height),
                );

                let info = scope_info(scopes, sample.scope);
                let category = sample_category(sample, info);
                let color = self.category_color(category);
                painter.rect_filled(bar, CornerRadius::same(1), color);
                painter.rect_stroke(
                    bar,
                    CornerRadius::same(1),
                    Stroke::new(1.0, color.gamma_multiply(0.55)),
                    StrokeKind::Inside,
                );

                // Unity marks the selected block with a 1 px light frame over
                // its category colour.
                if selected == Some(index) {
                    outcome.selected = Some(bar);
                    painter.rect_stroke(
                        bar,
                        CornerRadius::same(1),
                        Stroke::new(1.0, self.style.selection_outline.to_egui()),
                        StrokeKind::Inside,
                    );
                }

                // A click picks the block under the pointer. Blocks on one row
                // never overlap, but deeper samples are painted later, so the
                // last match is the topmost one.
                if let Some(pos) = clicked_at {
                    if bar.contains(pos) {
                        outcome.clicked = Some((index, bar));
                    }
                }

                // Labels are expensive; only paint them where they fit, and cap
                // them per row so a dense frame stays interactive.
                if labels > 0 && bar.width() >= 34.0 {
                    if let Some(info) = info {
                        labels -= 1;
                        let label = sample_label(sample, Some(info));
                        let clip = bar.shrink2(egui::vec2(2.0, 0.0));
                        painter.with_clip_rect(clip).text(
                            egui::pos2(clip.left() + 1.0, bar.center().y),
                            Align2::LEFT_CENTER,
                            &label,
                            self.timeline_block_font(),
                            self.style.sample_text.to_egui(),
                        );
                    }
                }
            }
        }

        // Rule under the row.
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(row.left(), row.bottom() - self.style.separator_width),
                egui::vec2(row.width(), self.style.separator_width),
            ),
            CornerRadius::ZERO,
            self.style.row_rule.to_egui(),
        );

        outcome
    }

    // -----------------------------------------------------------------------
    // Timeline selection — the selected block's outline and Unity's details box.
    // -----------------------------------------------------------------------

    /// Unity's details box for the selected block.
    ///
    /// It floats over the tracks, anchored under the clicked block and clamped
    /// inside `viewport` (the timeline's track area). The box swallows clicks
    /// (only another block or empty track dismisses it) and its buttons are
    /// registered after it, which keeps them on top for egui's hit test.
    fn paint_selection_details(
        &self,
        ui: &mut Ui,
        viewport: Rect,
        anchor: Rect,
        details: &SelectionDetails,
    ) {
        let style = &self.style;
        let padding = 8.0;
        let gap = 2.0;
        let section_gap = 8.0;
        let body_font = FontId::proportional(style.small_text_size + 1.0);
        let wrap = (style.selection_width - 2.0 * padding).max(80.0);
        let painter = ui.painter();

        // Measure first: the box is sized by its content, like Unity's.
        let title = painter.layout(
            details.title.clone(),
            FontId::proportional(style.toolbar_text_size + 1.0),
            style.sample_text.to_egui(),
            wrap,
        );
        let duration = painter.layout(
            duration_text(details.duration_ns),
            body_font.clone(),
            style.sample_text.to_egui(),
            wrap,
        );
        let body: Vec<_> = details
            .accumulated_lines()
            .into_iter()
            .map(|line| painter.layout(line, body_font.clone(), style.ruler_text.to_egui(), wrap))
            .collect();

        // Unity's buttons: Copy and Show (with a caret), measured from their
        // labels so the box grows with the text.
        let button_height = style.toolbar_height;
        let measure = |text: &str| {
            painter
                .layout_no_wrap(
                    text.to_owned(),
                    body_font.clone(),
                    style.toolbar_text.to_egui(),
                )
                .size()
                .x
        };
        let copy_width = (measure("Copy") + 24.0).max(54.0);
        let show_width = (measure("Show") + 36.0).max(54.0);

        let title_size = title.size();
        let duration_size = duration.size();
        let body_sizes: Vec<egui::Vec2> = body.iter().map(|line| line.size()).collect();
        let mut content_width = title_size.x.max(duration_size.x);
        let mut content_height = title_size.y + gap + duration_size.y;
        for (index, size) in body_sizes.iter().enumerate() {
            content_width = content_width.max(size.x);
            content_height += if index == 0 { section_gap } else { gap } + size.y;
        }
        content_width = content_width.max(copy_width + 3.0 * gap + show_width);
        let width = (content_width + 2.0 * padding).min(style.selection_width);
        let height = content_height + section_gap + button_height + 2.0 * padding;

        let viewport = viewport.intersect(ui.clip_rect());
        if !viewport.intersects(anchor) {
            return;
        }
        let mut pos = egui::pos2(anchor.left(), anchor.bottom() + 4.0);
        if pos.y + height > viewport.bottom() - 4.0 {
            pos.y = anchor.top() - height - 4.0;
        }
        pos.y = pos.y.clamp(
            viewport.top() + 4.0,
            (viewport.bottom() - height - 4.0).max(viewport.top() + 4.0),
        );
        pos.x = pos.x.clamp(
            viewport.left() + 4.0,
            (viewport.right() - width - 4.0).max(viewport.left() + 4.0),
        );
        let box_rect = Rect::from_min_size(pos, egui::vec2(width, height));
        let painter = painter.with_clip_rect(viewport);

        // Unity's popup: a raised dark panel with a drop shadow.
        painter.add(
            egui::epaint::Shadow {
                offset: [0, 3],
                blur: 10,
                spread: 0,
                color: egui::Color32::from_black_alpha(120),
            }
            .as_shape(box_rect, CornerRadius::same(3)),
        );
        painter.rect_filled(
            box_rect,
            CornerRadius::same(3),
            style.selection_bg.to_egui(),
        );
        painter.rect_stroke(
            box_rect,
            CornerRadius::same(3),
            Stroke::new(1.0, style.selection_border.to_egui()),
            StrokeKind::Inside,
        );

        // The box takes clicks away from the track rows underneath; its buttons
        // are added after it, so they win the hit test.
        let _ = ui.interact(
            box_rect,
            ui.id().with("profiler.selection.box"),
            Sense::click(),
        );

        let left = box_rect.left() + padding;
        let mut top = box_rect.top() + padding;
        // Unity's title and duration are the brightest text in the box; the
        // accumulated lines below are dim.
        let text_color = style.sample_text.to_egui();
        painter.galley(egui::pos2(left, top), title, text_color);
        top += title_size.y + gap;
        painter.galley(egui::pos2(left, top), duration, text_color);
        top += duration_size.y;
        // The accumulated section is optional (Unity omits it for a single
        // instance), so only spend the gap between the time and the buttons when
        // there is something in between.
        if !body_sizes.is_empty() {
            top += section_gap;
        }
        for (index, line) in body.into_iter().enumerate() {
            if index > 0 {
                top += gap;
            }
            painter.galley(egui::pos2(left, top), line, style.ruler_text.to_egui());
            top += body_sizes[index].y;
        }
        top += section_gap;

        let copy_rect =
            Rect::from_min_size(egui::pos2(left, top), egui::vec2(copy_width, button_height));
        if self
            .paint_selection_button(ui, copy_rect, "Copy", false)
            .clicked()
        {
            ui.ctx().copy_text(details.copy_text());
        }
        let show_rect = copy_rect.translate(egui::vec2(copy_width + 3.0 * gap, 0.0));
        if self
            .paint_selection_button(ui, show_rect, "Show", true)
            .clicked()
        {
            // TODO(profiler): Unity's Show menu lists the block in the Hierarchy
            // and Raw views, opens its call stack and jumps to the source line.
            // The capture carries each marker's `file:line`, so the menu can be
            // wired once those views are back in the panel.
        }
    }

    /// One button of the details box: Unity's small raised Copy / Show chips.
    fn paint_selection_button(
        &self,
        ui: &mut Ui,
        rect: Rect,
        label: &str,
        caret: bool,
    ) -> Response {
        let response = ui.interact(
            rect,
            ui.id().with(("profiler.selection.button", label)),
            Sense::click(),
        );
        let fill = if response.is_pointer_button_down_on() {
            self.style.click_bg.to_egui().gamma_multiply(0.85)
        } else if response.hovered() {
            self.style.click_bg.to_egui()
        } else {
            self.style.hover_bg.to_egui()
        };
        let color = self.style.toolbar_text.to_egui();
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(2), fill);
        painter.rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0, self.style.selection_border.to_egui()),
            StrokeKind::Inside,
        );
        let font = FontId::proportional(self.style.small_text_size + 1.0);
        let center = if caret {
            rect.center() - egui::vec2(4.0, 0.0)
        } else {
            rect.center()
        };
        painter.text(center, Align2::CENTER_CENTER, label, font, color);
        if caret {
            paint::caret(
                painter,
                egui::pos2(rect.right() - 10.0, rect.center().y),
                color,
            );
        }
        response
    }

    fn paint_empty_timeline(&self, ui: &Ui, area: Rect) {
        if area.height() <= 0.0 {
            return;
        }
        let painter = ui.painter().with_clip_rect(area);
        painter.rect_filled(area, CornerRadius::ZERO, self.style.track_bg.to_egui());
        painter.text(
            area.center(),
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
        let lanes = collect_lanes(session);
        self.cached = Some(CachedFrame {
            frame,
            scopes: session.scopes.clone(),
            rows,
            systems,
            lanes,
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
        // The selection addresses one frame of the capture being cleared.
        self.selection = None;
        // The zoom describes a frame of the capture being cleared.
        self.view = TimelineView::default();
        self.track_scroll = 0.0;
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

fn thread_label(thread: &ThreadSnapshot) -> String {
    if thread.name.is_empty() {
        format!("Thread {}", thread.id)
    } else {
        format!("{} (#{})", thread.name, thread.id)
    }
}

fn scope_info(scopes: &[ScopeInfo], id: u32) -> Option<&ScopeInfo> {
    scopes
        .binary_search_by_key(&id, |scope| scope.id)
        .ok()
        .map(|index| &scopes[index])
}

/// The module a timeline block belongs to.
///
/// Most markers answer from their scope metadata, which the parser filled from
/// the call site's tag (or from the fallback table for third-party scopes). The
/// executor's shared `Kairos.System` marker is the exception: which system ran,
/// and therefore which module it belongs to, is per-sample data.
fn sample_category(sample: &SampleSnapshot, scope: Option<&ScopeInfo>) -> Category {
    match scope {
        Some(info) if info.name == SYSTEM_MARKER => classify_system(sample.data.as_deref()),
        Some(info) => info.category,
        None => Category::Others,
    }
}

/// The text painted inside a timeline block: `Category · marker (duration)`.
///
/// The category comes first so even a clipped label says what the block is:
/// `Surface::get_current_texture` reads as `VSync · ...`. The duration after
/// the name is the block's inclusive time, the number Unity prints there.
fn sample_label(sample: &SampleSnapshot, scope: Option<&ScopeInfo>) -> String {
    format!(
        "{} · {} ({})",
        sample_category(sample, scope).label(),
        sample_detail(sample, scope),
        block_duration_text(sample.duration_ns),
    )
}

/// The marker's own name, without its category prefix.
///
/// A system sample shows the short system name from its per-call data instead of
/// the executor's shared `Kairos.System` marker.
fn sample_detail(sample: &SampleSnapshot, scope: Option<&ScopeInfo>) -> String {
    match scope {
        Some(info) if info.name == SYSTEM_MARKER => sample
            .data
            .as_deref()
            .map(strip_tag)
            .map(short_system_name)
            .unwrap_or_else(|| info.name.clone()),
        Some(info) => info.name.clone(),
        None => "<unknown scope>".to_owned(),
    }
}

/// Where a sample's block lands in `track` for the shown window: `(left, right)`
/// in points, clamped to the track.
///
/// `None` when the window does not show the block. A block narrower than a
/// hairline keeps one point so a dense frame still reads.
fn block_span(
    sample: &SampleSnapshot,
    track: Rect,
    view: TimelineView,
    duration_ns: u64,
) -> Option<(f32, f32)> {
    let (start_ms, visible_ms) = view.window_ms(duration_ns);
    if visible_ms <= 0.0 || track.width() <= 0.0 {
        return None;
    }
    let to_x = |ns: u64| {
        track.left() + (((ns as f64 / 1_000_000.0) - start_ms) / visible_ms) as f32 * track.width()
    };
    let x0 = to_x(sample.start_ns);
    let x1 = to_x(sample.start_ns.saturating_add(sample.duration_ns)).max(x0 + 1.0);
    if x1 <= track.left() || x0 >= track.right() {
        return None;
    }
    Some((x0.max(track.left()), x1.min(track.right())))
}

fn format_ms(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1_000_000.0)
}

/// The timeline's rows for one capture.
///
/// The registered lanes come first, in the order the host registered them (main
/// thread, then every task-pool worker), each matched to its session thread id
/// once the thread has reported. Threads that were never registered — an
/// audio thread, a pool created after the capture started, a rayon worker —
/// follow in id order, so nothing is hidden.
fn collect_lanes(session: &ProfileSession) -> Vec<Lane> {
    let mut seen: BTreeMap<u32, &str> = BTreeMap::new();
    for frame in &session.frames {
        for thread in &frame.threads {
            seen.entry(thread.id).or_insert(thread.name.as_str());
        }
    }

    let mut lanes = Vec::with_capacity(session.thread_lanes.len() + seen.len());
    let mut used = BTreeSet::new();
    for name in &session.thread_lanes {
        let id = seen
            .iter()
            .find(|(id, thread_name)| **thread_name == name.as_str() && !used.contains(*id))
            .map(|(id, _)| *id);
        if let Some(id) = id {
            used.insert(id);
        }
        lanes.push(Lane {
            id,
            name: name.clone(),
        });
    }
    for (id, name) in seen {
        if used.contains(&id) {
            continue;
        }
        lanes.push(Lane {
            id: Some(id),
            name: name.to_owned(),
        });
    }
    lanes
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
/// The engine's 120 FPS target, drawn as the chart's baseline.
const TARGET_120_FPS_MS: f32 = 1000.0 / 120.0;

/// Sample labels are the expensive part of a track; cap them per thread row.
const MAX_SAMPLE_LABELS: usize = 64;

/// Tick steps, in milliseconds, that read well at any capture length.
const RULER_STEPS_MS: [f64; 18] = [
    0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0,
    1000.0, 2000.0,
];

/// Minimum distance between two labelled ruler ticks, in points.
const MIN_TICK_SPACING: f32 = 80.0;

/// One mouse-wheel notch in egui's points (its `line_scroll_speed`): the wheel's
/// smooth delta is in points, so a notch is 50 of them.
const WHEEL_NOTCH_POINTS: f64 = 50.0;

/// How much one wheel notch zooms the time area over the timeline.
const ZOOM_PER_NOTCH: f64 = 1.2;

/// One tick on the timeline ruler.
#[derive(Clone, Debug, PartialEq)]
struct RulerTick {
    /// Position from the track column's left edge, in points.
    x: f32,
    /// Label at a major tick; `None` for the four minor ticks between majors.
    label: Option<String>,
}

/// Ruler ticks for the window `view` of a frame lasting `duration_ns` drawn
/// across `width` points.
///
/// Ticks sit at absolute frame times (multiples of the step), not at "0, step,
/// 2·step from the left edge", so panning slides them past the pointer the way
/// Unity's `TimeArea` ticks do. The step is the smallest "nice" value that keeps
/// labels at least [`MIN_TICK_SPACING`] apart, so a 2 ms window and a 2 s window
/// both read; each major tick is followed by four unlabelled minors.
fn ruler_ticks(view: TimelineView, duration_ns: u64, width: f32) -> Vec<RulerTick> {
    let (start_ms, visible_ms) = view.window_ms(duration_ns);
    if visible_ms <= 0.0 || width <= 0.0 {
        return Vec::new();
    }

    let step = RULER_STEPS_MS
        .iter()
        .copied()
        .find(|step| step / visible_ms * f64::from(width) >= f64::from(MIN_TICK_SPACING))
        .unwrap_or(RULER_STEPS_MS[RULER_STEPS_MS.len() - 1]);
    let minor = step / 5.0;

    // Walk the minor ticks from the first one at or after the window's left
    // edge. At most `5 * width / MIN_TICK_SPACING` of them fit, so the count is
    // bounded.
    let first = (start_ms / minor).ceil() as i64;
    let count = (visible_ms / minor).ceil() as usize + 2;
    let mut ticks = Vec::with_capacity(count);
    for offset in 0..count {
        let index = first + offset as i64;
        let at = index as f64 * minor;
        if at > start_ms + visible_ms + 1e-9 {
            break;
        }
        ticks.push(RulerTick {
            x: ((at - start_ms) / visible_ms * f64::from(width)) as f32,
            // Every fifth minor is a labelled major tick.
            label: (index.rem_euclid(5) == 0).then(|| tick_label(at, step)),
        });
    }
    ticks
}

/// A ruler tick's label, formatted like Unity's `FormatTickLabel`: the decimals
/// follow the tick period (so `0.005ms` and `5ms` both read), and periods of a
/// second or more switch to seconds.
fn tick_label(ms: f64, step_ms: f64) -> String {
    let period_log = step_ms.log10().floor() as i32;
    if period_log >= 3 {
        format!("{:.0}s", ms / 1000.0)
    } else {
        let decimals = (-period_log).max(0) as usize;
        format!("{ms:.decimals$}ms")
    }
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
            let Some(data) = sample.data.as_deref() else {
                continue;
            };
            // The executor tags the data with the system's declared category:
            // core-module systems fold into their module and are not listed.
            if Category::from_tagged(data) != Some(Category::Scripts) {
                continue;
            }
            let system = strip_tag(data);
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

/// What painting one thread row produced for the timeline's selection.
#[derive(Clone, Copy, Debug, Default)]
struct RowOutcome {
    /// The sample the pointer clicked, with its block's rect.
    clicked: Option<(usize, Rect)>,
    /// The selected sample's block, when the selection is on this row.
    selected: Option<Rect>,
}

/// Everything Unity's details box reports about the selected block.
///
/// Kept apart from painting so the aggregation — and the text Copy writes — is
/// testable without an egui context.
#[derive(Clone, Debug, PartialEq)]
struct SelectionDetails {
    /// Marker name without its category tag (`spin_system`).
    title: String,
    /// Duration of the clicked block.
    duration_ns: u64,
    /// Thread the block ran on, for the "on thread '…'" line.
    thread: String,
    /// The marker on that thread: invocations and accumulated time.
    thread_instances: usize,
    thread_ns: u64,
    /// The marker across the frame: invocations, accumulated time, threads.
    frame_instances: usize,
    frame_ns: u64,
    frame_threads: usize,
}

impl SelectionDetails {
    /// Unity's "Current frame accumulated time:" block, with Unity's conditions.
    ///
    /// `ProfilerTimelineGUI.DoSelectionTooltip` writes the section only when the
    /// marker has more than one instance in the frame (on this thread or
    /// anywhere), the thread line only when it ran more than once on the clicked
    /// block's thread, and the frame line only when it also ran on other threads.
    /// A marker with one instance on one thread therefore gets no section at all:
    /// the accumulation would just repeat the block's own time.
    fn accumulated_lines(&self) -> Vec<String> {
        if self.thread_instances <= 1 && self.frame_instances <= 1 {
            return Vec::new();
        }
        let mut lines = vec!["Current frame accumulated time:".to_owned()];
        if self.thread_instances > 1 {
            lines.push(format!(
                "{} for {} {} on thread '{}'",
                duration_text(self.thread_ns),
                self.thread_instances,
                plural(self.thread_instances, "instance", "instances"),
                self.thread,
            ));
        }
        if self.frame_instances > self.thread_instances {
            lines.push(format!(
                "{} for {} {} over {} {}",
                duration_text(self.frame_ns),
                self.frame_instances,
                plural(self.frame_instances, "instance", "instances"),
                self.frame_threads,
                plural(self.frame_threads, "thread", "threads"),
            ));
        }
        lines
    }

    /// What the box's Copy button writes: the box's text, one line per line.
    ///
    /// Unity's Copy writes the tooltip content plus the call stack; the capture
    /// records no call stacks, so this is the tooltip content only.
    fn copy_text(&self) -> String {
        let mut lines = vec![self.title.clone(), duration_text(self.duration_ns)];
        lines.extend(self.accumulated_lines());
        lines.join("\n")
    }
}

/// The details box's contents for `selection`, or `None` when the selection no
/// longer addresses a sample.
///
/// Unity accumulates per marker in the selected frame: the same call site with
/// the same per-call data, which splits the executor's shared `Kairos.System`
/// marker per system.
fn selection_details(
    frame: &FrameSnapshot,
    scopes: &[ScopeInfo],
    lanes: &[Lane],
    selection: Selection,
) -> Option<SelectionDetails> {
    let lane = lanes.get(selection.lane)?;
    let thread = frame
        .threads
        .iter()
        .find(|thread| Some(thread.id) == lane.id)?;
    let sample = thread.samples.get(selection.sample)?;
    let scope = scope_info(scopes, sample.scope);
    let marker = (sample.scope, sample.data.as_deref());

    let mut thread_instances = 0;
    let mut thread_ns = 0u64;
    for other in &thread.samples {
        if (other.scope, other.data.as_deref()) != marker {
            continue;
        }
        thread_instances += 1;
        thread_ns = thread_ns.saturating_add(other.duration_ns);
    }

    let mut frame_instances = 0;
    let mut frame_ns = 0u64;
    let mut frame_threads = 0;
    for other_thread in &frame.threads {
        let mut ran_here = 0;
        for other in &other_thread.samples {
            if (other.scope, other.data.as_deref()) != marker {
                continue;
            }
            ran_here += 1;
            frame_ns = frame_ns.saturating_add(other.duration_ns);
        }
        if ran_here > 0 {
            frame_instances += ran_here;
            frame_threads += 1;
        }
    }

    Some(SelectionDetails {
        title: sample_detail(sample, scope),
        duration_ns: sample.duration_ns,
        thread: thread_display_name(thread),
        thread_instances,
        thread_ns,
        frame_instances,
        frame_ns,
        frame_threads,
    })
}

/// A timeline block's duration, the way Unity prints it on the block: two
/// decimals (`0.79ms`, `1.08ms`).
fn block_duration_text(ns: u64) -> String {
    format!("{:.2}ms", ns as f64 / 1_000_000.0)
}

/// A duration the way Unity's timeline tooltip prints it: two decimals at or
/// above 1 ms, three below.
///
/// `ProfilerTimelineGUI.DoSelectionTooltip` formats the selected sample's
/// duration and both accumulated times with `{0:f2}ms` / `{0:f3}ms`, so a
/// 0.261 ms block reads `0.261ms` and a 1.03 ms one reads `1.03ms`.
fn duration_text(ns: u64) -> String {
    let ms = ns as f64 / 1_000_000.0;
    if ms >= 1.0 {
        format!("{ms:.2}ms")
    } else {
        format!("{ms:.3}ms")
    }
}

/// The thread name Unity quotes in the details box, without the session id.
fn thread_display_name(thread: &ThreadSnapshot) -> String {
    if thread.name.is_empty() {
        format!("Thread {}", thread.id)
    } else {
        thread.name.clone()
    }
}

/// `1 instance` / `9 instances`.
fn plural<'a>(count: usize, one: &'a str, many: &'a str) -> &'a str {
    if count == 1 { one } else { many }
}

#[cfg(test)]
mod probe {
    use super::*;
    use crate::frame::HostView;
    use egui::pos2;
    use kairos_editor_core::EditorState;
    use kairos_profile::ScopeKind;

    struct NoHost;
    impl HostView for NoHost {}

    struct ProfilerHost(Profiler);
    impl HostView for ProfilerHost {
        fn profiler(&self) -> Option<&Profiler> {
            Some(&self.0)
        }
    }

    /// Every stroked rect the pass painted, as `(rect, stroke)`.
    fn painted_strokes(output: &egui::FullOutput) -> Vec<(Rect, Stroke)> {
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.stroke != Stroke::NONE => {
                    Some((shape.rect, shape.stroke))
                }
                _ => None,
            })
            .collect()
    }

    /// One headless pass at 900 × 640, on a caller-owned context so a test can
    /// click what the previous pass laid out.
    fn draw_pass(
        ctx: &Context,
        panel: &mut ProfilerPanel,
        host: &ProfilerHost,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        draw_pass_sized(ctx, panel, host, egui::vec2(900.0, 640.0), events)
    }

    /// One headless pass at a chosen window size, for the layout tests.
    fn draw_pass_sized(
        ctx: &Context,
        panel: &mut ProfilerPanel,
        host: &ProfilerHost,
        size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let state = EditorState::new();
        let mut out = UiOut::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), size));
        input.events = events;
        ctx.run_ui(input, |ui| {
            panel.ui(ui, &Frame::with_host(&state, host), &mut out);
        })
    }

    /// Centre of the splitter strip between the module area and the timeline.
    fn splitter_center(panel: &ProfilerPanel) -> egui::Pos2 {
        let top = panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        egui::pos2(400.0, top + panel.splitter().thickness() * 0.5)
    }

    /// One primary-button press at `pos`, as a pass's input.
    fn press_events(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// A pointer move with the primary button still down.
    fn drag_events(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![egui::Event::PointerMoved(pos)]
    }

    /// The primary button released at `pos`.
    fn release_events(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// A press or release of `button` at `pos`.
    fn button_events(
        pos: egui::Pos2,
        button: egui::PointerButton,
        pressed: bool,
    ) -> Vec<egui::Event> {
        let mut events = Vec::new();
        if pressed {
            events.push(egui::Event::PointerMoved(pos));
        }
        events.push(egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        events
    }

    /// A wheel event over `pos`. A four-point `Point` delta is below egui's
    /// smoothing threshold, so it applies in the pass that carries it.
    fn wheel_events(pos: egui::Pos2, delta: egui::Vec2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta,
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// A key press over `pos`.
    fn key_events(pos: egui::Pos2, key: egui::Key) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }

    /// One primary-button click at `pos`, as a pass's input.
    fn click_events(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ]
    }

    /// Top-left of the first painted text whose content starts with `prefix`.
    fn text_pos_starting_with(output: &egui::FullOutput, prefix: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text().starts_with(prefix) => Some(text.pos),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no text starting with {prefix:?} painted"))
    }

    /// Centre of the first painted text equal to `text`.
    fn text_center(output: &egui::FullOutput, text: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(shape) if shape.galley.text() == text => {
                    Some(shape.pos + shape.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no text {text:?} painted"))
    }

    /// Click a timeline block and return the pass that processed the click.
    fn click_block(
        ctx: &Context,
        panel: &mut ProfilerPanel,
        host: &ProfilerHost,
        prefix: &str,
    ) -> egui::FullOutput {
        let first = draw_pass(ctx, panel, host, Vec::new());
        let label = text_pos_starting_with(&first, prefix);
        draw_pass(ctx, panel, host, click_events(label + egui::vec2(6.0, 5.0)))
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
            kairos_profile::scope!(Category::Others, "profiler.test.frame");
            {
                kairos_profile::scope!(Category::Others, "profiler.test.frame");
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

    /// Capture one frame in which `profiler.test.repeat` runs three times on
    /// this thread, so the details box has accumulated time to report.
    fn repeated_marker_profiler() -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        for _ in 0..3 {
            {
                kairos_profile::scope!(Category::Others, "profiler.test.repeat");
                std::thread::sleep(std::time::Duration::from_millis(1));
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

    /// Every registered lane gets a row, including a worker that never ran:
    /// that is what keeps the timeline's tracks from shifting between frames.
    #[test]
    fn lanes_reserve_a_row_for_a_worker_that_never_ran() {
        let session = ProfileSession {
            thread_lanes: vec![
                "main".to_owned(),
                "Compute (0)".to_owned(),
                "Compute (1)".to_owned(),
            ],
            frames: vec![Arc::new(FrameSnapshot {
                index: 0,
                source_index: 0,
                start_ns: 0,
                end_ns: 16_000_000,
                threads: vec![
                    ThreadSnapshot {
                        id: 0,
                        name: "main".to_owned(),
                        samples: Vec::new(),
                        warning: None,
                    },
                    ThreadSnapshot {
                        id: 3,
                        name: "Compute (0)".to_owned(),
                        samples: Vec::new(),
                        warning: None,
                    },
                ],
                categories: [0; Category::COUNT],
            })],
            ..Default::default()
        };

        let lanes = collect_lanes(&session);
        assert_eq!(lanes.len(), 3);
        assert_eq!(lanes[0].name, "main");
        assert_eq!(lanes[0].id, Some(0));
        assert_eq!(lanes[1].name, "Compute (0)");
        assert_eq!(
            lanes[1].id,
            Some(3),
            "the lane picks its thread's session id once it reports"
        );
        assert_eq!(lanes[2].name, "Compute (1)");
        assert_eq!(
            lanes[2].id, None,
            "a worker that never ran keeps an empty row"
        );
    }

    /// A thread that was never registered still shows up, after the lanes.
    #[test]
    fn unregistered_threads_follow_the_registered_lanes() {
        let session = ProfileSession {
            thread_lanes: vec!["main".to_owned()],
            frames: vec![Arc::new(FrameSnapshot {
                index: 0,
                source_index: 0,
                start_ns: 0,
                end_ns: 16_000_000,
                threads: vec![
                    ThreadSnapshot {
                        id: 0,
                        name: "main".to_owned(),
                        samples: Vec::new(),
                        warning: None,
                    },
                    ThreadSnapshot {
                        id: 1,
                        name: "audio".to_owned(),
                        samples: Vec::new(),
                        warning: None,
                    },
                ],
                categories: [0; Category::COUNT],
            })],
            ..Default::default()
        };

        let lanes = collect_lanes(&session);
        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes[1].name, "audio");
    }

    /// Unity's legend swatch hides its band from the chart.
    #[test]
    fn hiding_a_category_drops_it_from_the_frame_bar() {
        let frame = FrameSnapshot {
            index: 0,
            source_index: 0,
            start_ns: 0,
            end_ns: 10_000_000,
            threads: Vec::new(),
            categories: {
                let mut categories = [0u64; Category::COUNT];
                categories[Category::Scripts as usize] = 3_000_000;
                categories[Category::Physics as usize] = 2_000_000;
                categories
            },
        };

        let mut panel = ProfilerPanel::default();
        let full = panel.bar_ms(&frame);
        assert!((full - 10.0).abs() < 0.01, "the bar is the frame time");

        panel.hidden[Category::Physics as usize] = true;
        let hidden = panel.bar_ms(&frame);
        assert!(
            (full - hidden - 2.0).abs() < 0.01,
            "Only the physics band leaves: {full} -> {hidden}"
        );
        assert!(
            (panel.category_ms(&frame, Category::Physics) - 2.0).abs() < 0.01,
            "the legend keeps the hidden category's time"
        );
    }

    /// The timeline draws a row for a registered lane that never reported.
    #[test]
    fn the_timeline_lists_idle_registered_lanes() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "work");
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        profiler.register_thread_lane("Compute (7)");

        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let text = painted_text(&output);
        assert!(
            text.iter().any(|line| line == "Compute (7)"),
            "an idle lane keeps its row: {text:?}"
        );
    }

    /// Clicking a legend swatch takes the category out of the chart, like
    /// Unity's legend toggles.
    #[test]
    fn clicking_a_legend_row_hides_the_category() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let state = EditorState::new();
        let mut out = UiOut::new();
        let ctx = Context::default();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            egui::vec2(900.0, 640.0),
        ));

        // The first pass lays the panel out; the click in the second pass hits
        // the row rect the first pass registered.
        let first = ctx.run_ui(input.clone(), |ui| {
            panel.ui(ui, &Frame::with_host(&state, &host), &mut out);
        });
        let label = first
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == "Physics" => Some(text.pos),
                _ => None,
            })
            .expect("the legend paints a Physics row");
        let clicked_at = label + egui::vec2(4.0, 4.0);

        input.events = vec![
            egui::Event::PointerMoved(clicked_at),
            egui::Event::PointerButton {
                pos: clicked_at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos: clicked_at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ];
        ctx.run_ui(input, |ui| {
            panel.ui(ui, &Frame::with_host(&state, &host), &mut out);
        });

        assert!(
            panel.hidden[Category::Physics as usize],
            "a click on the legend row toggles its category"
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
            kairos_profile::scope!(Category::Others, "Kairos.Stage", "Update");
            {
                kairos_profile::scope!(
                    Category::Others,
                    "Kairos.System",
                    "Physics/kairos_physics::physics_step_system"
                );
                // Give the sample a measurable duration: puffin reads the clock
                // in nanoseconds, and back-to-back scopes can land on the same
                // tick, which would leave the Physics band at zero height.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            {
                kairos_profile::scope!(
                    Category::Others,
                    "Kairos.System",
                    "Scripts/kairos_game::spin_system"
                );
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

    /// The module area: CPU/GPU rows, the legend of the selected frame, the
    /// 30/60 FPS reference chips, the 120 FPS baseline and the selected-frame
    /// cursor.
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
                && text.iter().any(|line| line.contains("60FPS"))
                && text.iter().any(|line| line.contains("120FPS")),
            "Unity's 30/60 FPS reference chips and the 120 FPS baseline are drawn: {text:?}"
        );

        let cursor = panel.style.chart_cursor.to_egui();
        assert!(
            painted(&output)
                .iter()
                .any(|(rect, fill)| *fill == cursor && rect.height() > 10.0),
            "the selected frame has a cursor in the chart"
        );
    }

    /// The splitter between the module area and the timeline: dragging it down
    /// grows the module area (Unity's fixed pane) and pushes the timeline down;
    /// dragging it up gives the timeline the room back.
    #[test]
    fn dragging_the_splitter_resizes_the_module_area_and_the_timeline() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let header = text_pos_starting_with(&first, "Timeline");
        let module_before = panel.module_height;
        let from = splitter_center(&panel);

        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            drag_events(from + egui::vec2(0.0, 40.0)),
        );
        assert!(
            (panel.module_height - (module_before + 40.0)).abs() < 0.5,
            "the drag grew the module area: {module_before} -> {}",
            panel.module_height
        );
        let after = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let moved = text_pos_starting_with(&after, "Timeline");
        assert!(
            (moved.y - header.y - 40.0).abs() < 0.5,
            "the timeline moved down with the boundary: {} -> {}",
            header.y,
            moved.y
        );
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            release_events(from + egui::vec2(0.0, 40.0)),
        );
    }

    /// Both bands keep a minimum, and the module pane stops at its card stack:
    /// the splitter cannot grow it past the last module, so there is no dead
    /// space under the cards.
    #[test]
    fn the_splitter_stops_at_both_bands_minimum_sizes() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let from = splitter_center(&panel);
        let min_module = panel.min_module_height();
        let total = 640.0 - panel.style.toolbar_height - panel.style.separator_width;
        let max_module = (total - panel.splitter().thickness() - panel.min_timeline_height())
            .min(panel.module_content_height());

        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            drag_events(from + egui::vec2(0.0, -400.0)),
        );
        assert!(
            (panel.module_height - min_module).abs() < 0.5,
            "the module area stops at its minimum: {}",
            panel.module_height
        );
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            drag_events(from + egui::vec2(0.0, 1000.0)),
        );
        assert!(
            (panel.module_height - max_module).abs() < 0.5,
            "the pane stops where the card stack ends: {}",
            panel.module_height
        );
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            release_events(from + egui::vec2(0.0, 1000.0)),
        );
    }

    /// Unity's module pane is a list of fixed-height cards: dragging the
    /// splitter down reveals the GPU Usage card instead of stretching the CPU
    /// Usage chart, and the cards keep their places.
    #[test]
    fn dragging_the_splitter_reveals_the_module_below() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let cursor = panel.style.chart_cursor.to_egui();
        let chart_height = |output: &egui::FullOutput| {
            painted(output)
                .into_iter()
                .find(|(rect, fill)| *fill == cursor && rect.height() > 10.0)
                .map(|(rect, _)| rect.height())
                .expect("the CPU chart paints its selected-frame cursor")
        };
        let chart_before = chart_height(&first);
        let cpu = text_pos_starting_with(&first, "CPU Usage");
        let gpu = text_pos_starting_with(&first, "GPU Usage");
        let from = splitter_center(&panel);

        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            drag_events(from + egui::vec2(0.0, 100.0)),
        );
        let after = draw_pass(&ctx, &mut panel, &host, Vec::new());

        assert!(
            (panel.module_height - panel.module_content_height()).abs() < 0.5,
            "the drag revealed the whole card stack: {}",
            panel.module_height
        );
        assert_eq!(
            text_pos_starting_with(&after, "CPU Usage"),
            cpu,
            "the CPU card keeps its place"
        );
        assert_eq!(
            text_pos_starting_with(&after, "GPU Usage"),
            gpu,
            "the GPU card keeps its place"
        );
        assert!(
            (chart_height(&after) - chart_before).abs() < 0.5,
            "the CPU chart keeps its fixed height: {chart_before} -> {}",
            chart_height(&after)
        );
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            release_events(from + egui::vec2(0.0, 100.0)),
        );
    }

    /// The module pane scrolls like Unity's modules list when the cards are
    /// taller than the pane.
    #[test]
    fn the_module_pane_scrolls_with_the_wheel() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let cpu = text_pos_starting_with(&first, "CPU Usage");
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(egui::pos2(400.0, 60.0), egui::vec2(0.0, -4.0)),
        );
        assert!(
            panel.module_scroll > 0.0,
            "the wheel scrolls the card stack: {}",
            panel.module_scroll
        );
        let scrolled = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let cpu_after = text_pos_starting_with(&scrolled, "CPU Usage");
        assert!(
            cpu_after.y < cpu.y - 2.0,
            "the cards scrolled up: {} -> {}",
            cpu.y,
            cpu_after.y
        );
    }

    /// The module pane has its own vertical bar; dragging its thumb scrolls
    /// the cards like the timeline's bar scrolls the tracks.
    #[test]
    fn dragging_the_module_scroll_bar_scrolls_the_cards() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let splitter_top =
            panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        let thumb = painted(&first)
            .into_iter()
            .filter(|(rect, fill)| {
                *fill == panel.style.scrollbar_thumb.to_egui()
                    && rect.width() < 20.0
                    && rect.height() > 20.0
                    && rect.top() < splitter_top
            })
            .map(|(rect, _)| rect)
            .max_by(|a, b| a.height().total_cmp(&b.height()))
            .expect("the module pane's scroll bar thumb is painted");
        let from = thumb.center();
        let to = from + egui::vec2(0.0, 20.0);

        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(&ctx, &mut panel, &host, drag_events(to));
        let _ = draw_pass(&ctx, &mut panel, &host, release_events(to));

        assert!(
            panel.module_scroll > 0.0,
            "dragging the thumb down scrolls the cards: {}",
            panel.module_scroll
        );
    }

    /// The module cards are windowed: a card taller than the pane must not
    /// paint over the splitter or the transparent timeline header below it.
    #[test]
    fn the_module_cards_are_clipped_to_the_pane() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        // A pane shorter than the CPU card, so the cards overflow it.
        panel.module_height = 120.0;
        let ctx = Context::default();
        let output = draw_pass(&ctx, &mut panel, &host, Vec::new());

        assert!(
            panel.module_content_height() > panel.module_height,
            "the cards overflow the pane, so clipping is what keeps them in"
        );
        let pane_bottom =
            panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        let clipped = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == "CPU Usage" => {
                    Some(clipped.clip_rect)
                }
                _ => None,
            })
            .expect("the CPU card paints its title");
        assert!(
            (clipped.bottom() - pane_bottom).abs() < 0.5,
            "the module painting is clipped to its pane: {clipped:?} vs {pane_bottom}"
        );
    }
    /// The splitter is chrome of its own, brighter than the section rules, so
    /// it reads as a handle instead of another 1 px separator.
    #[test]
    fn the_splitter_line_is_brighter_than_the_section_rules() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let idle = panel.style.splitter_idle.to_egui();
        assert_ne!(
            idle,
            panel.style.separator.to_egui(),
            "the splitter must not reuse the section rule colour"
        );
        let line = painted(&first)
            .into_iter()
            .find(|(rect, fill)| *fill == idle && rect.height() <= 1.0 && rect.width() > 300.0)
            .map(|(rect, _)| rect)
            .expect("the splitter paints its idle line across the pane");
        let expected = panel.style.toolbar_height
            + panel.style.separator_width
            + panel.module_height
            + panel.splitter().thickness() * 0.5;
        assert!(
            (line.center().y - expected).abs() < 1.0,
            "the idle line sits on the pane boundary: {} vs {expected}",
            line.center().y
        );
    }
    /// Unity's split view has no reset, but this editor's dock separators reset
    /// on a double-click; the profiler splitter follows the dock.
    #[test]
    fn double_clicking_the_splitter_restores_the_skin_height() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.module_height = 260.0;
        let ctx = Context::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let at = splitter_center(&panel);

        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        assert_eq!(
            panel.module_height, 260.0,
            "one click leaves the boundary alone"
        );
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        assert!(
            (panel.module_height - panel.style.module_area_height).abs() < 0.5,
            "the double-click restores the skin's height: {}",
            panel.module_height
        );
    }

    /// The splitter tint follows the pointer, like the dock separators.
    #[test]
    fn the_splitter_highlights_under_the_pointer() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let at = splitter_center(&panel);
        let output = draw_pass(&ctx, &mut panel, &host, vec![egui::Event::PointerMoved(at)]);

        let hover = panel.style.splitter_hover.to_egui();
        assert!(
            painted(&output)
                .iter()
                .any(|(rect, fill)| *fill == hover && (rect.center().y - at.y).abs() < 1.0),
            "the splitter highlights under the pointer"
        );
    }

    /// A short panel clamps the split: the module area shrinks to its minimum
    /// before the timeline is squeezed, and a panel shorter than both minimums
    /// still lays both bands out (the timeline gives way first, like Unity's
    /// flexed pane).
    #[test]
    fn a_short_panel_keeps_both_bands_usable() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let ctx = Context::default();
        let size = egui::vec2(900.0, 260.0);
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass_sized(&ctx, &mut panel, &host, size, Vec::new());
        let total = size.y - panel.style.toolbar_height - panel.style.separator_width;
        let expected = total - panel.splitter().thickness() - panel.min_timeline_height();
        assert!(
            (panel.module_height - expected).abs() < 0.5,
            "the module area gives room to the timeline minimum: {}",
            panel.module_height
        );
        let output = draw_pass_sized(&ctx, &mut panel, &host, size, Vec::new());
        assert!(painted_text(&output).iter().any(|line| line == "Timeline"));

        let mut tiny = ProfilerPanel::default();
        let _ = draw_pass_sized(&ctx, &mut tiny, &host, egui::vec2(900.0, 120.0), Vec::new());
        assert!(
            (tiny.module_height - tiny.min_module_height()).abs() < 0.5,
            "the module area keeps its minimum: {}",
            tiny.module_height
        );
        assert!(
            tiny.min_module_height() < 190.0,
            "a minimum can be shared by both bands"
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
        let root = text
            .iter()
            .find(|line| line.starts_with("Others · profiler.test.frame ("))
            .unwrap_or_else(|| {
                panic!("the root sample is labelled with its category and duration: {text:?}")
            });
        assert!(
            root.ends_with("ms)"),
            "the block's duration follows its name: {root:?}"
        );

        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let track_left = panel.style.timeline_name_width;
        assert!(
            painted(&output).iter().any(|(rect, fill)| {
                rect.left() >= track_left && rect.height() >= 10.0 && palette.contains(fill)
            }),
            "sample bars use the category palette inside the track column"
        );
    }

    /// The timeline labels each block with its module first, so a third-party
    /// marker reads as its category (`Surface::get_current_texture` → VSync),
    /// and paints the block in that category's colour. The category is per
    /// block, so parallel systems on different lanes still read as one module.
    #[test]
    fn timeline_blocks_are_named_and_coloured_by_category() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Vsync, "Surface::get_current_texture");
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        {
            kairos_profile::scope!(
                Category::Others,
                "Kairos.System",
                "Physics/kairos_physics::physics_step_system"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        profiler.end_frame(true);
        profiler.begin_frame();

        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let text = painted_text(&output);

        assert!(
            text.iter()
                .any(|line| line.starts_with("VSync · Surface::get_current_texture (")),
            "a third-party marker is tagged with its category: {text:?}"
        );
        assert!(
            text.iter()
                .any(|line| line.starts_with("Physics · physics_step_system (")),
            "a system block is named by its system, not `Kairos.System`: {text:?}"
        );

        let vsync = panel.style.category_palette[Category::Vsync as usize].to_egui();
        assert!(
            painted(&output).iter().any(|(rect, fill)| {
                *fill == vsync
                    && rect.left() >= panel.style.timeline_name_width
                    && rect.height() >= 10.0
            }),
            "the VSync block uses the VSync category colour"
        );
    }

    /// Durations read like Unity's: a block label takes two decimals, the
    /// details box `{0:f2}` at or above 1 ms and `{0:f3}` below.
    #[test]
    fn durations_read_like_unitys() {
        assert_eq!(block_duration_text(790_000), "0.79ms");
        assert_eq!(block_duration_text(1_080_000), "1.08ms");
        assert_eq!(duration_text(261_000), "0.261ms");
        assert_eq!(duration_text(1_030_000), "1.03ms");
        assert_eq!(duration_text(4_000_000), "4.00ms");
        assert_eq!(duration_text(0), "0.000ms");
    }

    /// The details box accumulates the marker over the frame, not just the
    /// clicked sample: Unity's "instances on thread" and "over threads".
    #[test]
    fn the_details_box_accumulates_a_marker_over_the_frame() {
        let scope = ScopeInfo {
            id: 7,
            name: "profiler.test.worker".to_owned(),
            location: String::new(),
            kind: ScopeKind::Named,
            category: Category::Others,
        };
        let sample = |duration_ns: u64| SampleSnapshot {
            scope: 7,
            data: None,
            start_ns: 0,
            duration_ns,
            depth: 0,
            parent: None,
        };
        let frame = FrameSnapshot {
            index: 0,
            source_index: 0,
            start_ns: 0,
            end_ns: 16_000_000,
            threads: vec![
                ThreadSnapshot {
                    id: 0,
                    name: "Main Thread".to_owned(),
                    samples: vec![sample(2_000_000), sample(1_000_000)],
                    warning: None,
                },
                ThreadSnapshot {
                    id: 1,
                    name: "Compute (0)".to_owned(),
                    samples: vec![sample(4_000_000)],
                    warning: None,
                },
            ],
            categories: [0; Category::COUNT],
        };
        let lanes = vec![
            Lane {
                id: Some(0),
                name: "Main Thread".to_owned(),
            },
            Lane {
                id: Some(1),
                name: "Compute (0)".to_owned(),
            },
        ];
        let selection = Selection {
            frame: 0,
            lane: 1,
            sample: 0,
        };
        let details = selection_details(&frame, &[scope], &lanes, selection)
            .expect("the selection addresses a sample");

        assert_eq!(details.title, "profiler.test.worker");
        assert_eq!(details.duration_ns, 4_000_000);
        assert_eq!(details.thread, "Compute (0)");
        assert_eq!(details.thread_instances, 1);
        assert_eq!(details.thread_ns, 4_000_000);
        assert_eq!(details.frame_instances, 3);
        assert_eq!(details.frame_ns, 7_000_000);
        assert_eq!(details.frame_threads, 2);
        // Unity omits the thread line when the marker ran once on that thread.
        assert_eq!(
            details.copy_text(),
            "profiler.test.worker\n4.00ms\nCurrent frame accumulated time:\n\
             7.00ms for 3 instances over 2 threads"
        );
    }

    /// A selection addresses one frame's samples: stepping the transport drops
    /// it instead of describing a block that is no longer there.
    #[test]
    fn stepping_to_another_frame_drops_the_selection() {
        let _guard = lock_profiler_tests();
        let profiler = Profiler::new();
        profiler.set_recording(true);
        for _ in 0..2 {
            profiler.begin_frame();
            {
                kairos_profile::scope!(Category::Others, "profiler.test.frame");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            profiler.end_frame(true);
        }
        profiler.begin_frame();

        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();
        click_block(&ctx, &mut panel, &host, "Others · profiler.test.frame (");
        assert_eq!(panel.selected_frame, 1, "the newest frame is selected");
        assert!(panel.selection.is_some());

        // Step back one frame: the block indices no longer address this frame.
        panel.selected_frame = 0;
        draw_pass(&ctx, &mut panel, &host, Vec::new());
        assert!(
            panel.selection.is_none(),
            "a stale selection is dropped when the frame changes"
        );
    }

    /// Clicking a block outlines it and opens Unity's details box; selecting
    /// also pins the frame, so the box cannot be swapped out by a new one.
    #[test]
    fn clicking_a_block_selects_it_and_opens_the_details_box() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let selected = click_block(&ctx, &mut panel, &host, "Others · profiler.test.frame (");
        assert!(panel.selection.is_some(), "a click on a block selects it");
        assert!(
            !panel.follow_latest,
            "selecting pins the displayed frame so the box cannot vanish"
        );

        let text = painted_text(&selected);
        assert!(
            text.iter().any(|line| line == "profiler.test.frame"),
            "the box is titled with the marker: {text:?}"
        );
        for expected in ["Copy", "Show"] {
            assert!(
                text.iter().any(|line| line == expected),
                "the details box paints {expected:?}: {text:?}"
            );
        }
        assert!(
            !text
                .iter()
                .any(|line| line == "Current frame accumulated time:"),
            "a single instance on one thread gets no accumulated section, like Unity: {text:?}"
        );

        let outline = panel.style.selection_outline.to_egui();
        assert!(
            painted_strokes(&selected)
                .iter()
                .any(|(rect, stroke)| stroke.color == outline
                    && rect.left() >= panel.style.timeline_name_width
                    && rect.height() >= 10.0),
            "the selected block keeps a 1 px outline"
        );
        let box_fill = panel.style.selection_bg.to_egui();
        assert!(
            painted(&selected)
                .iter()
                .any(|(rect, fill)| *fill == box_fill
                    && rect.width() > 100.0
                    && rect.height() > 40.0),
            "the details box paints its panel fill"
        );
    }

    /// Unity only opens the accumulated-time section when the marker ran more
    /// than once; a marker that never left this thread gets no frame line.
    #[test]
    fn repeated_markers_get_unitys_accumulated_time_section() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let selected = click_block(&ctx, &mut panel, &host, "Others · profiler.test.repeat (");
        let text = painted_text(&selected);
        assert!(
            text.iter()
                .any(|line| line == "Current frame accumulated time:"),
            "three instances open the accumulated section: {text:?}"
        );
        assert!(
            text.iter()
                .any(|line| line.contains("for 3 instances on thread")),
            "the thread line reports all three instances: {text:?}"
        );
        assert!(
            !text.iter().any(|line| line.contains("over 1 thread")),
            "a marker that never left this thread gets no frame line: {text:?}"
        );
    }

    /// The accumulated section follows Unity's rules: omitted for one instance
    /// on one thread, thread line only when the marker repeats on the thread,
    /// frame line only when it also ran elsewhere.
    #[test]
    fn the_accumulated_time_section_follows_unitys_rules() {
        let details = |thread_instances: usize,
                       thread_ns: u64,
                       frame_instances: usize,
                       frame_ns: u64,
                       frame_threads: usize| SelectionDetails {
            title: "profiler.test.worker".to_owned(),
            duration_ns: 1_000_000,
            thread: "Main Thread".to_owned(),
            thread_instances,
            thread_ns,
            frame_instances,
            frame_ns,
            frame_threads,
        };

        assert!(
            details(1, 1_000_000, 1, 1_000_000, 1)
                .accumulated_lines()
                .is_empty(),
            "one instance on one thread adds nothing to the block's own time"
        );
        assert_eq!(
            details(2, 2_000_000, 2, 2_000_000, 1).accumulated_lines(),
            vec![
                "Current frame accumulated time:".to_owned(),
                "2.00ms for 2 instances on thread 'Main Thread'".to_owned(),
            ],
            "repeats on one thread: the thread line only"
        );
        assert_eq!(
            details(1, 1_000_000, 3, 7_000_000, 2).accumulated_lines(),
            vec![
                "Current frame accumulated time:".to_owned(),
                "7.00ms for 3 instances over 2 threads".to_owned(),
            ],
            "one instance here, three over two threads: the frame line only"
        );
        assert_eq!(
            details(2, 2_000_000, 5, 9_000_000, 3).accumulated_lines(),
            vec![
                "Current frame accumulated time:".to_owned(),
                "2.00ms for 2 instances on thread 'Main Thread'".to_owned(),
                "9.00ms for 5 instances over 3 threads".to_owned(),
            ],
            "repeats on this thread and elsewhere: both lines"
        );
    }

    /// A second click on the selected block dismisses the box.
    #[test]
    fn clicking_the_selected_block_again_dismisses_the_details_box() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let selected = click_block(&ctx, &mut panel, &host, "Others · profiler.test.frame (");
        assert!(panel.selection.is_some());
        let label = text_pos_starting_with(&selected, "Others · profiler.test.frame (");
        let dismissed = draw_pass(
            &ctx,
            &mut panel,
            &host,
            click_events(label + egui::vec2(6.0, 5.0)),
        );
        assert!(
            panel.selection.is_none(),
            "a second click on the same block deselects it"
        );
        assert!(
            !painted_text(&dismissed)
                .iter()
                .any(|line| line == "Current frame accumulated time:"),
            "the dismissed box is no longer painted"
        );
    }

    /// A click on a row without a block under the pointer dismisses the box.
    #[test]
    fn clicking_empty_track_dismisses_the_details_box() {
        let _guard = lock_profiler_tests();
        let profiler = captured_profiler();
        profiler.register_thread_lane("Compute (7)");
        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let selected = click_block(&ctx, &mut panel, &host, "Others · profiler.test.frame (");
        assert!(panel.selection.is_some());

        // The idle lane's row has no blocks at all.
        let idle = text_center(&selected, "Compute (7)");
        draw_pass(&ctx, &mut panel, &host, click_events(idle));
        assert!(
            panel.selection.is_none(),
            "a click on empty track dismisses the box"
        );
    }

    /// The time area's columns at the test harness's 900 pt panel width.
    fn track_columns(panel: &ProfilerPanel) -> (f32, f32) {
        (
            panel.style.timeline_name_width,
            900.0 - panel.style.scrollbar_width,
        )
    }

    /// The horizontal scroll bar's thumb: the widest fill in the thumb colour.
    fn horizontal_thumb(output: &egui::FullOutput, fill: egui::Color32) -> Rect {
        painted(output)
            .into_iter()
            .filter(|(rect, painted_fill)| *painted_fill == fill && rect.width() > 20.0)
            .max_by(|a, b| a.0.width().total_cmp(&b.0.width()))
            .map(|(rect, _)| rect)
            .expect("the horizontal scroll bar's thumb is painted")
    }

    /// The wheel over the time area zooms around the pointer: the time under the
    /// pointer stays under it, the way Unity's zoomable timeline behaves.
    #[test]
    fn the_wheel_zooms_the_time_area_around_the_pointer() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let label = text_pos_starting_with(&first, "Others · profiler.test.repeat (");
        let pointer = label + egui::vec2(10.0, 3.0);
        let before = panel.view;
        draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(pointer, egui::vec2(0.0, 4.0)),
        );
        let after = panel.view;

        assert!(
            after.span < before.span,
            "the wheel zooms in: {before:?} -> {after:?}"
        );
        let (left, right) = track_columns(&panel);
        let focus = f64::from((pointer.x - left) / (right - left));
        let under_pointer_before = before.pan + focus * before.span;
        let under_pointer_after = after.pan + focus * after.span;
        assert!(
            (under_pointer_before - under_pointer_after).abs() < 1e-6,
            "the time under the pointer stays put: {under_pointer_before} -> {under_pointer_after}"
        );
    }

    /// Middle-drag pans the time area, Unity's other pan gesture.
    #[test]
    fn middle_drag_pans_the_time_area() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.3,
            span: 0.4,
        };
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let label = text_pos_starting_with(&first, "Others · profiler.test.repeat (");
        let from = label + egui::vec2(10.0, 3.0);
        let to = from + egui::vec2(40.0, 0.0);

        draw_pass(
            &ctx,
            &mut panel,
            &host,
            button_events(from, egui::PointerButton::Middle, true),
        );
        draw_pass(&ctx, &mut panel, &host, drag_events(to));
        draw_pass(
            &ctx,
            &mut panel,
            &host,
            button_events(to, egui::PointerButton::Middle, false),
        );

        assert!(
            panel.view.pan < 0.3,
            "middle-drag right shows earlier time: {:?}",
            panel.view
        );
    }

    /// Dragging the horizontal bar's thumb pans the shown window, like Unity's
    /// min-max scroll bar.
    #[test]
    fn dragging_the_horizontal_bar_pans_the_timeline() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.25,
            span: 0.5,
        };
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let thumb = horizontal_thumb(&first, panel.style.scrollbar_thumb.to_egui());
        let before = panel.view;
        let from = thumb.center();
        let to = from + egui::vec2(24.0, 0.0);

        draw_pass(&ctx, &mut panel, &host, press_events(from));
        draw_pass(&ctx, &mut panel, &host, drag_events(to));
        draw_pass(&ctx, &mut panel, &host, release_events(to));

        assert!(
            panel.view.pan > before.pan,
            "dragging the thumb right shows later time: {before:?} -> {:?}",
            panel.view
        );
        assert_eq!(panel.view.span, before.span, "panning does not zoom");
    }

    /// Dragging an end of the horizontal bar's thumb resizes the window: that is
    /// how Unity's timeline zooms from its scroll bar.
    #[test]
    fn dragging_the_end_of_the_horizontal_bar_zooms_the_timeline() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.25,
            span: 0.5,
        };
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let thumb = horizontal_thumb(&first, panel.style.scrollbar_thumb.to_egui());
        let from = egui::pos2(thumb.right() - 3.0, thumb.center().y);
        let to = from - egui::vec2(80.0, 0.0);

        draw_pass(&ctx, &mut panel, &host, press_events(from));
        draw_pass(&ctx, &mut panel, &host, drag_events(to));
        draw_pass(&ctx, &mut panel, &host, release_events(to));

        assert!(
            panel.view.span < 0.5,
            "dragging the right end in zooms: {:?}",
            panel.view
        );
        assert!(
            (panel.view.pan - 0.25).abs() < 1e-9,
            "the left edge stays put: {:?}",
            panel.view
        );
    }

    /// Dragging the vertical bar scrolls the rows, like any scrollbar.
    #[test]
    fn dragging_the_vertical_bar_scrolls_the_rows() {
        let _guard = lock_profiler_tests();
        let profiler = captured_profiler();
        for index in 0..30 {
            profiler.register_thread_lane(format!("Lane {index:02}"));
        }
        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let thumb = painted(&first)
            .into_iter()
            .filter(|(rect, fill)| {
                *fill == panel.style.scrollbar_thumb.to_egui()
                    && rect.width() < 20.0
                    && rect.height() > 20.0
            })
            .max_by(|a, b| a.0.height().total_cmp(&b.0.height()))
            .map(|(rect, _)| rect)
            .expect("the vertical scroll bar's thumb is painted");
        let from = thumb.center();
        let to = from + egui::vec2(0.0, 30.0);

        draw_pass(&ctx, &mut panel, &host, press_events(from));
        draw_pass(&ctx, &mut panel, &host, drag_events(to));
        draw_pass(&ctx, &mut panel, &host, release_events(to));

        assert!(
            panel.track_scroll > 0.0,
            "dragging the thumb down scrolls the rows: {}",
            panel.track_scroll
        );
    }

    /// The header and the ruler are pinned: the wheel over the thread names
    /// scrolls only the rows, Unity's side-column gesture.
    #[test]
    fn the_header_and_ruler_stay_pinned_while_the_tracks_scroll() {
        let _guard = lock_profiler_tests();
        let profiler = captured_profiler();
        for index in 0..30 {
            profiler.register_thread_lane(format!("Lane {index:02}"));
        }
        let host = ProfilerHost(profiler);
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let header = text_pos_starting_with(&first, "Timeline");
        let ruler = text_pos_starting_with(&first, "0.000ms");
        let lane = text_pos_starting_with(&first, "Lane 00");
        let pointer = egui::pos2(50.0, lane.y + 4.0);
        draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(pointer, egui::vec2(0.0, -6.0)),
        );
        let scrolled = draw_pass(&ctx, &mut panel, &host, Vec::new());

        assert!(
            panel.track_scroll > 0.0,
            "the wheel over the names scrolls the rows"
        );
        assert_eq!(
            text_pos_starting_with(&scrolled, "Timeline"),
            header,
            "the header does not scroll"
        );
        assert_eq!(
            text_pos_starting_with(&scrolled, "0.000ms"),
            ruler,
            "the ruler does not scroll"
        );
        let lane_after = text_pos_starting_with(&scrolled, "Lane 00");
        assert!(
            lane_after.y < lane.y - 4.0,
            "the rows scrolled up: {} -> {}",
            lane.y,
            lane_after.y
        );
    }

    /// Unity's FrameAll: `A` over the timeline fits the whole frame again.
    #[test]
    fn pressing_a_fits_the_whole_frame_again() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.3,
            span: 0.4,
        };
        let ctx = Context::default();
        // One quiet pass so the timeline is laid out under the pointer.
        draw_pass(&ctx, &mut panel, &host, Vec::new());
        draw_pass(
            &ctx,
            &mut panel,
            &host,
            key_events(egui::pos2(400.0, 400.0), egui::Key::A),
        );

        assert_eq!(panel.view, TimelineView::default());
    }

    /// A zoomed window cuts blocks at the track's edges instead of spilling over
    /// the thread names or the scroll bar.
    #[test]
    fn zoomed_blocks_are_clipped_to_the_track_column() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.4,
            span: 0.1,
        };
        let ctx = Context::default();
        let output = draw_pass(&ctx, &mut panel, &host, Vec::new());

        let (left, right) = track_columns(&panel);
        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let blocks: Vec<Rect> = painted(&output)
            .into_iter()
            .filter(|(rect, fill)| {
                palette.contains(fill) && rect.height() >= 10.0 && rect.top() > 250.0
            })
            .map(|(rect, _)| rect)
            .collect();

        assert!(!blocks.is_empty(), "the zoomed window still shows blocks");
        for block in &blocks {
            assert!(
                block.left() >= left - 0.5 && block.right() <= right + 0.5,
                "a block is cut at the track column: {block:?}"
            );
        }
        assert!(
            blocks
                .iter()
                .any(|block| { block.left() <= left + 0.5 && block.right() >= right - 0.5 }),
            "a block crossing the window is cut at both edges: {blocks:?}"
        );
    }

    /// Clicking a block works on a zoomed window: the block under the pointer is
    /// selected and the details box opens.
    #[test]
    fn clicking_a_block_after_zooming_still_selects_it() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view = TimelineView {
            pan: 0.0,
            span: 0.5,
        };
        let ctx = Context::default();

        // Find the block by its category fill rather than by its label: a
        // narrow block drops its label, and this test is about hit-testing.
        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let (left, right) = track_columns(&panel);
        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let block = painted(&first)
            .into_iter()
            .filter(|(rect, fill)| {
                palette.contains(fill)
                    && rect.height() >= 10.0
                    && rect.top() > 250.0
                    && rect.left() >= left - 0.5
                    && rect.right() <= right + 0.5
            })
            .map(|(rect, _)| rect)
            .max_by(|a, b| a.width().total_cmp(&b.width()))
            .expect("a block is painted in the zoomed window");
        let clicked = draw_pass(&ctx, &mut panel, &host, click_events(block.center()));

        assert!(panel.selection.is_some(), "the clicked block is selected");
        assert!(
            painted_text(&clicked).iter().any(|line| line == "Copy"),
            "the details box opened: {:?}",
            painted_text(&clicked)
        );
    }

    /// Unity's Copy: the box's text lands on the clipboard.
    #[test]
    fn the_details_box_copy_button_writes_the_details_to_the_clipboard() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let selected = click_block(&ctx, &mut panel, &host, "Others · profiler.test.repeat (");
        let copy = text_center(&selected, "Copy");
        let copied = draw_pass(&ctx, &mut panel, &host, click_events(copy));

        let clipboard = copied
            .platform_output
            .commands
            .iter()
            .find_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .expect("Copy writes the details to the clipboard");
        assert!(
            clipboard.starts_with("profiler.test.repeat\n"),
            "the clipboard starts with the marker: {clipboard:?}"
        );
        assert!(
            clipboard.contains("Current frame accumulated time:"),
            "and carries the accumulated lines: {clipboard:?}"
        );
        assert!(
            clipboard.contains("for 3 instances on thread"),
            "including the thread's instance count: {clipboard:?}"
        );
    }
    /// The view stops at Unity's minimum window and at the whole frame, and
    /// never pans outside the frame.
    #[test]
    fn the_timeline_view_stops_at_its_zoom_limits() {
        let mut view = TimelineView::default();
        assert_eq!(TimelineView::min_span(0), 1.0);
        assert_eq!(TimelineView::min_span(45_000_000), 0.05 / 45.0);

        view.zoom_around(1_000.0, 0.5, 45_000_000);
        assert!(
            (view.span - 0.05 / 45.0).abs() < 1e-12,
            "the narrowest window is Unity's 0.05 ms: {view:?}"
        );

        view.pan_by(10.0, 45_000_000);
        assert!(
            (view.pan - (1.0 - view.span)).abs() < 1e-12,
            "the window stops at the frame's end: {view:?}"
        );

        view.zoom_around(0.000_001, 0.5, 45_000_000);
        assert_eq!(
            view,
            TimelineView::default(),
            "zooming out stops at the whole frame"
        );
    }

    /// The ruler picks a step that keeps labels apart at any window and zoom.
    #[test]
    fn ruler_ticks_stay_readable_at_every_frame_length() {
        let fit = TimelineView::default();
        let ticks = ruler_ticks(fit, 45_000_000, 900.0);
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

        // A zoomed window keeps the labels apart and starts at the window's left
        // edge: ticks are absolute frame times, so the first one is not at zero.
        let zoomed = TimelineView {
            pan: 0.41,
            span: 0.05,
        };
        let ticks = ruler_ticks(zoomed, 45_000_000, 900.0);
        let labels: Vec<f32> = ticks
            .iter()
            .filter(|tick| tick.label.is_some())
            .map(|tick| tick.x)
            .collect();
        assert!(labels.len() >= 2, "{labels:?}");
        assert!(labels[0] > 0.0, "ticks slide with the window: {labels:?}");
        for pair in labels.windows(2) {
            assert!(
                pair[1] - pair[0] >= MIN_TICK_SPACING,
                "labels must stay apart when zoomed: {labels:?}"
            );
        }
        // The label decimals follow the step: a 0.05 ms window labels in decimals.
        assert!(
            ticks
                .iter()
                .filter_map(|tick| tick.label.as_deref())
                .all(|label| { label.ends_with("ms") || label.ends_with('s') }),
            "{ticks:?}"
        );

        assert!(!ruler_ticks(fit, 2_000_000, 900.0).is_empty());
        assert!(!ruler_ticks(fit, 2_000_000_000, 900.0).is_empty());
        assert!(ruler_ticks(fit, 0, 900.0).is_empty());
        assert_eq!(tick_label(5.0, 5.0), "5ms");
        assert_eq!(tick_label(0.05, 0.05), "0.05ms");
        assert_eq!(tick_label(0.005, 0.005), "0.005ms");
        assert_eq!(tick_label(2000.0, 2000.0), "2s");
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
        input.screen_rect = Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            egui::vec2(800.0, 600.0),
        ));
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
            kairos_profile::scope!(Category::Others, "probe.frame");
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
            kairos_profile::scope!(Category::Others, "probe.frame");
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
        eprintln!("PROBE[4] stopped       latest_frames={}", latest(&panel));
    }
}
