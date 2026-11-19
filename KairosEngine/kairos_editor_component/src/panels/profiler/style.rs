//! Profiler panel style: the measured Unity/Tuanjie Profiler appearance.
//!
//! The values are code defaults for [`ProfilerStyle`], persisted per component in
//! `Preferences/Styles/Profiler.ron` (dark + light). The panel loads the file in
//! `Panel::init`; editing it restyles the Profiler without a rebuild.
//!
//! Layout is Unity's: a 20 px toolbar, a module pane of module cards (CPU Usage,
//! GPU Usage) over a vertical scroll bar, the splitter that resizes the two
//! bands, then the timeline (header, ruler, thread tracks and the two scroll
//! bars). The tokens are grouped the same way so a skin edit reads like the
//! panel; the pane's default height is derived from the card heights rather than
//! kept as a token of its own.

use kairos_color::Color;
use kairos_editor_widgets::style::Style;
use serde::{Deserialize, Serialize};

use crate::skin::StyleFields;

/// Appearance tokens for the Profiler panel: colours, toolbar/transport icons,
/// module-area and timeline geometry.
#[derive(Clone, Serialize, Deserialize, StyleFields)]
pub struct ProfilerStyle {
    pub icons: ProfilerIconPaths,

    // -- Chrome shared by every section -------------------------------------
    /// 1 px rules: under the toolbar, between toolbar segments, between rows.
    pub separator: Color,
    pub separator_width: f32,

    // -- Toolbar ------------------------------------------------------------
    /// Toolbar row height (Unity: 20 px). The record and transport toggles are
    /// squares of this size, and the vertical rules are as tall as the row.
    pub toolbar_height: f32,
    /// Left/right padding of a toolbar text segment.
    pub toolbar_item_padding: f32,
    /// Side of a toolbar toggle's icon (record, previous/next/current frame).
    /// Unity ships a 16 pt slot; the files are the @2x 32 px artwork.
    pub toolbar_icon_size: f32,
    /// Toolbar label colour and size (Unity dark skin: #C4C4C4 at 12 px).
    pub toolbar_text: Color,
    pub toolbar_text_size: f32,
    /// Fill of a toolbar button under the pointer.
    pub hover_bg: Color,
    pub click_bg: Color,
    /// Fill of a toggle that is on (recording).
    pub recording_bg: Color,
    /// Fill of a toggle that is on (the timeline's Live).
    pub selected_bg: Color,

    // -- Text shared by the module and timeline areas -----------------------
    pub timeline_text: Color,
    pub ruler_text: Color,
    /// Small captions: chips, sample labels, ruler ticks.
    pub small_text_size: f32,
    /// Warning glyph next to a module that cannot report (GPU).
    pub warning: Color,

    // -- Module area --------------------------------------------------------
    /// Preferred height of the CPU Usage card (title row + frame-history chart).
    /// The card keeps this height while the pane can show it, so dragging the
    /// pane down reveals the modules below instead of stretching this chart; a
    /// shorter pane shrinks the card to fit rather than clipping its chart at
    /// the pane's edge (the bars grow from the chart's bottom).
    pub module_cpu_height: f32,
    /// Height of the GPU Usage card (title row + empty body until the capture
    /// records GPU timings). It keeps this height below the CPU card, so a pane
    /// too short for both scrolls down to it.
    pub module_gpu_height: f32,
    /// Height of one module's title row.
    pub module_header_height: f32,
    /// Box of a module's badge, the CPU/GPU mark in its title row, and the
    /// inset/gap that place it. The badge's centre lines up with the legend
    /// swatches below it and the title with the legend labels, so a card reads
    /// as one column. The artwork is the @2x file for this box, and both marks
    /// are drawn to the same optical size inside it — the chip fills the box,
    /// the display is as wide and shorter by its stand — so CPU and GPU read as
    /// a pair instead of one icon looking smaller than the other.
    pub module_badge_width: f32,
    pub module_badge_height: f32,
    pub module_badge_inset: f32,
    pub module_badge_gap: f32,
    /// Width of the module name/legend column.
    pub module_name_width: f32,
    /// Height of one legend row.
    pub legend_row_height: f32,
    /// How much of the usage chart's height the tallest visible bar fills.
    pub usage_bar_fill: f32,
    /// How many frames the module area's time axis shows by default (the most
    /// recent ones); the wheel or the zoom bar widens the window to the whole
    /// capture.
    pub window_fram_count: usize,
    /// Fill of the frame-history bars.
    pub chart_bar: Color,
    /// Horizontal guide line inside the usage chart (Unity's ms/FPS tiers) and
    /// the faint vertical grid under the clock ticks.
    pub chart_grid: Color,
    /// Vertical marker at the selected frame.
    pub chart_cursor: Color,
    /// Fill of a break band: the compressed pause between two takes on the
    /// module area's time axis.
    pub break_bg: Color,
    pub chip_bg: Color,
    pub chip_text: Color,
    /// Category colours in legend order (the order of `Category::ALL`).
    pub category_palette: Vec<Color>,

