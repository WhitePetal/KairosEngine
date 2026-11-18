//! Profiler panel: Profiler window over the structured capture.
//!
//! The panel owns no capture data. It reads the
//! [`kairos_profile::ProfileSession`] through [`crate::HostView::profiler`], keeps
//! only view state (recording flag, selected frame, live-follow, the selected
//! block, the module area's height, the timeline's zoom and scroll) and draws
//! bands, top to bottom:
//!
//! * **toolbar** — record toggle, frame transport, `Frame: n / m`, Clear and
//!   Save, separated into segments by 1 px rules. Save writes the capture to
//!   `Library/Profiler` and the toolbar names the written files next to the
//!   button (the exact paths are in its hover text), so a capture can be handed
//!   to another reader without a file dialog;
//! * **module area** — Modules pane with this editor's own time axis: a
//!   pinned clock ruler on top and a horizontal min-max bar at the bottom around
//!   the fixed-height module cards. CPU Usage has the category legend and a
//!   stacked frame chart whose bars sit at their capture times, scaled so the
//!   tallest bar fills 80% of the chart with four ms/FPS guide tiers;
//!   GPU Usage is a fixed-height empty card until the capture records GPU
//!   timings;
//! * **splitter** — the draggable rule between the module area and the
//!   timeline, split view handle;
//! * **timeline** — a pinned header (view selector, Live toggle, CPU/GPU
//!   summary) and time ruler over one track per registered thread lane, a
//!   zoomable time axis and two scroll bars. The view selector switches
//!   the band between Timeline and Hierarchy views.
//!
//! Layout, colours and measured geometry are [`ProfilerStyle`] tokens loaded
//! from `Preferences/Styles/Profiler.ron`.
//!
//! # Categories
//!
//! The chart stacks the engine modules ([`Category`]): Rendering, Scripts,
//! Physics, Audio, IO, Editor, VSync and Others. `IO` is the asset server,
//! loaders, processors and filesystem watching: resource
//! loading is its own bucket instead of disappearing into `Others`. A marker
//! declares its category at the call site — `kairos_profile::scope!(Category::Physics,
//! ...)` appends a `Physics/` tag to the marker name — and the executor tags the
//! sample data of its shared `Kairos.System` scope the same way. Classification
//! is therefore part of the code that emits the marker; only third-party scopes
//! (wgpu, rapier, epaint) are still read from their source crate.
//!
//! Subsystem systems (physics, audio, rendering, IO, editor) fold into their
//! module's bucket: they never get a legend row of their own. Everything else is
//! `Scripts` (a system's default when it declares nothing), and the legend
//! refines it into one row per ECS system. Legend swatches are toggles,
//! a hidden category keeps its row and its time and leaves the chart.
//! Scheduler wrappers (`Kairos.Stage`, `Kairos.Engine.Frame`) and the scheduler
//! drivers (`run_main`, `run_fixed_main_loop`) contribute no time of their own:
//! what they measure is their children's work, counted where it ran, and the
//! remainder becomes `Others` — uninstrumented time, presentation waits and the
//! engine's own bookkeeping, i.e. the part no category claims.
//!
//! The timeline paints every block in its category's colour and labels it
//! `Category · marker (duration)` (a system block uses the short system name
//! from its data, and the duration is the block's inclusive time — the number
//! prints after the name). That tag is a property of each block, not a
//! contiguous region: with ECS parallelism one module's systems can run on
//! several workers or interleave with other work on a lane. The block still
//! reads as its module — for example a third-party
//! `Surface::get_current_texture` block reads as
//! `VSync · Surface::get_current_texture (0.021ms)`.
//!
//! # Module area and timeline split
//!
//! The two bands share the space under the toolbar and are separated by a
//! draggable splitter — `TwoPaneSplitView` in `ProfilerWindow`, which
//! persists the fixed pane's height as `MainSplitView.FixedPaneSize`. The module
//! pane is the fixed pane: it keeps the height the user last dragged to and the
//! timeline takes whatever is left, so resizing the panel grows the timeline.
//!
//! The module pane is a **window onto a stack of module cards** (CPU Usage, then
//! the empty GPU Usage card), plus two pinned rows of
//! its own: the clock ruler on top and the time zoom bar at the bottom. The cards
//! sit at their skin heights while the pane can show them, so dragging the
//! splitter down reveals the modules below instead of stretching the CPU chart,
//! and the wheel over the name column or the pane's scroll bar moves a stack
//! taller than the pane. The splitter cannot grow the pane past the whole stack
//! plus its pinned rows, so there is never dead space under the last card.
//!
//! The pane starts at the height its cards ask for: the CPU card whole plus half
//! of the card below it, so a fresh panel shows the usage chart and the top of the
//! next module. The default is derived from the cards (see `module_default_height`)
//! rather than a fixed token, so a skin that resizes a card, or a later module
//! that follows the GPU one, keeps the same relationship; a double-click on the
//! splitter restores it. The timeline takes the rest of the panel, so on a panel
//! too short for the cards it starts collapsed to its header and the splitter
//! trades the two bands.
//!
//! The CPU chart is the pane's reason to exist, so it is never cut off: a pane
//! shorter than the CPU card's skin height shrinks the card (and with it the
//! chart) instead of clipping it. A usage chart is read from its zero line up —
//! the bars grow from the plot's bottom — so a card clipped at the pane's edge
//! would hide exactly the short bars a high frame rate produces and read as
//! "nothing was recorded". The GPU card keeps its height below the CPU card, so a
//! short pane scrolls to the GPU instead of shrinking the CPU card to leave the
//! GPU room.
//!
//! # Module time axis
//!
//! A capture can be stopped and started again: the stored frames stay, and the
//! next take continues on the same clock, so the capture has pauses in it.
//! [`kairos_profile`] records those takes as `RecordingSegment`s and the local
//! wall clock at the capture origin, and the module area's axis flattens the
//! recorded time: the takes end to end, every pause compressed to a fixed-width
//! break band. A band prints its two sides — the clock time of the stop on the
//! left and of the restart on the right — and the tick labels are the real clock
//! readings of the capture, not elapsed time. Per-frame timestamps already
//! existed (`FrameSnapshot::start_ns/end_ns`), so the axis costs no per-frame
//! work; only a stop or start adds a segment. The mapping and the ruler are the
//! shared [`kairos_editor_widgets::timeline_axis`] widget; this panel supplies
//! the capture's takes and its clock labels, so an animation or timeline panel
//! gets the same axis without a Profiler-specific copy.
//!
//! The axis shows the most recent `window_fram_count` frames by default, and the
//! header's Live toggle keeps its right edge on the newest frame. The min-max bar
//! under the cards is the same widget as the timeline's: dragging its thumb pans
//! the window (and turns Live off), dragging either end zooms it, the wheel over
//! the chart columns zooms around the pointer, and a double-click on the ruler
//! restores the default window. Clicking a chart column selects the frame
//! recorded at that time — the gesture belongs to the time column rather than to
//! a bar, so a GPU chart gets it for free once it has bars. Dragging on the
//! chart or on the ruler scrubs the selection, which is also how the
//! selected-frame cursor is moved: the cursor is one line from the clock row
//! through the cards, riding the middle of the frame's bar so the bar it selects
//! is unambiguous. Stepping frames with the toolbar pans the window to the
//! selected frame when it is off screen, so the transport reaches the first frame
//! of the first take.
//!
//! Both panes keep a minimum. The module pane keeps its pinned rows, its first
//! title row and two legend rows. The timeline's minimum is only its header row:
//! dragging the splitter down collapses the band to the view selector, the Live
//! toggle and the CPU/GPU summary, and the ruler, tracks and bars are clipped, so
//! the module pane can take the whole panel when a capture has more modules than
//! the panel has room for. When the panel is shorter than both minima, the module
//! pane keeps its minimum and the timeline gives way first, which is what
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
//! # Hierarchy view
//!
//! The view selector's Hierarchy entry replaces the per-thread tracks with
//! aggregated marker tree for the selected frame: the whole frame's
//! work, merged across every thread. That is where a system that ran on a task
//! pool worker is found: the timeline shows it on its worker's track, but the
//! stage wrapper that scheduled it has no child block on the main thread, so the
//! stage alone never explains its time.
//!
//! Columns are Total, Self, Calls and Max; clicking a column header sorts the
//! sibling lists by it (caret points down for the default best-first
//! order, up after a second click). Rows with children carry a collapse caret
//! and keep their collapse state by marker path, so a re-sort does not lose it.
//! Hovering a row reports the full marker name (a system keeps its module path),
//! its totals and its `file:line`. The wheel scrolls the rows and the right-hand
//! bar is the same scrollbar widget as the timeline's.
//!
//! # Timeline navigation
//!
//! The tracks show a window into the selected frame, `ZoomableArea`:
//! [`TimelineView`] keeps it as a fraction of the frame, so stepping frames or
//! resizing the panel keeps the zoom. The gestures are CPU-module ones:
//!
//! * the wheel over the time area zooms horizontally around the pointer, down
//!   to `minWidth` of 0.05 ms and out to the whole frame (the capture
//!   has no neighbouring frames to reveal past the edges);
//! * the wheel over the thread names or the vertical bar scrolls the rows;
//! * the wheel over the horizontal bar pans;
//! * middle-drag, or alt-drag, pans the tracks; alt+right-drag zooms;
//! * `A` fits the whole frame again, FrameAll.
//!
//! The two bars are [`kairos_editor_widgets::min_max_scroll::MinMaxScroll`],
//! min-max scroller: the horizontal one's thumb is the shown window, so
//! dragging it pans and dragging either end resizes it — the timeline's second
//! zoom gesture. egui's `ScrollArea` bar is a plain thumb that cannot resize a
//! window, which is why the timeline lays out its own bars.
//!
//! # Timeline selection
//!
//! Clicking a block selects it, The block keeps a 1 px outline and a
//! details box opens under it with `Copy` / `Show` buttons. The box names the
//! block (`Kairos.Stage[Update]`, or a system's full module path), then reports
//! its category, the marker's call site (`file:line`), its self time and the
//! recorded call stack (the enclosing marker chain on that thread). The self
//! line is what explains a wrapper: when no child sample was recorded,
//! `Kairos.Stage`'s whole block is unattributed time and the box says so
//! (`Self: 40.57ms (no child samples recorded)`) instead of leaving the block a
//! black box.
//!
//! The accumulated-time section follows the rules: it appears only when
//! that adds information — `ProfilerTimelineGUI.DoSelectionTooltip`,
//! the section needs more than one instance in the frame, the thread line more
//! than one instance on the block's thread, and the frame line instances on
//! other threads too. A block that ran once on one thread therefore gets no
//! accumulated section. `Copy` puts the whole box text on the clipboard.
//! Clicking the same block again, or empty track, dismisses the box. A click
//! also turns Live off: the box describes one frame, and a capture that is
//! still recording would replace that frame every tick.
//!
//! # Not implemented yet (the seams are marked `TODO(profiler)` in the code)
//!
//! * module selection (`Profiler Modules`) and the Raw / Call Stacks views: the
//!   view selector only switches Timeline and Hierarchy;
//! * thread grouping and resizable
//!   thread rows: the vertical bar scrolls, but the rows have no splitter and no
//!   vertical zoom;
//! * the module legend still only has the rows the module area leaves free, so
//!   it shows the top few and `+N more`; the Hierarchy view is where the whole
//!   ranking lives;
//! * GPU timings, per-marker colour overrides (the views use the category
//!   palette), hover details on timeline blocks (they report after a click, not
//!   on hover; Hierarchy rows do have a tooltip) and the details box's Show menu
//! * the tooltip's cross-frame line: it needs samples that span frames to be linked, and the
//!   capture currently parses each frame on its own;
//! * Range selection (drag across the tracks to measure a window), the
//!   `F` shortcut that frames the selected block, and the ghost frames
//!   draws before and after the selected one while the timeline is panned.
//!
//! The JSON view that predates the rewrite (`CachedFrame::json`) is still in
//! this file but is no longer wired into [`Panel::ui`]; its marker table is
//! what the capture's JSON export carries.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use egui::{
    Align2, Context, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    WidgetText,
};
use kairos_editor_core::StyleValue;
use kairos_editor_widgets::button::{Button, ButtonAlign, ButtonStyle};
use kairos_editor_widgets::color::Color32Ext;
use kairos_editor_widgets::divider::Divider;
use kairos_editor_widgets::menu::MenuStyle;
use kairos_editor_widgets::min_max_scroll::{MinMaxScroll, MinMaxScrollStyle, ScrollWindow};
use kairos_editor_widgets::paint;
use kairos_editor_widgets::splitter::Splitter;
use kairos_editor_widgets::style::{Style, StyleFile};
use kairos_editor_widgets::text::TextStyle;
use kairos_editor_widgets::timeline_axis::{TimelineRuler, TimelineRulerStyle};
use kairos_editor_widgets::ui_image;
use kairos_profile::{
    CaptureFiles, Category, FrameSnapshot, HierarchyRow, ProfileSession, SYSTEM_MARKER,
    SampleSnapshot, ScopeInfo, ThreadSnapshot, classify_system, strip_tag,
};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;
use crate::panels::profiler::hierarchy::HierarchySort;
use crate::panels::profiler::style::ProfilerStyle;
use crate::skin::{PanelSkin, StyleFields};