    // -- Timeline -----------------------------------------------------------
    /// Width of the thread name column.
    pub timeline_name_width: f32,
    /// Width of the header's view selector ("Timeline" + caret).
    pub timeline_view_width: f32,
    /// Width of the header's Live toggle.
    pub timeline_live_width: f32,
    /// Height of the ruler above the thread rows, and of the module area's
    /// pinned clock axis.
    pub ruler_height: f32,
    pub ruler_bg: Color,
    /// Height of one nesting level inside a thread track.
    pub level_height: f32,
    /// Vertical padding above and below a thread's levels.
    pub thread_row_padding: f32,
    pub timeline_block_text_size: f32,
    pub timeline_name_bg: Color,
    pub track_bg: Color,
    /// Vertical grid line aligned with a ruler tick.
    pub grid: Color,
    /// Rule between two thread rows.
    pub row_rule: Color,
    /// Sample label text (drawn only when the bar is wide enough).
    pub sample_text: Color,

    // -- Timeline selection -------------------------------------------------
    /// Outline that marks the selected block (Unity keeps a 1 px light frame
    /// around it).
    pub selection_outline: Color,
    /// Fill of the selected block's details box.
    pub selection_bg: Color,
    /// Border of the details box, and of the buttons inside it.
    pub selection_border: Color,
    /// Width cap of the details box; its text wraps at this width.
    pub selection_width: f32,

    // -- Timeline scroll bars -----------------------------------------------
    /// Thickness of the timeline's scroll bars (Unity's ZoomableArea uses 13 px),
    /// and of the horizontal bar under the tracks.
    pub scrollbar_width: f32,
    /// The trough a scroll bar's thumb moves in.
    pub scrollbar_bg: Color,
    pub scrollbar_thumb: Color,
    /// Thumb fill while the pointer is over it.
    pub scrollbar_thumb_hover: Color,
    /// Thumb fill while it is being dragged.
    pub scrollbar_thumb_active: Color,
    /// The grip lines on the horizontal bar's two zoom handles.
    pub scrollbar_grip: Color,
    /// Width of each zoom handle: the drag target at the thumb's ends.
    pub scrollbar_handle_width: f32,

    // -- Module area / timeline split ---------------------------------------
    /// Rule colour of the splitter between the module area and the timeline.
    /// Its own grey rather than [`separator`](Self::separator): the handle is
    /// interactive chrome, not another section rule.
    pub splitter_idle: Color,
    /// Rule colour of the splitter while the pointer is over it.
    pub splitter_hover: Color,
    /// Rule colour of the splitter while it is dragged.
    pub splitter_active: Color,

    // -- Data views ---------------------------------------------------------
    /// Row caps of the hierarchy and raw data views.
    pub max_tree_rows: usize,
    pub max_raw_rows: usize,
}

impl Style for ProfilerStyle {
    const FILE: &'static str = "Profiler.ron";

    fn dark() -> Self {
        Self {
            icons: ProfilerIconPaths::dark(),

            separator: Color::from_srgb8([0x25, 0x25, 0x25, 255]),
            separator_width: 1.0,

            toolbar_height: 20.0,
            toolbar_item_padding: 8.0,
            toolbar_icon_size: 16.0,
            toolbar_text: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            toolbar_text_size: 12.0,
            hover_bg: Color::from_srgb8([0x40, 0x40, 0x40, 255]),
            click_bg: Color::from_srgb8([74, 74, 74, 255]),
            recording_bg: Color::from_srgb8([0x49, 0x49, 0x49, 255]),
            selected_bg: Color::from_srgb8([0x2C, 0x2C, 0x2C, 255]),

            timeline_text: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            ruler_text: Color::from_srgb8([0x8A, 0x8A, 0x8A, 255]),
            small_text_size: 10.0,
            warning: Color::from_srgb8([0xE0, 0xB0, 0x3A, 255]),

            module_cpu_height: 169.0,
            module_gpu_height: 120.0,
            module_header_height: 20.0,
            module_badge_width: 14.0,
            module_badge_height: 14.0,
            module_badge_inset: 5.0,
            module_badge_gap: 5.0,
            module_name_width: 180.0,
            legend_row_height: 14.0,
            usage_bar_fill: 0.8,
            window_fram_count: 300,
            chart_bar: Color::from_srgb8([0x79, 0x7B, 0x1D, 255]),
            chart_grid: Color::from_srgb8([0x57, 0x57, 0x57, 255]),
            chart_cursor: Color::from_srgb8([0xE6, 0xE6, 0xE6, 255]),
            break_bg: Color::from_srgb8([0x3A, 0x3A, 0x3A, 255]),
            chip_bg: Color::from_srgb8([0x2A, 0x2A, 0x2A, 255]),
            chip_text: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            category_palette: unity_category_palette(),

            timeline_name_width: 180.0,
            timeline_view_width: 165.0,
            timeline_live_width: 55.0,
            ruler_height: 20.0,
            ruler_bg: Color::from_srgb8([0x34, 0x34, 0x34, 255]),
            level_height: 20.0,
            thread_row_padding: 2.0,
            timeline_block_text_size: 13.0,
            timeline_name_bg: Color::from_srgb8([0x38, 0x38, 0x38, 255]),
            track_bg: Color::from_srgb8([0x28, 0x28, 0x28, 255]),
            grid: Color::from_srgb8([0x2E, 0x2E, 0x2E, 255]),
            row_rule: Color::from_srgb8([0x1C, 0x1C, 0x1C, 255]),
            sample_text: Color::from_srgb8([0xED, 0xED, 0xED, 255]),

            selection_outline: Color::from_srgb8([0xE6, 0xE6, 0xE6, 255]),
            selection_bg: Color::from_srgb8([0x2C, 0x2C, 0x2C, 255]),
            selection_border: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            selection_width: 320.0,

            scrollbar_width: 13.0,
            scrollbar_bg: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            scrollbar_thumb: Color::from_srgb8([0x3A, 0x3A, 0x3A, 255]),
            scrollbar_thumb_hover: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            scrollbar_thumb_active: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            scrollbar_grip: Color::from_srgb8([0x8A, 0x8A, 0x8A, 255]),
            scrollbar_handle_width: 6.0,

            splitter_idle: Color::from_srgb8([0x6A, 0x6A, 0x6A, 255]),
            splitter_hover: Color::from_srgb8([0x9A, 0x9A, 0x9A, 255]),
            splitter_active: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),

            max_tree_rows: 4_000,
            max_raw_rows: 4_000,
        }
    }

    /// Light-skin starting point: Unity's light chrome, not yet reviewed
    /// against a light-skin capture (same caveat as the Console's light skin).
    fn light() -> Self {
        Self {
            icons: ProfilerIconPaths::light(),

            separator: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            separator_width: 1.0,

            toolbar_height: 20.0,
            toolbar_item_padding: 8.0,
            toolbar_icon_size: 16.0,
            toolbar_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            toolbar_text_size: 12.0,
            hover_bg: Color::from_srgb8([0xB9, 0xB9, 0xB9, 255]),
            click_bg: Color::from_srgb8([74, 74, 74, 255]),
            recording_bg: Color::from_srgb8([0xC4, 0xC4, 0xC4, 255]),
            selected_bg: Color::from_srgb8([0xA9, 0xA9, 0xA9, 255]),

            timeline_text: Color::from_srgb8([0x1E, 0x1E, 0x1E, 255]),
            ruler_text: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            small_text_size: 10.0,
            warning: Color::from_srgb8([0xB0, 0x78, 0x00, 255]),

            module_cpu_height: 169.0,
            module_gpu_height: 120.0,
            module_header_height: 20.0,
            module_badge_width: 14.0,
            module_badge_height: 14.0,
            module_badge_inset: 5.0,
            module_badge_gap: 5.0,
            module_name_width: 180.0,
            legend_row_height: 14.0,
            usage_bar_fill: 0.8,
            window_fram_count: 300,
            chart_bar: Color::from_srgb8([0x7E, 0x84, 0x20, 255]),
            chart_grid: Color::from_srgb8([0xA0, 0xA0, 0xA0, 255]),
            chart_cursor: Color::from_srgb8([0x30, 0x30, 0x30, 255]),
            break_bg: Color::from_srgb8([0xC0, 0xC0, 0xC0, 255]),
            chip_bg: Color::from_srgb8([0xDC, 0xDC, 0xDC, 255]),
            chip_text: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            category_palette: unity_category_palette(),

            timeline_name_width: 180.0,
            timeline_view_width: 165.0,
            timeline_live_width: 55.0,
            ruler_height: 20.0,
            ruler_bg: Color::from_srgb8([0xC8, 0xC8, 0xC8, 255]),
            level_height: 20.0,
            thread_row_padding: 2.0,
            timeline_block_text_size: 13.0,
            timeline_name_bg: Color::from_srgb8([0xD2, 0xD2, 0xD2, 255]),
            track_bg: Color::from_srgb8([0xEE, 0xEE, 0xEE, 255]),
            grid: Color::from_srgb8([0xC9, 0xC9, 0xC9, 255]),
            row_rule: Color::from_srgb8([0xB5, 0xB5, 0xB5, 255]),
            sample_text: Color::from_srgb8([0x10, 0x10, 0x10, 255]),

            selection_outline: Color::from_srgb8([0x20, 0x20, 0x20, 255]),
            selection_bg: Color::from_srgb8([0xF2, 0xF2, 0xF2, 255]),
            selection_border: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            selection_width: 320.0,

            scrollbar_width: 13.0,
            scrollbar_bg: Color::from_srgb8([0xC6, 0xC6, 0xC6, 255]),
            scrollbar_thumb: Color::from_srgb8([0xA6, 0xA6, 0xA6, 255]),
            scrollbar_thumb_hover: Color::from_srgb8([0x8E, 0x8E, 0x8E, 255]),
            scrollbar_thumb_active: Color::from_srgb8([0x7A, 0x7A, 0x7A, 255]),
            scrollbar_grip: Color::from_srgb8([0x5A, 0x5A, 0x5A, 255]),
            scrollbar_handle_width: 6.0,

            splitter_idle: Color::from_srgb8([0x6A, 0x6A, 0x6A, 255]),
            splitter_hover: Color::from_srgb8([0x4A, 0x4A, 0x4A, 255]),
            splitter_active: Color::from_srgb8([0x30, 0x30, 0x30, 255]),

            max_tree_rows: 4_000,
            max_raw_rows: 4_000,
        }
    }
}