use self::axis::ModuleAxis;

mod axis;
mod hierarchy;
mod main_toolbar;
mod modules;
pub mod style;

/// What the cached capture frame was built from.
#[derive(Clone, Copy, PartialEq, Eq)]
struct CaptureFrameState {
    /// The capture's revision: a new capture invalidates the cached frame.
    revision: u64,
    /// The selected frame's index in the capture.
    frame: usize,
}

impl Default for CaptureFrameState {
    /// Nothing cached: the sentinels match no frame, so the first cache pass
    /// rebuilds.
    fn default() -> Self {
        Self {
            revision: u64::MAX,
            frame: usize::MAX,
        }
    }
}

/// The cached reduction of one selected frame. Rebuilt when the capture
/// revision, the selected frame or the thread filter changes.
struct CaptureFrame {
    snapshot: Arc<FrameSnapshot>,
    scopes: Vec<ScopeInfo>,
    /// Category per full system name. The aggregation strips the category tag
    /// from the executor's data, so the Hierarchy view reads the category from
    /// the samples here instead of re-deriving it from a stripped name.
    system_categories: BTreeMap<String, Category>,
    /// The selected frame's Scripts systems by inclusive time, highest first.
    systems: Vec<(String, u64)>,
    /// The timeline's rows: every registered thread lane (whether or not it ran
    /// in the selected frame) followed by threads that only appeared in the
    /// frames. Fixed per capture, so rows do not jump between frames.
    lanes: Vec<Lane>,
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
/// Timeline is a `ZoomableArea`: a window into the frame time that the
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
    /// The narrowest window the timeline zooms to: (0.05 ms, the shortest slice its timeline reads).
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
    /// track from 0 to 1: the time under the pointer stays under it
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

/// Which view the timeline band shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PanelView {
    #[default]
    Timeline,
    Hierarchy,
}

impl PanelView {
    fn label(self) -> &'static str {
        match self {
            Self::Timeline => "Timeline",
            Self::Hierarchy => "Hierarchy",
        }
    }
}

/// A small filled triangle: the Hierarchy view's collapse caret and sort
/// indicator (no icon assets needed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Triangle {
    Down,
    Right,
    Up,
}