/// The Profiler legend palette, sampled from the Unity/Tuanjie capture
/// (Rendering, Scripts, Physics, Animation, GarbageCollector, VSync, Global
/// Illumination, UI, Others).
fn unity_palette() -> Vec<Color> {
    vec![
        Color::from_srgb8([0xA6, 0xB6, 0x2B, 255]),
        Color::from_srgb8([0x4A, 0x90, 0xB8, 255]),
        Color::from_srgb8([0xC7, 0x78, 0x26, 255]),
        Color::from_srgb8([0x2E, 0x8E, 0x9E, 255]),
        Color::from_srgb8([0x8A, 0x7D, 0x14, 255]),
        Color::from_srgb8([0xB3, 0x9A, 0x1C, 255]),
        Color::from_srgb8([0xA3, 0x34, 0x10, 255]),
        Color::from_srgb8([0x7B, 0x5C, 0xA5, 255]),
        Color::from_srgb8([0x8A, 0x87, 0x26, 255]),
    ]
}

/// Category colours in legend order, picked from the Unity legend swatches:
/// Rendering, Scripts, Physics, Audio (Unity's Animation swatch), IO (Unity's
/// Global Illumination swatch standing in for Loading), Editor (Unity's UI
/// swatch), VSync, Others.
fn unity_category_palette() -> Vec<Color> {
    let unity = unity_palette();
    [0, 1, 2, 3, 6, 7, 5, 8]
        .iter()
        .filter_map(|index| unity.get(*index).copied())
        .collect()
}

/// Runtime icon file paths for the Profiler panel, relative to the editor's
/// working directory: Unity's toolbar transport artwork plus the module badges
/// (the editor's own marks, which replace the painted CPU/GPU letters).
#[derive(Clone, Serialize, Deserialize, StyleFields)]
pub struct ProfilerIconPaths {
    pub tab: String,
    pub record_on: String,
    pub record_off: String,
    pub prev_frame: String,
    pub next_frame: String,
    pub current_frame: String,
    /// The CPU/GPU mark in each module's title row. Unity ships this artwork
    /// per module; these are the editor's own @2x badge files, pre-coloured
    /// for their skin like every other icon here.
    pub cpu_badge: String,
    pub gpu_badge: String,
}

impl ProfilerIconPaths {
    pub fn dark() -> Self {
        Self {
            tab: "Preferences/Textures/tab_icons/profiler.png".to_owned(),
            record_on: "Preferences/Textures/anim/record_on.png".to_owned(),
            record_off: "Preferences/Textures/anim/record_off.png".to_owned(),
            prev_frame: "Preferences/Textures/anim/prev_frame.png".to_owned(),
            next_frame: "Preferences/Textures/anim/next_frame.png".to_owned(),
            current_frame: "Preferences/Textures/anim/current_frame.png".to_owned(),
            cpu_badge: "Preferences/Textures/profiler/cpu.png".to_owned(),
            gpu_badge: "Preferences/Textures/profiler/gpu.png".to_owned(),
        }
    }

    pub fn light() -> Self {
        Self {
            tab: "Preferences/Textures/tab_icons/profiler.png".to_owned(),
            record_on: "Preferences/Textures/anim/light/record_on.png".to_owned(),
            record_off: "Preferences/Textures/anim/light/record_off.png".to_owned(),
            prev_frame: "Preferences/Textures/anim/light/prev_frame.png".to_owned(),
            next_frame: "Preferences/Textures/anim/light/next_frame.png".to_owned(),
            current_frame: "Preferences/Textures/anim/light/current_frame.png".to_owned(),
            cpu_badge: "Preferences/Textures/profiler/light/cpu.png".to_owned(),
            gpu_badge: "Preferences/Textures/profiler/light/gpu.png".to_owned(),
        }
    }

    /// All paths in deterministic order, for preloading in `Panel::init`.
    pub fn all(&self) -> [&str; 8] {
        [
            &self.tab,
            &self.record_on,
            &self.record_off,
            &self.prev_frame,
            &self.next_frame,
            &self.current_frame,
            &self.cpu_badge,
            &self.gpu_badge,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kairos_editor_widgets::style::StyleFile;
    use strum::EnumCount;

    /// The shipped skin is user-editable: it must keep parsing (a missing field
    /// falls back to the code defaults and silently drops the edit) and its
    /// toolbar artwork must exist on disk.
    #[test]
    fn the_shipped_style_file_loads_with_its_icons() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let file = StyleFile::<ProfilerStyle>::load_from(
            &workspace.join(StyleFile::<ProfilerStyle>::path()),
        )
        .expect("Preferences/Styles/Profiler.ron parses");

        for style in [file.dark, file.light] {
            assert!(style.toolbar_height > 0.0);
            assert!(style.toolbar_icon_size > 0.0);
            assert!(style.toolbar_text_size > 0.0);
            assert!(style.level_height > 0.0);
            assert!(style.module_cpu_height > 0.0);
            assert!(style.module_gpu_height > 0.0);
            // A badge taller than its title row would paint over the legend
            // below it; a zero-width one would drop the module's identity.
            assert!(style.module_badge_width > 0.0);
            assert!(style.module_badge_height > 0.0);
            assert!(style.module_badge_height <= style.module_header_height);
            assert!(style.module_badge_gap >= 0.0);
            assert!(style.module_badge_inset >= 0.0);
            assert!(style.scrollbar_width > 0.0);
            assert!(style.scrollbar_handle_width > 0.0);
            assert_eq!(
                style.category_palette.len(),
                kairos_profile::Category::COUNT,
                "the shipped skin paints one swatch per category"
            );
            for path in style.icons.all() {
                assert!(
                    workspace.join(path).is_file(),
                    "missing profiler icon {path}"
                );
            }
        }
    }

    /// The code defaults paint every category: the panel indexes the palette by
    /// the category's discriminant, so a short palette would paint the missing
    /// categories with the chart colour.
    #[test]
    fn the_category_palette_covers_every_category() {
        for style in [ProfilerStyle::dark(), ProfilerStyle::light()] {
            assert_eq!(
                style.category_palette.len(),
                kairos_profile::Category::COUNT
            );
        }
    }
}