fn paint_triangle(
    painter: &egui::Painter,
    center: egui::Pos2,
    size: f32,
    triangle: Triangle,
    color: egui::Color32,
) {
    let half = size * 0.5;
    let points = match triangle {
        Triangle::Down => vec![
            egui::pos2(center.x - half, center.y - half * 0.6),
            egui::pos2(center.x + half, center.y - half * 0.6),
            egui::pos2(center.x, center.y + half * 0.7),
        ],
        Triangle::Up => vec![
            egui::pos2(center.x - half, center.y + half * 0.6),
            egui::pos2(center.x + half, center.y + half * 0.6),
            egui::pos2(center.x, center.y - half * 0.7),
        ],
        Triangle::Right => vec![
            egui::pos2(center.x - half * 0.7, center.y - half),
            egui::pos2(center.x + half * 0.7, center.y),
            egui::pos2(center.x - half * 0.7, center.y + half),
        ],
    };
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

/// The rectangles the timeline lays out for one pass.
///
/// The header and ruler are pinned; only `body` scrolls. `track_left` and
/// `track_right` bound the zoomable time area: the thread name column ends at
/// `track_left` (it does not pan) and the vertical scroll bar starts at
/// `track_right`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TimelineLayout {
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

#[crate::panel(PROFILER = "profiler")]
pub(crate) struct ProfilerPanel {
    style: ProfilerStyle,
    recording: bool,
    follow_latest: bool,
    selected_frame: usize,
    /// Which view the timeline band draws (view selector).
    view_kind: PanelView,
    /// Hierarchy rows whose subtree is collapsed, by path key.
    collapsed: BTreeSet<String>,
    frame_state: CaptureFrameState,
    frame: Option<CaptureFrame>,
    /// The aggregated tree, sorted by the Hierarchy view's current column.
    frame_rows: Vec<HierarchyRow>,
    /// The Hierarchy view's sort column and direction.
    hierarchy_sort: HierarchySort,
    hierarchy_descending: bool,
    /// Result of the last Save click: the exact files written, or the error.
    /// Kept so the panel can show where the capture went.
    save: Option<Result<CaptureFiles, String>>,
    /// Categories the user switched off in the legend, indexed by
    /// [`Category`]. Swatches are toggles: a hidden category keeps its
    /// row and its time, and leaves the chart's stack.
    hidden: [bool; Category::COUNT],
    /// The timeline block the user clicked: it keeps an outline, and the
    /// details box describes it until another block or empty track is clicked.
    selection: Option<Selection>,
    /// Height of the module area in points. The splitter between it and the
    /// timeline keeps this, so the timeline gets the rest of the panel; a drag
    /// replaces it for the session. It starts at
    /// [`Self::module_default_height`], the height the cards ask for.
    /// Persists the same value as `ProfilerWindow.MainSplitView.FixedPaneSize`;
    /// the editor has no per-panel layout settings file to write it to yet.
    module_height: f32,
    /// Vertical offset of the module pane's card stack, in points.
    module_scroll: f32,
    /// The module area's compressed time axis: the capture's takes and the
    /// window over them that the zoom bar pans and resizes.
    module_axis: ModuleAxis,
    /// Whether the axis has been given its first window (the last frames).
    module_view_started: bool,
    /// Whether the axis still shows the default last-N-frames window and tracks
    /// it while Live is on. The wheel and the zoom bar leave this mode; a
    /// double-click on the ruler restores it.
    module_default_window: bool,
    /// The usage chart's vertical scale in ms: the tallest visible bar fills
    /// 80% of the chart. Eased towards its target instead of snapping.
    module_scale_ms: f32,
    /// Whether the next pass pans the axis window to the selected frame.
    reveal_frame: bool,
    /// The slice of the selected frame the tracks show.
    timeline_view: TimelineView,
    /// Vertical offset of the timeline's rows, in points.
    track_scroll: f32,
}

impl Default for ProfilerPanel {
    fn default() -> Self {
        let mut panel = Self {
            style: ProfilerStyle::dark(),
            // The module pane's height needs the style, so it is derived below.
            module_height: 0.0,
            module_scroll: 0.0,
            module_axis: ModuleAxis::default(),
            module_view_started: false,
            module_default_window: true,
            module_scale_ms: 0.0,
            reveal_frame: false,
            recording: false,
            follow_latest: true,
            selected_frame: 0,
            frame_state: CaptureFrameState::default(),
            frame: None,
            frame_rows: vec![],
            hierarchy_sort: HierarchySort::default(),
            hierarchy_descending: true,
            view_kind: PanelView::default(),
            collapsed: BTreeSet::new(),
            save: None,
            hidden: [false; Category::COUNT],
            selection: None,
            timeline_view: TimelineView::default(),
            track_scroll: 0.0,
        };
        panel.module_height = panel.module_default_height();
        panel
    }
}

impl Panel for ProfilerPanel {
    fn icon(&self) -> Option<&str> {
        Some(&self.style.icons.tab)
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    /// The Profiler skin, editable from Preferences.
    fn skin(&self) -> Option<PanelSkin> {
        Some(PanelSkin {
            panel: PROFILER,
            title: "Profiler",
            file: ProfilerStyle::FILE,
            fields: self.style.style_fields(),
        })
    }

    fn set_skin_field(&mut self, path: &str, value: &StyleValue) -> bool {
        self.style.set_style_field(path, value)
    }

    fn save_skin(&self, ctx: &Context) -> Result<(), String> {
        crate::skin::save_skin::<ProfilerStyle>(&self.style, ctx.theme())
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

        // A skin may size the module cards differently; the pane starts at the
        // height they ask for and a drag replaces it for the session.
        self.module_height = self.module_default_height();

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
        // Point the panel at the newest frame while the header's Live toggle is on
        // (and keep the index inside the capture after a clear).
        let last = session.frames.len().saturating_sub(1);
        if self.follow_latest || self.selected_frame > last {
            self.selected_frame = last;
        }
        if session.frames.is_empty() {
            self.frame = None;
            // A cleared capture has no cached frame; the next one rebuilds it.
            self.frame_state = CaptureFrameState::default();
            // A cleared capture restarts the module area from its defaults: the
            // last-frames window and an unpinned vertical scale.
            self.module_view_started = false;
            self.module_default_window = true;
            self.module_scale_ms = 0.0;
        } else {
            self.refresh_frame_state(&session);
        }

        // The panel is measured chrome: rows are flush and the gaps
        // between sections are rules, not item spacing.
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

        // top to bottom: toolbar, module area, splitter, timeline.
        self.main_toolbar(ui, profiler, &session);
        self.bands(ui, &session);
    }
}

impl ProfilerPanel {
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
    /// The module pane is *fixed pane*: it keeps the height the user
    /// dragged the splitter to and the timeline takes what is left. The
    /// splitter applies its drag to `module_height` on this pass, but the pane
    /// above it was already allocated, so the two panes move on the next pass —
    /// the same one-pass lag a dock separator has. The pane stops growing at the
    /// card stack's height: beyond it there is nothing more to reveal, and the
    /// timeline stops shrinking at its header row (see
    /// [`Self::min_timeline_height`]).
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
            .reset_to(self.module_default_height())
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

    /// Smallest module pane: its first card's title row, two legend rows, and
    /// the pinned time axis and zoom bar around them.
    fn min_module_height(&self) -> f32 {
        self.module_chrome_height()
            + self.style.module_header_height
            + 2.0 * self.style.legend_row_height
    }

    /// Height of the pinned time axis row at the top of the module pane.
    fn module_axis_height(&self) -> f32 {
        self.style.ruler_height
    }

    /// Height of the pinned zoom bar at the bottom of the module pane.
    fn module_bar_height(&self) -> f32 {
        self.style.scrollbar_width
    }

    /// The module pane's fixed chrome: the time axis row, the rule under it, and
    /// the horizontal zoom bar.
    fn module_chrome_height(&self) -> f32 {
        self.module_axis_height() + self.style.separator_width + self.module_bar_height()
    }

    /// The module card stack at the cards' skin heights: the two cards and the
    /// rule between. This is the stack's preferred height; in a shorter pane the
    /// CPU card yields first (see [`Self::module_cpu_fit`]).
    fn module_cards_height(&self) -> f32 {
        self.style.module_cpu_height + self.style.separator_width + self.style.module_gpu_height
    }

    /// Everything the module pane shows at the stack's preferred height: the
    /// card stack plus its pinned chrome. The splitter stops here so there is no
    /// dead space under the last card.
    fn module_content_height(&self) -> f32 {
        self.module_cards_height() + self.module_chrome_height()
    }

    /// The module pane's default height: its pinned chrome, the CPU card whole,
    /// the rule under it and half of the card below it (today the empty GPU card).
    ///
    /// It is derived from the cards rather than kept as a fixed token, so a skin
    /// that resizes a card — or a module added after the GPU one — keeps the same
    /// relationship: the default shows the usage chart whole plus the top half of
    /// the next module. The splitter restores this height on a double-click.
    fn module_default_height(&self) -> f32 {
        self.module_chrome_height()
            + self.style.module_cpu_height.max(0.0)
            + self.style.separator_width
            + 0.5 * self.style.module_gpu_height.max(0.0)
    }

    /// The CPU Usage card's height in a card viewport `viewport` points tall: its
    /// skin height, or the whole viewport when the pane is shorter.
    ///
    /// The card yields to the pane instead of being clipped, because a usage
    /// chart is read from its zero line up: the bars grow from the plot's bottom,
    /// so a card cut off at the pane's edge hides the short bars a high frame rate
    /// produces and reads as "CPU Usage recorded nothing".
    fn module_cpu_fit(&self, viewport: f32) -> f32 {
        self.style.module_cpu_height.clamp(0.0, viewport.max(0.0))
    }

    /// Smallest timeline: its header row and the rule under it. The band gives
    /// way to the module pane, so dragging the splitter down collapses the
    /// timeline to the view selector, the Live toggle and the CPU/GPU summary;
    /// the ruler, the tracks and the bars are clipped above the pane's edge.
    fn min_timeline_height(&self) -> f32 {
        self.style.toolbar_height + self.style.separator_width
    }

    /// The module area's pinned time axis: real clock ticks over the chart
    /// columns, with a break band for every pause between two takes.
    ///
    /// The axis and its ruler are
    /// [kairos_editor_widgets::timeline_axis]; this panel supplies the capture's
    /// takes and its clock labels.
    fn paint_module_axis(&self, ui: &Ui, row: Rect, chart_left: f32, chart_right: f32) {
        let track = Rect::from_min_max(
            egui::pos2(chart_left, row.top()),
            egui::pos2(chart_right.max(chart_left), row.bottom()),
        );
        let style = TimelineRulerStyle {
            background: self.style.ruler_bg.to_egui(),
            text: self.style.ruler_text.to_egui(),
            break_background: self.style.break_bg.to_egui(),
            font: self.small_font(),
        };
        let labels = self.module_axis.labels();
        TimelineRuler::new(
            &self.module_axis,
            self.module_axis.ruler_scale(),
            &labels,
            &style,
        )
        .show(ui, track);

        // Rule between the name column and the clock columns, like the ruler
        // over the timeline's tracks.
        self.paint_vertical_rule(
            ui,
            egui::pos2(track.left() - self.style.separator_width, row.top()),
            row.height(),
            self.style.separator.to_egui(),
        );
    }

    /// The module pane's vertical scroll bar: the shared non-resizable
    /// MinMaxScroll, so it looks and behaves like the timeline's bars.
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

    /// Select the frame under a chart column: clicking a usage chart at a time
    /// selects the frame recorded then. The gesture belongs to the chart
    /// columns, not to a particular bar, so a GPU chart gets it for free once it
    /// has bars.
    fn select_module_frame_at(&mut self, x: f32, session: &ProfileSession) {
        let Some((take, capture)) = self.module_axis.source_at_screen(x) else {
            return;
        };
        let Some(index) = frame_at_capture(&self.module_axis, session, take, capture) else {
            return;
        };
        self.selected_frame = index;
        self.follow_latest = false;
    }

    fn paint_cpu_module(&mut self, ui: &mut Ui, row: Rect, session: &ProfileSession) {
        let name_width = self.style.module_name_width.min(row.width());
        let name_col =
            Rect::from_min_max(row.min, egui::pos2(row.left() + name_width, row.bottom()));
        // The chart starts below the module title row.
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

        // Clicking or dragging the chart selects the frame recorded at that
        // time: the gesture is about the time column, not about hitting a bar,
        // so a GPU chart gets it for free once it has bars. Dragging scrubs the
        // selection, which is also how the selected-frame cursor moves.
        let response = ui.interact(
            chart,
            ui.id().with("profiler.module.cpu"),
            Sense::click_and_drag(),
        );
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let x = (pos.x - chart.left()).clamp(0.0, chart.width());
                self.select_module_frame_at(x, session);
            }
        }
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        } else if response.hovered() {
            let pointer = ui.input(|input| input.pointer.hover_pos());
            if let (Some(pointer), Some(cursor)) = (pointer, self.module_cursor_column(session)) {
                if (pointer.x - chart.left() - cursor).abs() <= 3.0 {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
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

    /// The usage chart: one stacked bar per frame, positioned on the module
    /// area's time axis so the bars line up with the clock ruler above them,
    /// with Unity's ms/FPS guide lines and the selected-frame cursor.
    ///
    /// The vertical scale is the tallest visible bar divided by the 80% fill
    /// target (clamped to Unity's module-chart range), so the graph uses the
    /// chart's height whatever the frame rate is.
    fn paint_chart(&mut self, ui: &Ui, plot: Rect, session: &ProfileSession) {
        if plot.width() <= 0.0 || plot.height() <= 0.0 {
            return;
        }
        let painter = ui.painter().with_clip_rect(plot);
        let text = self.style.ruler_text.to_egui();
        if session.frames.is_empty() || self.module_axis.is_empty() {
            painter.text(
                plot.center(),
                Align2::CENTER_CENTER,
                "Waiting for the first captured frame…",
                self.small_font(),
                text,
            );
            return;
        }

        // Where every visible frame's column is. A bar's left edge follows its
        // frame start, so the bars sit at the times the ruler labels.
        let zoom = self.module_axis.zoom();
        let mut columns: Vec<ChartColumn> = Vec::new();
        for segment in self.module_axis.visible_segments() {
            let (capture_start, capture_end) = segment.source;
            let frames = &session.frames;
            let lo = frames.partition_point(|frame| (frame.start_ns as f64) < capture_start);
            let hi = frames.partition_point(|frame| (frame.start_ns as f64) < capture_end);
            for (offset, frame) in frames[lo..hi].iter().enumerate() {
                let x = segment.x.0 + ((frame.start_ns as f64 - capture_start) * zoom) as f32;
                let width = (((frame.end_ns - frame.start_ns) as f64 * zoom) as f32).max(1.0);
                columns.push(ChartColumn {
                    frame: lo + offset,
                    x,
                    width,
                    ms: self.bar_ms(frame),
                });
            }
        }

        // While recording, the tallest bar can change every pass; the scale
        // eases towards its target instead of snapping. Its first measurement
        // snaps, and a cleared capture resets it to that state.
        let tallest = columns
            .iter()
            .map(|column| column.ms)
            .fold(0.0_f32, f32::max);
        let target =
            (tallest.max(0.001) / USAGE_BAR_FILL).clamp(MIN_USAGE_SCALE_MS, MAX_USAGE_SCALE_MS);
        if self.module_scale_ms <= 0.0 {
            self.module_scale_ms = target;
        } else {
            let next = self.module_scale_ms + (target - self.module_scale_ms) * USAGE_SCALE_EASE;
            self.module_scale_ms = if (next - target).abs() < 0.005 {
                target
            } else {
                next
            };
            if (self.module_scale_ms - target).abs() > 0.005 {
                ui.ctx().request_repaint();
            }
        }
        let scale = self.module_scale_ms.max(0.001);

        // Break bands first: they are the compressed pauses, and two takes'
        // bars never touch across them.
        for band in self.module_axis.breaks() {
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(plot.left() + band.x, plot.top()),
                    egui::vec2(band.width, plot.height()),
                ),
                CornerRadius::ZERO,
                self.style.break_bg.to_egui(),
            );
        }

        // Clock grid: a faint vertical line under every labelled ruler tick, so
        // a bar reads against the time axis above it.
        let grid = self.style.chart_grid.to_egui().gamma_multiply(0.45);
        let ruler = self.module_axis.ruler_scale();
        for tick in self
            .module_axis
            .ticks(&ruler)
            .into_iter()
            .filter(|tick| tick.major)
        {
            let x = plot.left() + tick.x;
            if x <= plot.left() || x >= plot.right() {
                continue;
            }
            painter.rect_filled(
                Rect::from_min_size(egui::pos2(x, plot.top()), egui::vec2(1.0, plot.height())),
                CornerRadius::ZERO,
                grid,
            );
        }

        // One stacked bar per frame. Unity anchors Others at the bottom and
        // stacks up the legend, so the first row is the topmost band.
        for column in &columns {
            let frame = &session.frames[column.frame];
            let mut bottom = plot.bottom();
            for category in Category::ALL.iter().rev().copied() {
                if self.hidden[category as usize] {
                    continue;
                }
                let ms = self.category_ms(frame, category);
                if ms <= 0.0 {
                    continue;
                }
                let height = (ms / scale * plot.height()).max(0.5);
                painter.rect_filled(
                    Rect::from_min_max(
                        egui::pos2(plot.left() + column.x, bottom - height),
                        egui::pos2(plot.left() + column.x + column.width, bottom),
                    ),
                    CornerRadius::ZERO,
                    self.category_color(category),
                );
                bottom -= height;
            }
        }

        // Unity's ms/FPS guide lines, placed under the 80% scale. They are
        // drawn over the bars so their chips stay readable. A pane barely taller
        // than its card header leaves a short plot that bunches the tiers up, so
        // a chip whose label would collide with the one above it steps aside and
        // leaves its line — the way the clock ruler drops colliding labels.
        let mut reserved: Vec<(f32, f32)> = Vec::new();
        for (ms, label) in usage_grid(scale) {
            if *ms > f64::from(scale) {
                continue;
            }
            let y = plot.bottom() - (*ms as f32 / scale * plot.height());
            self.paint_chart_guide(ui, plot, y, label, &mut reserved);
        }
    }

    /// The selected frame's centre column on the module time axis.
    ///
    /// The cursor rides the middle of the frame's bar, not its left edge, so the
    /// bar under the line is unambiguous.
    fn module_cursor_column(&self, session: &ProfileSession) -> Option<f32> {
        let frame = session.frames.get(self.selected_frame)?;
        let start = frame.start_ns as f64;
        let end = frame.end_ns as f64;
        self.module_axis
            .screen_of_source(start + (end - start) * 0.5)
    }

    /// One usage guide: a 1 px rule across the chart with Unity's
    /// xxms (xxFPS) chip at the left edge. The line stays at its true height;
    /// only the chip is pushed inside the plot, and `reserved` (the bands of the
    /// chips already drawn) drops a chip that a shorter plot would stack on top
    /// of another one.
    fn paint_chart_guide(
        &self,
        ui: &Ui,
        plot: Rect,
        y: f32,
        label: &str,
        reserved: &mut Vec<(f32, f32)>,
    ) {
        Divider::horizontal()
            .thickness(1.0)
            .color(self.style.chart_grid.to_egui())
            .paint_in(
                ui,
                Rect::from_min_size(egui::pos2(plot.left(), y), egui::vec2(plot.width(), 1.0)),
            );
        let half = self.style.legend_row_height * 0.5;
        if plot.height() < self.style.legend_row_height {
            // A chip is taller than the plot: its line is the whole guide.
            return;
        }
        let center = y.clamp(plot.top() + half, plot.bottom() - half);
        let band = (center - half, center + half);
        if reserved
            .iter()
            .any(|(top, bottom)| band.0 < *bottom && band.1 > *top)
        {
            return;
        }
        reserved.push(band);
        self.paint_chip(ui, egui::pos2(plot.left() + 6.0, center), label);
    }

    /// The timeline rows: registered lanes plus threads seen in the frames.
    fn lanes(&self) -> &[Lane] {
        self.frame
            .as_ref()
            .map_or(&[], |frame| frame.lanes.as_slice())
    }

    /// The selected frame's Scripts systems, highest first.
    fn system_entries(&self) -> &[(String, u64)] {
        self.frame
            .as_ref()
            .map_or(&[], |frame| frame.systems.as_slice())
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

        if self.view_kind == PanelView::Hierarchy {
            self.hierarchy(ui, &layout, session);
            return;
        }

        let frame = session.frames.get(self.selected_frame).map(Arc::as_ref);
        let duration_ns = frame.map_or(0, |frame| frame.duration_ns());
        self.timeline_view.clamp(duration_ns);

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
        self.timeline_view.clamp(duration_ns);
        self.track_scroll = self.track_scroll.clamp(0.0, max_scroll);

        let view = self.timeline_view;
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
                        self.timeline_view.zoom_around(factor, focus, duration_ns);
                    }
                    if wheel.x != 0.0 {
                        let delta =
                            -f64::from(wheel.x) / f64::from(track_width) * self.timeline_view.span;
                        self.timeline_view.pan_by(delta, duration_ns);
                    }
                    true
                } else if layout.body.contains(pos) || layout.vertical_bar.contains(pos) {
                    self.track_scroll -= wheel.y;
                    true
                } else if layout.horizontal_bar.contains(pos) {
                    let step =
                        f64::from(wheel.y) / WHEEL_NOTCH_POINTS * self.timeline_view.span * 0.1;
                    self.timeline_view.pan_by(-step, duration_ns);
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
            self.timeline_view = TimelineView::default();
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
                self.timeline_view.zoom_around(factor, focus, duration_ns);
            }
            ui.ctx().request_repaint();
        } else if body.dragged_by(egui::PointerButton::Middle)
            || (alt && body.dragged_by(egui::PointerButton::Primary))
        {
            let dx = body.drag_delta().x;
            self.timeline_view.pan_by(
                -f64::from(dx) / f64::from(track_width) * self.timeline_view.span,
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
        let mut window =
            ScrollWindow::new(self.timeline_view.pan, self.timeline_view.span, (0.0, 1.0))
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
            self.timeline_view.pan = window.value;
            self.timeline_view.span = window.span;
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

        // View selector: Unity's Timeline / Hierarchy / Call Stacks menu. The
        // Raw and Call Stacks views are still open; Timeline and Hierarchy share
        // this band.
        let view = Rect::from_min_size(
            row.min,
            egui::vec2(self.style.timeline_view_width, row.height()),
        );
        let response = ui.put(
            view,
            Button::new(self.view_kind.label(), &style)
                .width(view.width())
                .height(view.height())
                .caret(),
        );
        let menu_style = MenuStyle::default();
        egui::Popup::menu(&response)
            .style(menu_style.style_modifier())
            .info(egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(
                egui::containers::menu::MenuConfig::MENU_CONFIG_TAG,
                menu_style.config(),
            ))
            .show(|ui| {
                for kind in [PanelView::Timeline, PanelView::Hierarchy] {
                    if ui
                        .selectable_label(self.view_kind == kind, kind.label())
                        .clicked()
                    {
                        self.view_kind = kind;
                    }
                }
            });
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
            .body_lines()
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

    fn refresh_frame_state(&mut self, session: &ProfileSession) {
        let index = self
            .selected_frame
            .min(session.frames.len().saturating_sub(1));
        let current_frame_state = CaptureFrameState {
            revision: session.revision,
            frame: index,
        };
        if self.frame_state == current_frame_state {
            return;
        }

        let frame = Arc::clone(&session.frames[index]);
        let system_categories = collect_system_categories(&frame, &session.scopes);
        let systems = system_entries(&frame, &session.scopes);
        let lanes = collect_lanes(session);
        self.frame = Some(CaptureFrame {
            snapshot: frame,
            scopes: session.scopes.clone(),
            system_categories,
            systems,
            lanes,
        });
        self.sort_hierachy(session);
    }

    fn reset_view(&mut self) {
        self.frame = None;
        // Nothing cached: the next capture rebuilds the frame from scratch.
        self.frame_state = CaptureFrameState::default();
        self.frame_rows.clear();
        self.selected_frame = 0;
        self.follow_latest = true;
        // The selection addresses one frame of the capture being cleared.
        self.selection = None;
        // The zoom describes a frame of the capture being cleared.
        self.timeline_view = TimelineView::default();
        self.track_scroll = 0.0;
        // A new capture invalidates the previously written files' contents.
        self.save = None;
        // Row keys address the cleared frame's tree; a new capture re-uses
        // marker ids, so keep no collapse state across it.
        self.collapsed.clear();
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
        // A wrapper carries its identity in the per-invocation data
        // (`Kairos.Stage` + "Update"), so a stage block reads as
        // `Kairos.Stage[Update]` instead of an anonymous wrapper.
        Some(info) => match sample.data.as_deref().map(strip_tag) {
            Some(data) if !data.is_empty() => format!("{}[{}]", info.name, data),
            _ => info.name.clone(),
        },
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

/// How much of the usage chart's height the tallest visible bar fills.
const USAGE_BAR_FILL: f32 = 0.8;
/// The chart's vertical scale clamps into this range, like Unity's module chart
/// (0.11 ms to 70 ms): one hitch cannot flatten the whole graph.
const MIN_USAGE_SCALE_MS: f32 = 0.11;
const MAX_USAGE_SCALE_MS: f32 = 70.0;
/// How much of the remaining gap the chart's scale closes per pass. Unity eases
/// its module chart's scale the same way (0.4) instead of snapping it.
const USAGE_SCALE_EASE: f32 = 0.4;

/// Unity's module-chart guide lines: four bands chosen by the chart's maximum,
/// each with three ms/FPS readings. The two faster bands use the engine's
/// refresh targets (60/120/240 FPS, 120/240/960 FPS); the slowest keeps Unity's.
const USAGE_GRID_FAST: [(f64, &str); 3] = [
    (1.0, "1ms (1000FPS)"),
    (0.25, "0.25ms (4000FPS)"),
    (0.1, "0.1ms (10000FPS)"),
];
const USAGE_GRID_10MS: [(f64, &str); 3] = [
    (1000.0 / 120.0, "8ms (120FPS)"),
    (1000.0 / 240.0, "4ms (240FPS)"),
    (1000.0 / 960.0, "1ms (960FPS)"),
];
const USAGE_GRID_30MS: [(f64, &str); 3] = [
    (1000.0 / 60.0, "16ms (60FPS)"),
    (1000.0 / 120.0, "8ms (120FPS)"),
    (1000.0 / 240.0, "4ms (240FPS)"),
];
const USAGE_GRID_SLOW: [(f64, &str); 3] = [
    (1000.0 / 15.0, "66ms (15FPS)"),
    (1000.0 / 30.0, "33ms (30FPS)"),
    (1000.0 / 60.0, "16ms (60FPS)"),
];

/// The guide lines for a chart whose vertical scale is `scale_ms`.
fn usage_grid(scale_ms: f32) -> &'static [(f64, &'static str)] {
    if scale_ms < 1.5 {
        &USAGE_GRID_FAST
    } else if scale_ms < 10.0 {
        &USAGE_GRID_10MS
    } else if scale_ms < 30.0 {
        &USAGE_GRID_30MS
    } else {
        &USAGE_GRID_SLOW
    }
}

/// One frame's column on the module area's usage chart.
struct ChartColumn {
    /// Index into ProfileSession::frames.
    frame: usize,
    /// Left edge, in chart columns (the plot starts at zero).
    x: f32,
    /// Column width: the frame's wall time at the axis' zoom.
    width: f32,
    /// Stacked height of the frame's visible categories, in ms.
    ms: f32,
}

/// The frame recorded at `capture` inside the take with index `take`, for a
/// click on a module chart.
///
/// A click between two frames (or in the margin before a take's first frame)
/// falls back to the neighbouring frame of the same take; a click that maps to
/// another take selects nothing. The axis already rejected clicks on a break
/// band.
fn frame_at_capture(
    axis: &ModuleAxis,
    session: &ProfileSession,
    take: usize,
    capture: f64,
) -> Option<usize> {
    let frames = &session.frames;
    if frames.is_empty() {
        return None;
    }
    let in_take = |index: usize| -> bool {
        frames.get(index).is_some_and(|frame| {
            axis.time_of_source(frame.start_ns as f64)
                .is_some_and(|(frame_take, _)| frame_take == take)
        })
    };
    let next = frames.partition_point(|frame| (frame.start_ns as f64) <= capture);
    if let Some(index) = next.checked_sub(1) {
        if in_take(index) {
            return Some(index);
        }
    }
    if in_take(next) {
        return Some(next);
    }
    None
}

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

/// The category of every system that ran in the frame, keyed by its full name.
///
/// The executor's data carries the category tag (`Physics/kairos_physics::…`),
/// but the Hierarchy aggregation strips tags for display; the view therefore
/// reads the per-system category from the frame's samples once per frame.
fn collect_system_categories(
    frame: &FrameSnapshot,
    scopes: &[ScopeInfo],
) -> BTreeMap<String, Category> {
    let mut categories = BTreeMap::new();
    for thread in &frame.threads {
        for sample in &thread.samples {
            let Some(info) = scope_info(scopes, sample.scope) else {
                continue;
            };
            if info.name != SYSTEM_MARKER {
                continue;
            }
            let data = sample.data.as_deref().unwrap_or_default();
            let name = strip_tag(data);
            if name.is_empty() {
                continue;
            }
            categories
                .entry(name.to_owned())
                .or_insert_with(|| classify_system(Some(data)));
        }
    }
    categories
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
    /// Marker name without its category tag, with per-invocation data when the
    /// call site recorded some: `Kairos.Stage[Update]`, or the full system path
    /// for an executor sample.
    title: String,
    /// Duration of the clicked block.
    duration_ns: u64,
    /// The category the block is painted as.
    category: Category,
    /// The marker's call site (`file:line`), empty when the capture has none.
    location: String,
    /// Time inside the block, minus its recorded children.
    self_ns: u64,
    /// Direct child samples recorded inside the block (0 = none).
    children: usize,
    /// Enclosing samples on the same thread, outermost first.
    parents: Vec<String>,
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
    /// The block's own facts: category, source location, self time and the
    /// recorded call stack. The capture doesn't sample native stacks, so the
    /// call stack is the enclosing marker chain puffin recorded on this thread.
    ///
    /// The self line is what explains a wrapper like `Kairos.Stage`: when no
    /// child sample was recorded, its whole block is unattributed time, and the
    /// line says so instead of leaving the 40 ms a mystery.
    fn body_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("Category: {}", self.category.label())];
        if !self.location.is_empty() {
            lines.push(format!("Location: {}", self.location));
        }
        if self.children == 0 {
            lines.push(format!(
                "Self: {} (no child samples recorded)",
                duration_text(self.self_ns)
            ));
        } else {
            lines.push(format!(
                "Self: {} over {} {}",
                duration_text(self.self_ns),
                self.children,
                plural(self.children, "child sample", "child samples")
            ));
        }
        if !self.parents.is_empty() {
            lines.push("Call stack:".to_owned());
            lines.extend(self.parents.iter().map(|parent| format!("  {parent}")));
        }
        lines.extend(self.accumulated_lines());
        lines
    }

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
        lines.extend(self.body_lines());
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

    // A system sample's full path lives in its data; the executor's shared
    // marker name says nothing useful. Everything else is titled the way its
    // timeline block is (`Kairos.Stage[Update]`).
    let title = match scope {
        Some(info) if info.name == SYSTEM_MARKER => {
            strip_tag(sample.data.as_deref().unwrap_or("<unknown system>")).to_owned()
        }
        _ => sample_detail(sample, scope),
    };
    // Self time: the block minus the children this thread recorded inside it.
    // A wrapper with no child samples therefore reports its whole duration as
    // unattributed instead of hiding it.
    let mut self_ns = sample.duration_ns;
    let mut children = 0usize;
    for other in &thread.samples {
        if other.parent == Some(selection.sample as u32) {
            children += 1;
            self_ns = self_ns.saturating_sub(other.duration_ns);
        }
    }
    // The call stack is the enclosing marker chain puffin recorded on this
    // thread, outermost first.
    let mut parents = Vec::new();
    let mut enclosing = sample.parent;
    while let Some(index) = enclosing {
        let Some(parent) = thread.samples.get(index as usize) else {
            break;
        };
        parents.push(format!(
            "{} ({})",
            sample_detail(parent, scope_info(scopes, parent.scope)),
            block_duration_text(parent.duration_ns)
        ));
        enclosing = parent.parent;
    }
    parents.reverse();

    Some(SelectionDetails {
        title,
        duration_ns: sample.duration_ns,
        category: sample_category(sample, scope),
        location: scope.map(|info| info.location.clone()).unwrap_or_default(),
        self_ns,
        children,
        parents,
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
    use crate::{
        frame::HostView,
        panels::profiler::hierarchy::{
            HierarchySort, hierarchy_row_label, sort_hierarchy_rows, visible_hierarchy_rows,
        },
    };
    use egui::pos2;
    use kairos_editor_core::EditorState;
    use kairos_profile::{Profiler, ScopeKind};

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

    /// Capture one committed frame that lasts a few milliseconds, so the module
    /// clock axis has whole-millisecond ticks to label.
    fn slow_captured_profiler() -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "profiler.test.frame");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        assert!(
            !profiler.session().is_empty(),
            "the fixture captured one committed frame"
        );
        profiler
    }

    /// Capture one committed frame that lasts at least 100 ms. The usage chart's
    /// scale is capped at 70 ms, so any frame that long pins the guide to the slow
    /// band (66 / 33 / 16 ms) instead of letting the machine's speed pick one.
    fn long_frame_profiler() -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "profiler.test.frame");
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        assert!(
            !profiler.session().is_empty(),
            "the fixture captured one committed frame"
        );
        profiler
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

    /// A system sample's details name the system's full module path and point
    /// at the `add_systems` call site the executor recorded in its scope, not at
    /// the executor frame that ran it.
    #[test]
    fn a_system_block_reports_its_module_path_and_registration_site() {
        let scope = ScopeInfo {
            id: 9,
            name: SYSTEM_MARKER.to_owned(),
            location: "kairos_engine/src/schedule.rs:365".to_owned(),
            kind: ScopeKind::Named,
            category: Category::Scripts,
        };
        let frame = FrameSnapshot {
            index: 0,
            source_index: 0,
            start_ns: 0,
            end_ns: 1_000_000,
            threads: vec![ThreadSnapshot {
                id: 0,
                name: "main".to_owned(),
                samples: vec![SampleSnapshot {
                    scope: 9,
                    data: Some("Others/kairos_engine::schedule::run_main".to_owned()),
                    start_ns: 0,
                    duration_ns: 900_000,
                    depth: 0,
                    parent: None,
                }],
                warning: None,
            }],
            categories: [0; Category::COUNT],
        };
        let lanes = vec![Lane {
            id: Some(0),
            name: "main".to_owned(),
        }];
        let details = selection_details(
            &frame,
            &[scope],
            &lanes,
            Selection {
                frame: 0,
                lane: 0,
                sample: 0,
            },
        )
        .expect("the selection addresses a sample");

        assert_eq!(details.title, "kairos_engine::schedule::run_main");
        assert_eq!(details.category, Category::Others);
        assert_eq!(details.location, "kairos_engine/src/schedule.rs:365");
        let lines = details.body_lines();
        assert!(
            lines.contains(&"Location: kairos_engine/src/schedule.rs:365".to_owned()),
            "{lines:?}"
        );
    }

    /// Save writes the capture; the toolbar reports the result and the hover
    /// text carries the exact files, so the user can hand a capture over.
    #[test]
    fn the_toolbar_has_a_save_button_and_reports_the_written_files() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(Profiler::new());
        let mut panel = ProfilerPanel::default();
        panel.save = Some(Ok(CaptureFiles {
            stem: "capture-test".to_owned(),
            json: std::path::PathBuf::from("Library/Profiler/capture-test.json"),
            chrome_trace: std::path::PathBuf::from("Library/Profiler/capture-test.trace.json"),
            puffin: std::path::PathBuf::from("Library/Profiler/capture-test.puffin"),
        }));
        let output = draw_panel(&mut panel, &host);
        let text = painted_text(&output);
        assert!(
            text.iter().any(|line| line == "Save"),
            "the toolbar paints Save: {text:?}"
        );
        assert!(
            text.iter().any(|line| line == "Saved"),
            "the toolbar reports the save: {text:?}"
        );
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
        let _ = ctx.run_ui(input, |ui| {
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

    /// The module area: CPU/GPU cards, the pinned clock axis over the chart
    /// columns, the legend of the selected frame, a usage guide chip and the
    /// selected-frame cursor.
    #[test]
    fn module_area_draws_the_cpu_gpu_rows_and_history_chart() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(slow_captured_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);

        let text = painted_text(&output);
        assert!(text.iter().any(|line| line == "CPU Usage"), "{text:?}");
        assert!(text.iter().any(|line| line == "GPU Usage"), "{text:?}");
        assert!(
            text.iter().any(|line| line.contains("profiler.test.frame")),
            "the legend lists the selected frame's markers: {text:?}"
        );

        // The pinned clock axis labels real wall-clock times over the chart
        // columns, above the cards.
        let clock = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(shape)
                    if shape.galley.text().matches(':').count() == 2
                        && shape.galley.text().contains('.') =>
                {
                    Some(shape.pos)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("a clock tick label is painted: {text:?}"));
        assert!(
            clock.x >= panel.style.module_name_width,
            "the clock labels sit over the chart columns: {clock:?}"
        );
        let cpu_title = text_pos_starting_with(&output, "CPU Usage");
        assert!(
            clock.y < cpu_title.y,
            "the clock axis is pinned above the cards: {} vs {}",
            clock.y,
            cpu_title.y
        );
        assert!(
            text.iter().any(|line| line.contains("FPS")),
            "a usage guide chip is drawn: {text:?}"
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
        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let chart_left = panel.style.module_name_width;
        let chart_height = |output: &egui::FullOutput| {
            painted(output)
                .into_iter()
                .filter(|(rect, fill)| {
                    palette.contains(fill) && rect.height() > 5.0 && rect.left() >= chart_left
                })
                .map(|(rect, _)| rect.height())
                .fold(0.0_f32, f32::max)
        };
        let chart_before = chart_height(&first);
        assert!(chart_before > 0.0, "the CPU chart paints its tallest bar");
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
        // The chart columns zoom the module time axis, so the wheel over the
        // legend (the name column) is what scrolls the card stack.
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(egui::pos2(60.0, 60.0), egui::vec2(0.0, -4.0)),
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

    /// The module panes are windowed: a stack taller than the pane must not
    /// paint over the splitter or the transparent timeline header below it.
    #[test]
    fn the_module_cards_are_clipped_to_the_pane() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        // A pane shorter than the card stack, so the stack overflows it.
        panel.module_height = 120.0;
        let ctx = Context::default();
        let output = draw_pass(&ctx, &mut panel, &host, Vec::new());

        assert!(
            panel.module_content_height() > panel.module_height,
            "the card stack overflows the pane, so clipping is what keeps it in"
        );
        let pane_bottom =
            panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        let cards_bottom = pane_bottom - panel.module_bar_height();
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
            (clipped.bottom() - cards_bottom).abs() < 0.5,
            "the module cards are clipped above the pinned zoom bar: {clipped:?} vs {cards_bottom}"
        );
    }

    /// A pane shorter than the CPU card shrinks the card instead of clipping it:
    /// the usage chart keeps its zero line at the pane's bottom. The bars grow
    /// from that line, so a clipped chart shows nothing for the short bars a high
    /// frame rate produces, and reads as "CPU Usage recorded nothing".
    #[test]
    fn a_short_module_pane_keeps_the_usage_charts_zero_line() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_slow_frames(3));
        let mut panel = ProfilerPanel::default();
        // A pane the size the editor's default bottom dock leaves the profiler:
        // shorter than the CPU card's skin height.
        panel.module_height = 120.0;
        let ctx = Context::default();
        let output = draw_pass(&ctx, &mut panel, &host, Vec::new());

        let pane_top = panel.style.toolbar_height + panel.style.separator_width;
        let pane_bottom = pane_top + panel.module_height;
        let cards_top = pane_top + panel.module_axis_height() + panel.style.separator_width;
        let cards_bottom = pane_bottom - panel.module_bar_height();
        let chart_top = cards_top + panel.style.module_header_height;
        let viewport = cards_bottom - cards_top;
        assert!(
            viewport < panel.style.module_cpu_height,
            "the fixture's pane is shorter than the card's skin height: {viewport}"
        );

        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let chart_left = panel.style.module_name_width;
        let bars: Vec<Rect> = painted(&output)
            .into_iter()
            .filter(|(rect, fill)| {
                palette.contains(fill)
                    && rect.left() >= chart_left - 0.5
                    && rect.top() >= chart_top - 0.5
                    && rect.bottom() <= cards_bottom + 0.5
            })
            .map(|(rect, _)| rect)
            .collect();
        assert!(
            !bars.is_empty(),
            "the fitted chart still paints the frame's bar"
        );
        let baseline = bars.iter().map(|bar| bar.bottom()).fold(f32::MIN, f32::max);
        assert!(
            (baseline - cards_bottom).abs() < 0.5,
            "the chart's zero line sits on the pane's bottom: {baseline} vs {cards_bottom}"
        );
        let tallest = bars.iter().map(|bar| bar.height()).fold(0.0_f32, f32::max);
        assert!(
            tallest > 1.0,
            "the fitted chart scales the bar to its plot: {tallest}"
        );

        // The card fills the pane, so the GPU card is what a scroll reveals.
        let gpu = text_pos_starting_with(&output, "GPU Usage");
        assert!(
            gpu.y >= cards_bottom - 0.5,
            "the GPU card sits below the pane's bottom: {} vs {cards_bottom}",
            gpu.y
        );
    }

    /// A fresh panel's module pane is as tall as its cards ask for: the CPU card
    /// whole, the rule under it and half of the GPU card. The timeline takes the
    /// rest of the panel, so the default costs it nothing but the space the cards
    /// need.
    #[test]
    fn the_module_pane_defaults_to_the_cpu_card_and_half_the_next() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();
        let output = draw_pass(&ctx, &mut panel, &host, Vec::new());

        let expected = panel.module_chrome_height()
            + panel.style.module_cpu_height
            + panel.style.separator_width
            + 0.5 * panel.style.module_gpu_height;
        assert!(
            (panel.module_height - expected).abs() < 0.5,
            "the pane defaults to the CPU card and half the GPU card: {} vs {expected}",
            panel.module_height
        );
        assert!(
            panel.module_height < panel.module_content_height(),
            "the default shows part of the stack, not all of it"
        );

        // The CPU card is whole and the GPU card's top half is inside the pane.
        let pane_top = panel.style.toolbar_height + panel.style.separator_width;
        let cards_top = pane_top + panel.module_axis_height() + panel.style.separator_width;
        let cards_bottom = pane_top + panel.module_height - panel.module_bar_height();
        let cpu_bottom = cards_top + panel.style.module_cpu_height;
        let gpu_visible = cards_bottom - (cpu_bottom + panel.style.separator_width);
        assert!(
            (gpu_visible - 0.5 * panel.style.module_gpu_height).abs() < 1.0,
            "half of the GPU card is inside the pane: {gpu_visible}"
        );
        let cpu = text_pos_starting_with(&output, "CPU Usage");
        let gpu = text_pos_starting_with(&output, "GPU Usage");
        assert!(
            cpu.y > cards_top && cpu.y < cards_bottom,
            "the CPU card's title is inside the pane"
        );
        assert!(
            gpu.y > cards_bottom - panel.style.module_gpu_height,
            "the GPU card starts inside the pane"
        );
    }

    /// The timeline's minimum is its header row, so the splitter can collapse
    /// the band and hand the panel to the module pane — the way a capture with
    /// more modules than room needs.
    #[test]
    fn the_splitter_collapses_the_timeline_to_its_header() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();
        // A panel too short for the whole card stack beside the timeline: the
        // splitter keeps handing the module pane room until the timeline is down
        // to its header.
        let size = egui::vec2(900.0, 340.0);
        let _ = draw_pass_sized(&ctx, &mut panel, &host, size, Vec::new());
        let from = splitter_center(&panel);
        let total = size.y - panel.style.toolbar_height - panel.style.separator_width;
        let max_module = (total - panel.splitter().thickness() - panel.min_timeline_height())
            .min(panel.module_content_height());
        assert!(
            max_module < panel.module_content_height(),
            "the fixture's panel must be too short for the whole card stack"
        );
        assert!(
            panel.module_height < max_module,
            "the fixture must leave the splitter room to grow the module pane"
        );

        let _ = draw_pass_sized(&ctx, &mut panel, &host, size, press_events(from));
        let _ = draw_pass_sized(
            &ctx,
            &mut panel,
            &host,
            size,
            drag_events(from + egui::vec2(0.0, 400.0)),
        );
        let output = draw_pass_sized(&ctx, &mut panel, &host, size, Vec::new());
        assert!(
            (panel.module_height - max_module).abs() < 0.5,
            "the module pane takes the timeline's space: {} vs {max_module}",
            panel.module_height
        );
        assert!(
            (total
                - panel.splitter().thickness()
                - panel.module_height
                - panel.min_timeline_height())
            .abs()
                < 0.5,
            "the timeline is squeezed to its header"
        );
        let header = text_pos_starting_with(&output, "Timeline");
        let band_top = panel.style.toolbar_height
            + panel.style.separator_width
            + panel.module_height
            + panel.splitter().thickness();
        assert!(
            header.y >= band_top - 1.0 && header.y <= band_top + panel.style.toolbar_height,
            "the collapsed timeline still paints its header at {band_top}: {}",
            header.y
        );
        let _ = draw_pass_sized(
            &ctx,
            &mut panel,
            &host,
            size,
            release_events(from + egui::vec2(0.0, 400.0)),
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
    fn double_clicking_the_splitter_restores_the_default_height() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.module_height = 300.0;
        let ctx = Context::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let at = splitter_center(&panel);

        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        assert_eq!(
            panel.module_height, 300.0,
            "one click leaves the boundary alone"
        );
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        assert!(
            (panel.module_height - panel.module_default_height()).abs() < 0.5,
            "the double-click restores the cards' default height: {}",
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

    /// A short panel clamps the split: the module pane's default yields to the
    /// timeline's header minimum, and a panel shorter than both minimums still
    /// lays both bands out (the timeline gives way first, like Unity's flexed
    /// pane).
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
            "the default module pane leaves the timeline its header: {}",
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
        assert_eq!(details.category, Category::Others);
        assert_eq!(details.location, "");
        assert_eq!(details.self_ns, 4_000_000);
        assert_eq!(details.children, 0);
        // Unity omits the thread line when the marker ran once on that thread.
        assert_eq!(
            details.copy_text(),
            "profiler.test.worker\n4.00ms\nCategory: Others\n\
             Self: 4.00ms (no child samples recorded)\n\
             Current frame accumulated time:\n7.00ms for 3 instances over 2 threads"
        );
    }

    /// A wrapper block is a black box in the timeline: nothing was recorded
    /// inside it, so the details box is the only place that can say what it is,
    /// where its marker is opened and how much time no child sample covered.
    #[test]
    fn a_wrapper_block_reports_its_stage_location_and_unaccounted_time() {
        let scopes = vec![
            ScopeInfo {
                id: 1,
                name: "Kairos.Engine.Frame".to_owned(),
                location: "kairos_engine/src/engine.rs:98".to_owned(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 2,
                name: "Kairos.Stage".to_owned(),
                location: "kairos_engine/src/schedule.rs:224".to_owned(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
        ];
        let frame = FrameSnapshot {
            index: 0,
            source_index: 0,
            start_ns: 0,
            end_ns: 5_000_000,
            threads: vec![ThreadSnapshot {
                id: 0,
                name: "main".to_owned(),
                samples: vec![
                    SampleSnapshot {
                        scope: 1,
                        data: None,
                        start_ns: 0,
                        duration_ns: 5_000_000,
                        depth: 0,
                        parent: None,
                    },
                    SampleSnapshot {
                        scope: 2,
                        data: Some("Update".to_owned()),
                        start_ns: 0,
                        duration_ns: 4_000_000,
                        depth: 1,
                        parent: Some(0),
                    },
                ],
                warning: None,
            }],
            categories: [0; Category::COUNT],
        };
        let lanes = vec![Lane {
            id: Some(0),
            name: "main".to_owned(),
        }];
        let details = selection_details(
            &frame,
            &scopes,
            &lanes,
            Selection {
                frame: 0,
                lane: 0,
                sample: 1,
            },
        )
        .expect("the selection addresses a sample");

        assert_eq!(
            details.title, "Kairos.Stage[Update]",
            "the stage's data names the stage, like Unity's blocks"
        );
        assert_eq!(details.category, Category::Others);
        assert_eq!(details.location, "kairos_engine/src/schedule.rs:224");
        assert_eq!(
            details.self_ns, 4_000_000,
            "no child sample explains the block"
        );
        assert_eq!(details.children, 0);
        assert_eq!(
            details.parents,
            vec!["Kairos.Engine.Frame (5.00ms)".to_owned()]
        );
        let lines = details.body_lines();
        assert!(
            lines.contains(&"Self: 4.00ms (no child samples recorded)".to_owned()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"  Kairos.Engine.Frame (5.00ms)".to_owned()),
            "{lines:?}"
        );
        assert_eq!(
            details.copy_text().lines().next(),
            Some("Kairos.Stage[Update]")
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
            category: Category::Others,
            location: String::new(),
            self_ns: 1_000_000,
            children: 0,
            parents: Vec::new(),
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
            // The module area has a horizontal zoom bar of its own above the
            // timeline's; the timeline's is the lower one.
            .max_by(|a, b| a.0.top().total_cmp(&b.0.top()))
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
        let before = panel.timeline_view;
        draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(pointer, egui::vec2(0.0, 4.0)),
        );
        let after = panel.timeline_view;

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
        panel.timeline_view = TimelineView {
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
            panel.timeline_view.pan < 0.3,
            "middle-drag right shows earlier time: {:?}",
            panel.timeline_view
        );
    }

    /// Dragging the horizontal bar's thumb pans the shown window, like Unity's
    /// min-max scroll bar.
    #[test]
    fn dragging_the_horizontal_bar_pans_the_timeline() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.timeline_view = TimelineView {
            pan: 0.25,
            span: 0.5,
        };
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let thumb = horizontal_thumb(&first, panel.style.scrollbar_thumb.to_egui());
        let before = panel.timeline_view;
        let from = thumb.center();
        let to = from + egui::vec2(24.0, 0.0);

        draw_pass(&ctx, &mut panel, &host, press_events(from));
        draw_pass(&ctx, &mut panel, &host, drag_events(to));
        draw_pass(&ctx, &mut panel, &host, release_events(to));

        assert!(
            panel.timeline_view.pan > before.pan,
            "dragging the thumb right shows later time: {before:?} -> {:?}",
            panel.timeline_view
        );
        assert_eq!(
            panel.timeline_view.span, before.span,
            "panning does not zoom"
        );
    }

    /// Dragging an end of the horizontal bar's thumb resizes the window: that is
    /// how Unity's timeline zooms from its scroll bar.
    #[test]
    fn dragging_the_end_of_the_horizontal_bar_zooms_the_timeline() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.timeline_view = TimelineView {
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
            panel.timeline_view.span < 0.5,
            "dragging the right end in zooms: {:?}",
            panel.timeline_view
        );
        assert!(
            (panel.timeline_view.pan - 0.25).abs() < 1e-9,
            "the left edge stays put: {:?}",
            panel.timeline_view
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
        // The module pane has a vertical bar of its own above the splitter; the
        // timeline's is the one under it.
        let splitter_top =
            panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        let thumb = painted(&first)
            .into_iter()
            .filter(|(rect, fill)| {
                *fill == panel.style.scrollbar_thumb.to_egui()
                    && rect.width() < 20.0
                    && rect.height() > 20.0
                    && rect.top() > splitter_top
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
        panel.timeline_view = TimelineView {
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

        assert_eq!(panel.timeline_view, TimelineView::default());
    }

    /// A zoomed window cuts blocks at the track's edges instead of spilling over
    /// the thread names or the scroll bar.
    #[test]
    fn zoomed_blocks_are_clipped_to_the_track_column() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(repeated_marker_profiler());
        let mut panel = ProfilerPanel::default();
        panel.timeline_view = TimelineView {
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
        panel.timeline_view = TimelineView {
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

    /// Capture `count` committed frames as fast as the host can flush them.
    fn captured_frames(count: usize) -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        for _ in 0..count {
            {
                kairos_profile::scope!(Category::Others, "profiler.test.frame");
            }
            profiler.end_frame(true);
            profiler.begin_frame();
        }
        profiler
    }

    /// Capture `count` frames that last about a millisecond each, so they have
    /// distinct wall times on the module axis.
    fn captured_slow_frames(count: usize) -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        for _ in 0..count {
            {
                kairos_profile::scope!(Category::Others, "profiler.test.frame");
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            profiler.end_frame(true);
            profiler.begin_frame();
        }
        profiler
    }

    /// Capture two takes with a pause between them: two recording segments, so
    /// the module axis has one break band to draw.
    fn two_take_profiler() -> Profiler {
        let profiler = Profiler::new();
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "profiler.test.frame");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        profiler.set_recording(false);
        std::thread::sleep(std::time::Duration::from_millis(2));
        profiler.set_recording(true);
        profiler.begin_frame();
        {
            kairos_profile::scope!(Category::Others, "profiler.test.frame");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        profiler.end_frame(true);
        profiler.begin_frame();
        profiler
    }

    /// The module area's horizontal zoom bar thumb (the timeline's own bar is
    /// below the module band).
    fn module_thumb(output: &egui::FullOutput, panel: &ProfilerPanel) -> Rect {
        let module_bottom =
            panel.style.toolbar_height + panel.style.separator_width + panel.module_height;
        let thumb_fills = [
            panel.style.scrollbar_thumb.to_egui(),
            panel.style.scrollbar_thumb_hover.to_egui(),
            panel.style.scrollbar_thumb_active.to_egui(),
        ];
        painted(output)
            .into_iter()
            .filter(|(rect, fill)| {
                thumb_fills.contains(fill) && rect.width() > 20.0 && rect.top() < module_bottom
            })
            .max_by(|a, b| a.0.width().total_cmp(&b.0.width()))
            .map(|(rect, _)| rect)
            .expect("the module zoom bar's thumb is painted")
    }

    /// Top of the chart's plot (below the pinned clock axis and the CPU card's
    /// title row) and its bottom, which is where the bars start.
    fn module_chart_top(panel: &ProfilerPanel) -> (f32, f32) {
        let cards_top = panel.style.toolbar_height
            + panel.style.separator_width
            + panel.module_axis_height()
            + panel.style.separator_width;
        let viewport = (panel.module_height - panel.module_chrome_height()).max(0.0);
        let cpu_height = panel.module_cpu_fit(viewport);
        (
            cards_top + panel.style.module_header_height,
            cards_top + cpu_height,
        )
    }

    /// The module axis places its default window on the most recent frames: the
    /// split view opens on the tail of a long capture, and on all of a short one.
    #[test]
    fn the_module_axis_defaults_to_the_last_frames_window() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();

        let host = ProfilerHost(captured_frames(320));
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let session = host.0.session();
        let count = session.frames.len();
        assert!(count > 300, "the fixture captured a long capture: {count}");

        let window = panel.module_axis.window();
        let first_shown = session.frames[count - 300].start_ns as f64;
        assert!(
            (window.value - first_shown).abs() < 1.0,
            "the default window starts at the 300th frame from the end: {} vs {first_shown}",
            window.value
        );
        assert!(
            (window.value + window.span - window.bounds.1).abs() < 1.0,
            "the default window is pinned to the newest data: {window:?}"
        );
        let oldest = panel
            .module_axis
            .screen_of_source(session.frames[0].start_ns as f64)
            .expect("frame 0 maps onto the axis");
        assert!(oldest < 0.0, "the first frames are to the left: {oldest}");
        drop(host);

        // A capture shorter than the window fits entirely.
        let host = ProfilerHost(captured_frames(3));
        let mut small = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut small, &host, Vec::new());
        assert!(small.module_axis.window().value.abs() < 1e-9);
    }

    /// The module area's min-max bar: dragging an end zooms the clock axis (Live
    /// keeps the newest frame on the right), dragging the thumb pans it.
    #[test]
    fn the_module_zoom_bar_zooms_and_pans_the_axis() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_slow_frames(30));
        let mut panel = ProfilerPanel::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let before = panel.module_axis.window();
        let thumb = module_thumb(&first, &panel);
        let from = egui::pos2(thumb.right() - 3.0, thumb.center().y);
        let to = from - egui::vec2(120.0, 0.0);
        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(&ctx, &mut panel, &host, drag_events(to));
        let _ = draw_pass(&ctx, &mut panel, &host, release_events(to));
        let zoomed = panel.module_axis.window();
        assert!(
            zoomed.span < before.span * 0.9,
            "dragging the right end in zooms the axis: {before:?} -> {zoomed:?}"
        );
        assert!(
            !panel.module_default_window,
            "manual zoom leaves the default window"
        );
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let live = panel.module_axis.window();
        assert!(
            (live.value + live.span - live.bounds.1).abs() < 1.0,
            "Live keeps the newest frame on the right edge: {live:?}"
        );

        let pan_output = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let thumb = module_thumb(&pan_output, &panel);
        let from = thumb.center();
        let to = from - egui::vec2(40.0, 0.0);
        let _ = draw_pass(&ctx, &mut panel, &host, press_events(from));
        let _ = draw_pass(&ctx, &mut panel, &host, drag_events(to));
        let _ = draw_pass(&ctx, &mut panel, &host, release_events(to));
        let panned = panel.module_axis.window();
        assert!(
            panned.value + 1.0 < live.value,
            "dragging the thumb left pans towards earlier frames: {live:?} -> {panned:?}"
        );
        assert!(
            (panned.span - live.span).abs() < 1.0,
            "panning does not zoom"
        );
        assert!(
            !panel.follow_latest,
            "panning stops following the newest frame"
        );
    }

    /// The wheel over the chart columns zooms the clock axis around the pointer:
    /// the time under the pointer stays put, like the timeline's zoom.
    #[test]
    fn the_module_wheel_zooms_the_clock_axis_around_the_pointer() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_slow_frames(40));
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());

        let chart_left = panel.style.module_name_width;
        let x = 300.0;
        let before = panel
            .module_axis
            .source_at_screen(x)
            .expect("the pointer is over a frame");
        let zoom_before = panel.module_axis.zoom();
        let pointer = egui::pos2(chart_left + x, 100.0);
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(pointer, egui::vec2(0.0, 4.0)),
        );

        assert!(
            panel.module_axis.zoom() > zoom_before,
            "the wheel zooms the clock axis"
        );
        let after = panel
            .module_axis
            .source_at_screen(x)
            .expect("the pointer is still over a frame");
        assert_eq!(before.0, after.0, "the same take stays under the pointer");
        assert!(
            (before.1 - after.1).abs() < 1.0,
            "the time under the pointer stays: {} -> {}",
            before.1,
            after.1
        );
        assert!(!panel.module_default_window);
    }

    /// A pause between two takes is a break band, with the stop clock on its left
    /// and the restart clock on its right.
    #[test]
    fn a_pause_is_a_break_band_with_its_clock_times() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(two_take_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let session = host.0.session();

        assert_eq!(session.segments.len(), 2, "two takes were recorded");
        let breaks = panel.module_axis.breaks();
        assert_eq!(breaks.len(), 1, "the pause is one break band");
        let wall = session
            .wall_clock
            .expect("the live capture has a wall clock");
        let stop = super::axis::clock_label(
            wall.time_of_day_ns as f64 + session.segments[0].end_ns.unwrap() as f64,
            true,
        );
        let start = super::axis::clock_label(
            wall.time_of_day_ns as f64 + session.segments[1].start_ns as f64,
            true,
        );
        assert_eq!(breaks[0].stop, session.segments[0].end_ns.unwrap() as f64);
        assert_eq!(breaks[0].restart, session.segments[1].start_ns as f64);

        let band = panel.style.break_bg.to_egui();
        let bands: Vec<Rect> = painted(&output)
            .into_iter()
            .filter(|(_, fill)| *fill == band)
            .map(|(rect, _)| rect)
            .collect();
        assert!(
            bands
                .iter()
                .any(|rect| (rect.height() - panel.style.ruler_height).abs() < 0.5),
            "the band shows on the clock row: {bands:?}"
        );
        assert!(
            bands.iter().any(|rect| rect.height() > 50.0),
            "the band runs through the chart: {bands:?}"
        );

        let text = painted_text(&output);
        assert!(
            text.iter().any(|line| line == &stop) && text.iter().any(|line| line == &start),
            "the band's two sides print their clock times: {text:?}"
        );
    }

    /// Clicking a chart column selects the frame recorded at that time.
    #[test]
    fn clicking_a_chart_column_selects_the_frame_at_that_time() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_slow_frames(3));
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let session = host.0.session();
        assert!(session.frames.len() >= 3);

        let chart_left = panel.style.module_name_width;
        let (chart_top, _) = module_chart_top(&panel);
        let x = panel
            .module_axis
            .screen_of_source(session.frames[1].start_ns as f64)
            .expect("the second frame maps onto the axis");
        let at = egui::pos2(chart_left + x + 1.0, chart_top + 30.0);
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));

        assert_eq!(
            panel.selected_frame, 1,
            "the click selected the second frame"
        );
        assert!(!panel.follow_latest, "clicking a frame leaves Live");
    }

    /// A click on a break band is a click on the pause, not on a frame.
    #[test]
    fn clicking_a_break_band_selects_nothing() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(two_take_profiler());
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let before = panel.selected_frame;
        let breaks = panel.module_axis.breaks();
        let band = breaks[0].x + breaks[0].width * 0.5;

        let chart_left = panel.style.module_name_width;
        let (chart_top, _) = module_chart_top(&panel);
        let at = egui::pos2(chart_left + band, chart_top + 30.0);
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        assert_eq!(panel.selected_frame, before, "the pause holds no frame");
    }

    /// The tallest bar fills 80% of the chart, and the ms/FPS guide line is
    /// placed by that same scale.
    #[test]
    fn the_tallest_bar_fills_eighty_percent_of_the_chart() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(slow_captured_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let session = host.0.session();
        let frame = session.frames.last().expect("one frame");

        let chart_left = panel.style.module_name_width;
        let (chart_top, plot_bottom) = module_chart_top(&panel);
        let plot_height = plot_bottom - chart_top;
        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let mut columns: BTreeMap<i32, f32> = BTreeMap::new();
        let mut bar_left = f32::MAX;
        for (rect, fill) in painted(&output) {
            if !palette.contains(&fill)
                || rect.left() < chart_left - 0.5
                || rect.top() < chart_top - 0.5
                || rect.bottom() > plot_bottom + 0.5
                || rect.height() <= 0.0
            {
                continue;
            }
            *columns
                .entry((rect.left() * 4.0).round() as i32)
                .or_default() += rect.height();
            bar_left = bar_left.min(rect.left());
        }
        let tallest = columns.values().copied().fold(0.0_f32, f32::max);
        assert!(tallest > 0.0, "the chart paints a bar");
        let fill = tallest / plot_height;
        assert!(
            (fill - 0.8).abs() < 0.01,
            "the tallest bar fills 80% of the chart: {tallest} / {plot_height} = {fill}"
        );

        // The bar starts at the frame's time on the clock axis.
        let expected_left = chart_left
            + panel
                .module_axis
                .screen_of_source(frame.start_ns as f64)
                .expect("the frame maps onto the axis");
        assert!(
            (bar_left - expected_left).abs() < 1.0,
            "the bar lines up with the clock axis: {bar_left} vs {expected_left}"
        );

        // The guide chips are the tier for this scale; the engine's 960 FPS
        // reading labels the 10 ms band.
        let scale = frame.duration_ns() as f32 / 1_000_000.0 / 0.8;
        let text = painted_text(&output);
        let expected_labels: Vec<&str> = usage_grid(scale)
            .iter()
            .filter(|(ms, _)| *ms <= f64::from(scale))
            .map(|(_, label)| *label)
            .collect();
        assert!(
            expected_labels
                .iter()
                .any(|label| text.iter().any(|line| line.contains(label))),
            "the chart labels its guide for this scale ({scale} ms): {text:?}"
        );
        if scale < 10.0 {
            assert!(
                text.iter().any(|line| line.contains("960FPS")),
                "the 10 ms tier uses the engine's 960 FPS reading: {text:?}"
            );
        }

        // The smallest guide is drawn at the scale's 80%-based position.
        let expected_y = plot_bottom - (1000.0 / 960.0 / scale) * plot_height;
        let grid = panel.style.chart_grid.to_egui();
        let lines: Vec<Rect> = painted(&output)
            .into_iter()
            .filter(|(rect, fill)| {
                *fill == grid
                    && rect.height() <= 1.0
                    && rect.width() > 400.0
                    && rect.top() > chart_top
                    && rect.top() < plot_bottom
            })
            .map(|(rect, _)| rect)
            .collect();
        assert!(
            lines
                .iter()
                .any(|rect| (rect.top() - expected_y).abs() < 1.0),
            "the 960 FPS guide sits at {expected_y}: {lines:?}"
        );
    }

    /// A short chart bunches the guide tiers up: a chip that would overlap the
    /// one above it steps aside, and its line stays so the scale still reads.
    #[test]
    fn a_short_chart_drops_the_guide_chips_that_collide() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(long_frame_profiler());
        let mut panel = ProfilerPanel::default();

        let tall = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let session = host.0.session();
        let frame = session.frames.last().expect("one frame");
        let scale = (frame.duration_ns() as f32 / 1_000_000.0 / USAGE_BAR_FILL)
            .clamp(MIN_USAGE_SCALE_MS, MAX_USAGE_SCALE_MS);
        let tiers: Vec<&'static str> = usage_grid(scale)
            .iter()
            .filter(|(ms, _)| *ms <= f64::from(scale))
            .map(|(_, label)| *label)
            .collect();
        assert_eq!(tiers.len(), 3, "the fixture's scale holds every tier");
        let chips = |output: &egui::FullOutput| -> usize {
            painted_text(output)
                .iter()
                .filter(|line| tiers.contains(&line.as_str()))
                .count()
        };
        let grid = panel.style.chart_grid.to_egui();
        let lines = |output: &egui::FullOutput| -> usize {
            painted(output)
                .into_iter()
                .filter(|(rect, fill)| {
                    *fill == grid && rect.height() <= 1.0 && rect.width() > 400.0
                })
                .count()
        };
        let tall_chips = chips(&tall);
        assert_eq!(
            tall_chips,
            tiers.len(),
            "the tall chart labels each tier apart: {tall_chips}"
        );
        assert_eq!(lines(&tall), tiers.len(), "the tall chart draws each line");

        // A pane the default editor dock leaves the profiler: the CPU card is
        // fitted to it, so the chart is too short to space the chips out.
        panel.module_height = 100.0;
        let short = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let short_chips = chips(&short);
        assert!(
            short_chips < tall_chips,
            "a short chart drops the chips that would collide: {short_chips} of {tall_chips}"
        );
        assert_eq!(
            lines(&short),
            tiers.len(),
            "every tier keeps its line when a chip steps aside"
        );
    }

    /// The usage guide tiers are Unity's four bands, with the engine's refresh
    /// targets in the two faster ones.
    #[test]
    fn the_usage_grid_uses_the_refresh_rate_tiers() {
        let labels = |scale: f32| -> Vec<&'static str> {
            usage_grid(scale).iter().map(|(_, label)| *label).collect()
        };
        assert_eq!(
            labels(1.0),
            vec!["1ms (1000FPS)", "0.25ms (4000FPS)", "0.1ms (10000FPS)"]
        );
        assert_eq!(
            labels(4.0),
            vec!["8ms (120FPS)", "4ms (240FPS)", "1ms (960FPS)"]
        );
        assert_eq!(
            labels(20.0),
            vec!["16ms (60FPS)", "8ms (120FPS)", "4ms (240FPS)"]
        );
        assert_eq!(
            labels(50.0),
            vec!["66ms (15FPS)", "33ms (30FPS)", "16ms (60FPS)"]
        );
        assert!((usage_grid(20.0)[0].0 - 1000.0 / 60.0).abs() < 1e-9);
        assert!((usage_grid(4.0)[0].0 - 1000.0 / 120.0).abs() < 1e-9);
        assert!((usage_grid(4.0)[1].0 - 1000.0 / 240.0).abs() < 1e-9);
        assert!((usage_grid(4.0)[2].0 - 1000.0 / 960.0).abs() < 1e-9);
    }

    /// A double-click on the clock ruler restores the default last-frames window.
    #[test]
    fn double_clicking_the_clock_ruler_restores_the_default_window() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_frames(320));
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());

        let chart_left = panel.style.module_name_width;
        let _ = draw_pass(
            &ctx,
            &mut panel,
            &host,
            wheel_events(egui::pos2(chart_left + 300.0, 100.0), egui::vec2(0.0, 20.0)),
        );
        assert!(!panel.module_default_window, "the wheel zoomed the axis");

        let ruler_y = panel.style.toolbar_height
            + panel.style.separator_width
            + panel.module_axis_height() * 0.5;
        let at = egui::pos2(chart_left + 300.0, ruler_y);
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(at));

        assert!(
            panel.module_default_window,
            "the double-click restored the window"
        );
        let session = host.0.session();
        let count = session.frames.len();
        let restored = panel.module_axis.window();
        let first_shown = session.frames[count - 300].start_ns as f64;
        assert!(
            (restored.value - first_shown).abs() < 1.0,
            "the restored window shows the last frames again: {} vs {first_shown}",
            restored.value
        );
    }

    /// Stepping back to a frame the module window has scrolled past pans the
    /// axis to it, so the transport reaches the first recorded frame.
    #[test]
    fn stepping_to_an_older_frame_reveals_it_on_the_axis() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_frames(320));
        let mut panel = ProfilerPanel::default();
        let toolbar = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let session = host.0.session();
        assert!(session.frames.len() > 300);

        // Frame 0 is left of the default window; pin the selection to frame 1 so
        // Previous is enabled, then step back. The record toggle's state fill is
        // how the toolbar's segment geometry is read back from the painting.
        panel.selected_frame = 1;
        panel.follow_latest = false;
        let record = painted(&toolbar)
            .into_iter()
            .find(|(rect, fill)| {
                *fill == panel.style.recording_bg.to_egui()
                    && rect.top() < panel.style.toolbar_height
            })
            .map(|(rect, _)| rect)
            .expect("the recording toggle paints its state fill");
        let previous = egui::pos2(
            record.right() + panel.style.separator_width + record.width() * 0.5,
            record.center().y,
        );
        let _ = draw_pass(&ctx, &mut panel, &host, click_events(previous));

        assert_eq!(panel.selected_frame, 0);
        let x = panel
            .module_axis
            .screen_of_source(session.frames[0].start_ns as f64)
            .expect("frame 0 maps onto the axis");
        assert!(x >= 0.0, "the axis revealed the frame: {x}");
        let chart_width = 900.0 - panel.style.module_name_width - panel.style.scrollbar_width;
        assert!(
            x >= 0.0 && x < 5.0,
            "the first frame is revealed at the start of the axis: {x}"
        );
        assert!(
            x <= chart_width,
            "the revealed frame is inside the chart columns"
        );
    }

    /// The selected-frame cursor runs from the clock row down through the cards
    /// and rides the middle of the frame's bar.
    #[test]
    fn the_cursor_spans_the_clock_and_sits_on_the_bar_centre() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(slow_captured_profiler());
        let mut panel = ProfilerPanel::default();
        let output = draw_panel(&mut panel, &host);
        let session = host.0.session();
        let frame = session.frames.last().expect("one frame");

        let cursor = panel.style.chart_cursor.to_egui();
        let cursor_rect = painted(&output)
            .into_iter()
            .find(|(rect, fill)| *fill == cursor && rect.height() > 20.0)
            .map(|(rect, _)| rect)
            .expect("the selected-frame cursor is painted");
        let chart_left = panel.style.module_name_width;
        let (chart_top, plot_bottom) = module_chart_top(&panel);
        let axis_top = panel.style.toolbar_height + panel.style.separator_width;
        assert!(
            cursor_rect.top() <= axis_top + panel.module_axis_height() * 0.5,
            "the cursor reaches into the pinned clock row: {cursor_rect:?}"
        );
        assert!(
            cursor_rect.bottom() >= plot_bottom - 1.0,
            "the cursor runs through the whole chart: {cursor_rect:?} vs {plot_bottom}"
        );

        // The clock reading of the frame's middle is where the line sits.
        let mid = frame.start_ns as f64 + (frame.end_ns - frame.start_ns) as f64 * 0.5;
        let expected = chart_left
            + panel
                .module_axis
                .screen_of_source(mid)
                .expect("the frame maps onto the axis");
        assert!(
            (cursor_rect.center().x - expected).abs() < 1.0,
            "the cursor rides the frame's middle: {} vs {expected}",
            cursor_rect.center().x
        );

        // And that is the middle of the painted bar, not its left edge.
        let palette: Vec<egui::Color32> = panel
            .style
            .category_palette
            .iter()
            .map(|color| color.to_egui())
            .collect();
        let bar = painted(&output)
            .into_iter()
            .find(|(rect, fill)| {
                palette.contains(fill)
                    && rect.height() > 20.0
                    && rect.left() >= chart_left
                    && rect.top() >= chart_top
                    && rect.bottom() <= plot_bottom + 0.5
            })
            .map(|(rect, _)| rect)
            .expect("the chart paints the frame's bar");
        let bar_centre = bar.left() + bar.width() * 0.5;
        assert!(
            (cursor_rect.center().x - bar_centre).abs() < 1.0,
            "the cursor sits on the bar's centre: {} vs {bar_centre}",
            cursor_rect.center().x
        );
    }

    /// Dragging the cursor (or anywhere on the chart) scrubs the selected frame.
    #[test]
    fn dragging_the_cursor_scrubs_the_selected_frame() {
        let _guard = lock_profiler_tests();
        let ctx = Context::default();
        let host = ProfilerHost(captured_slow_frames(5));
        let mut panel = ProfilerPanel::default();
        let _ = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let session = host.0.session();
        assert!(session.frames.len() >= 5, "five frames captured");
        let selected_before = panel.selected_frame;
        assert!(selected_before > 1, "the newest frame starts selected");

        let mid = |frame: &FrameSnapshot| {
            frame.start_ns as f64 + (frame.end_ns - frame.start_ns) as f64 * 0.5
        };
        let chart_left = panel.style.module_name_width;
        let (chart_top, _) = module_chart_top(&panel);
        let start_x = panel
            .module_axis
            .screen_of_source(mid(&session.frames[selected_before]))
            .expect("the selected frame maps onto the axis");
        let target_x = panel
            .module_axis
            .screen_of_source(mid(&session.frames[1]))
            .expect("the second frame maps onto the axis");

        let start = egui::pos2(chart_left + start_x, chart_top + 30.0);
        let target = egui::pos2(chart_left + target_x, chart_top + 30.0);
        let _ = draw_pass(&ctx, &mut panel, &host, press_events(start));
        let _ = draw_pass(&ctx, &mut panel, &host, drag_events(target));
        let _ = draw_pass(&ctx, &mut panel, &host, release_events(target));

        assert_eq!(
            panel.selected_frame, 1,
            "the drag selected the frame under the pointer"
        );
        assert!(!panel.follow_latest, "scrubbing leaves Live");
    }

    /// Sibling lists re-sort, parents stay above their children, and the
    /// direction toggle reverses every sibling list rather than the flatten.
    #[test]
    fn hierarchy_sorting_keeps_parents_above_children() {
        let scopes = vec![
            ScopeInfo {
                id: 1,
                name: "root.a".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 2,
                name: "child.a1".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 3,
                name: "child.a2".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 4,
                name: "root.b".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
        ];
        let row = |scope: u32, depth: u32, total: u64, self_ns: u64| HierarchyRow {
            scope,
            data: None,
            depth,
            total_ns: total,
            self_ns,
            calls: 1,
            max_ns: total,
        };
        let mut rows = vec![
            row(1, 0, 10, 1),
            row(3, 1, 4, 4),
            row(2, 1, 6, 6),
            row(4, 0, 20, 20),
        ];

        sort_hierarchy_rows(&mut rows, HierarchySort::Total, true, &scopes);
        assert_eq!(
            rows.iter().map(|row| row.scope).collect::<Vec<_>>(),
            vec![4, 1, 2, 3],
            "roots by total descending, A's children by total descending"
        );
        assert_eq!(
            rows.iter().map(|row| row.depth).collect::<Vec<_>>(),
            vec![0, 0, 1, 1],
            "depth tags still address the tree"
        );

        sort_hierarchy_rows(&mut rows, HierarchySort::Total, false, &scopes);
        assert_eq!(
            rows.iter().map(|row| row.scope).collect::<Vec<_>>(),
            vec![1, 3, 2, 4],
            "ascending reverses every sibling list, not the flattening"
        );

        sort_hierarchy_rows(&mut rows, HierarchySort::Name, true, &scopes);
        assert_eq!(
            rows.iter().map(|row| row.scope).collect::<Vec<_>>(),
            vec![1, 2, 3, 4],
            "name ascending: root.a with its children, then root.b"
        );
    }

    /// A collapsed row keeps its own row and hides only its subtree.
    #[test]
    fn a_collapsed_hierarchy_row_hides_only_its_subtree() {
        let scopes = [
            ScopeInfo {
                id: 1,
                name: "root".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 2,
                name: "branch".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
            ScopeInfo {
                id: 3,
                name: "leaf".to_owned(),
                location: String::new(),
                kind: ScopeKind::Named,
                category: Category::Others,
            },
        ];
        let row = |scope: u32, depth: u32| HierarchyRow {
            scope,
            data: None,
            depth,
            total_ns: 1,
            self_ns: 1,
            calls: 1,
            max_ns: 1,
        };
        let rows = vec![row(1, 0), row(2, 1), row(3, 2), row(2, 1), row(1, 0)];

        let visible = visible_hierarchy_rows(&rows, &BTreeSet::new());
        assert_eq!(visible.len(), 5);
        assert!(visible[0].has_children, "the root has a child row");
        assert!(!visible[2].has_children, "the leaf is the deepest row");

        let branch_key = "1:/2:";
        let collapsed = BTreeSet::from([branch_key.to_owned()]);
        let visible = visible_hierarchy_rows(&rows, &collapsed);
        assert_eq!(
            visible.iter().map(|entry| entry.index).collect::<Vec<_>>(),
            vec![0, 1, 3, 4],
            "the branch stays, the leaf under it hides, the rest stays"
        );
        assert!(visible[1].has_children);
        assert!(!visible[2].has_children);
        assert_eq!(hierarchy_row_label(&rows[0], &scopes), "root");
    }

    /// The Hierarchy view is the whole-frame ranking: headers, rows and the
    /// active view in the selector.
    #[test]
    fn the_hierarchy_view_paints_the_ranked_tree() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(slow_captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view_kind = PanelView::Hierarchy;
        let output = draw_panel(&mut panel, &host);
        let text = painted_text(&output);
        for header in ["Total", "Self", "Calls", "Max"] {
            assert!(
                text.iter().any(|line| line == header),
                "the {header} column header paints: {text:?}"
            );
        }
        assert!(
            text.iter().any(|line| line == "profiler.test.frame"),
            "the aggregated row paints: {text:?}"
        );
        assert!(
            text.iter().any(|line| line.ends_with("ms")),
            "the row's time paints: {text:?}"
        );
        assert_eq!(panel.view_kind.label(), "Hierarchy");
    }

    /// The Hierarchy view must survive a panel narrower than its numeric
    /// columns; a clamp over crossed bounds would panic there.
    #[test]
    fn the_hierarchy_view_survives_a_narrow_panel() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view_kind = PanelView::Hierarchy;
        let ctx = Context::default();
        let output = draw_pass_sized(
            &ctx,
            &mut panel,
            &host,
            egui::vec2(200.0, 180.0),
            Vec::new(),
        );
        assert!(
            !painted_text(&output).is_empty(),
            "the narrow view still paints its chrome"
        );
    }

    /// Clicking a column header sorts by it; clicking the active column flips
    /// the direction.
    #[test]
    fn clicking_a_hierarchy_column_sorts_it() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view_kind = PanelView::Hierarchy;
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let calls = text_center(&first, "Calls");
        draw_pass(&ctx, &mut panel, &host, click_events(calls));
        assert_eq!(panel.hierarchy_sort, HierarchySort::Calls);
        assert!(panel.hierarchy_descending, "a new column starts best-first");

        let second = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let calls = text_center(&second, "Calls");
        draw_pass(&ctx, &mut panel, &host, click_events(calls));
        assert!(
            !panel.hierarchy_descending,
            "clicking the active column flips the direction"
        );
    }

    /// Clicking a row with children collapses its subtree.
    #[test]
    fn clicking_a_hierarchy_row_collapses_its_subtree() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        panel.view_kind = PanelView::Hierarchy;
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let row = text_center(&first, "profiler.test.frame");
        draw_pass(&ctx, &mut panel, &host, click_events(row));
        assert!(
            !panel.collapsed.is_empty(),
            "the row with a child row collapses"
        );
    }

    /// The header's view selector opens Unity's menu and switches the band to
    /// the Hierarchy view.
    #[test]
    fn the_view_selector_switches_to_the_hierarchy_view() {
        let _guard = lock_profiler_tests();
        let host = ProfilerHost(captured_profiler());
        let mut panel = ProfilerPanel::default();
        let ctx = Context::default();

        let first = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let selector = text_center(&first, "Timeline");
        draw_pass(&ctx, &mut panel, &host, click_events(selector));

        let second = draw_pass(&ctx, &mut panel, &host, Vec::new());
        let hierarchy_item = text_center(&second, "Hierarchy");
        draw_pass(&ctx, &mut panel, &host, click_events(hierarchy_item));
        assert_eq!(panel.view_kind, PanelView::Hierarchy);
    }
}
