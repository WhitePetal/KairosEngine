use std::collections::{BTreeMap, BTreeSet};

use egui::{Align2, CornerRadius, Rect, Sense, Ui};
use kairos_editor_widgets::{color::Color32Ext, min_max_scroll::{MinMaxScroll, ScrollWindow}};
use kairos_profile::{Category, HierarchyRow, ProfileSession, SYSTEM_MARKER, ScopeInfo, hierarchy, strip_tag};

use crate::panels::profiler::{ProfilerPanel, TimelineLayout, Triangle, duration_text, paint_triangle, scope_info, short_system_name};



/// The Hierarchy view's sibling ordering: the column the user last clicked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HierarchySort {
    #[default]
    Total,
    SelfTime,
    Calls,
    Max,
    Name,
}

impl HierarchySort {
    fn label(self) -> &'static str {
        match self {
            Self::Total => "Total",
            Self::SelfTime => "Self",
            Self::Calls => "Calls",
            Self::Max => "Max",
            Self::Name => "Hierarchy",
        }
    }
}

/// One row the current collapse state shows, addressed into the cached tree.
#[derive(Clone, Debug)]
pub(super) struct VisibleHierarchyRow {
    /// Index into the cached HierarchyRow list.
    pub(super) index: usize,
    /// Stable identity of the row's place in the tree, for collapsing.
    pub(super) key: String,
    /// Whether the row has children in the cached list.
    pub(super) has_children: bool,
}

/// A Hierarchy row's local key: marker id plus the per-invocation data.
///
/// Rows are cached pre-order and re-sorted when a column is clicked, so a
/// collapsed row cannot be addressed by index; its path (this key per level)
/// is stable across sorts and frames.
fn hierarchy_row_key(row: &HierarchyRow) -> String {
    format!("{}:{}", row.scope, row.data.as_deref().unwrap_or_default())
}

/// The name the Hierarchy view prints for a row.
pub(super) fn hierarchy_row_label(row: &HierarchyRow, scopes: &[ScopeInfo]) -> String {
    match scope_info(scopes, row.scope) {
        Some(info) if info.name == SYSTEM_MARKER => row
            .data
            .as_deref()
            .map(strip_tag)
            .map(short_system_name)
            .unwrap_or_else(|| info.name.clone()),
        Some(info) => match row.data.as_deref().map(strip_tag) {
            Some(data) if !data.is_empty() => format!("{}[{}]", info.name, data),
            _ => info.name.clone(),
        },
        None => format!("scope #{}", row.scope),
    }
}

/// The full name the hierarchy tooltip prints; a system keeps its module path.
fn hierarchy_row_full_name(row: &HierarchyRow, scopes: &[ScopeInfo]) -> String {
    match scope_info(scopes, row.scope) {
        Some(info) if info.name == SYSTEM_MARKER => {
            strip_tag(row.data.as_deref().unwrap_or_default()).to_owned()
        }
        _ => hierarchy_row_label(row, scopes),
    }
}

/// The category a Hierarchy row is painted in.
///
/// The aggregation strips the category tag from a system's data, so the
/// per-system category is looked up from the frame's executor samples (the
/// cache collects them once per frame).
fn hierarchy_row_category(
    row: &HierarchyRow,
    scopes: &[ScopeInfo],
    system_categories: &BTreeMap<String, Category>,
) -> Category {
    let Some(info) = scope_info(scopes, row.scope) else {
        return Category::Others;
    };
    if info.name == SYSTEM_MARKER {
        return system_categories
            .get(row.data.as_deref().unwrap_or_default())
            .copied()
            .unwrap_or(Category::Scripts);
    }
    info.category
}

/// Re-order the aggregated rows by the chosen column, keeping every parent
/// above its children: sibling lists are sorted and the pre-order flatten is
/// rebuilt.
///
/// "Descending" is "best first": the largest Total/Self/Calls/Max, or the
/// alphabetically first name.
pub(super) fn sort_hierarchy_rows(
    rows: &mut Vec<HierarchyRow>,
    sort: HierarchySort,
    descending: bool,
    scopes: &[ScopeInfo],
) {
    if rows.len() < 2 {
        return;
    }

    // Rebuild the parent/child shape from the pre-order depths.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); rows.len()];
    let mut roots = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    for index in 0..rows.len() {
        while stack
            .last()
            .is_some_and(|&top| rows[top].depth >= rows[index].depth)
        {
            stack.pop();
        }
        match stack.last() {
            Some(&parent) => children[parent].push(index),
            None => roots.push(index),
        }
        stack.push(index);
    }

    let labels: Vec<String> = rows
        .iter()
        .map(|row| hierarchy_row_label(row, scopes))
        .collect();
    let compare = |a: usize, b: usize| -> std::cmp::Ordering {
        let natural = match sort {
            HierarchySort::Total => rows[b].total_ns.cmp(&rows[a].total_ns),
            HierarchySort::SelfTime => rows[b].self_ns.cmp(&rows[a].self_ns),
            HierarchySort::Calls => rows[b].calls.cmp(&rows[a].calls),
            HierarchySort::Max => rows[b].max_ns.cmp(&rows[a].max_ns),
            HierarchySort::Name => labels[a].cmp(&labels[b]),
        };
        let natural = natural
            .then_with(|| labels[a].cmp(&labels[b]))
            .then_with(|| rows[a].scope.cmp(&rows[b].scope));
        if descending {
            natural
        } else {
            natural.reverse()
        }
    };

    roots.sort_by(|a, b| compare(*a, *b));
    for child_list in &mut children {
        child_list.sort_by(|a, b| compare(*a, *b));
    }

    let original = rows.clone();
    fn walk(
        index: usize,
        children: &[Vec<usize>],
        original: &[HierarchyRow],
        order: &mut Vec<HierarchyRow>,
    ) {
        order.push(original[index].clone());
        for &child in &children[index] {
            walk(child, children, original, order);
        }
    }
    let mut order = Vec::with_capacity(original.len());
    for root in roots {
        walk(root, &children, &original, &mut order);
    }
    *rows = order;
}

/// The right-aligned numeric columns of the Hierarchy view.
///
/// The name column takes the left 45% of the track, the four numbers split the
/// rest evenly; the header and the rows share this geometry so a click on a
/// header lands on the column it names.
#[derive(Clone, Copy, Debug)]
struct HierarchyColumns {
    name_left: f32,
    name_right: f32,
    total_left: f32,
    total_right: f32,
    self_left: f32,
    self_right: f32,
    calls_left: f32,
    calls_right: f32,
    max_left: f32,
    max_right: f32,
}

impl HierarchyColumns {
    fn new(left: f32, right: f32) -> Self {
        let width = (right - left).max(1.0);
        // A narrow panel must not make the clamp bounds cross (min > max).
        let numeric = (width * 0.55).clamp(120.0_f32.min(width), width);
        let name_right = left + width - numeric;
        let column = numeric / 4.0;
        Self {
            name_left: left,
            name_right,
            total_left: name_right,
            total_right: name_right + column,
            self_left: name_right + column,
            self_right: name_right + 2.0 * column,
            calls_left: name_right + 2.0 * column,
            calls_right: name_right + 3.0 * column,
            max_left: name_right + 3.0 * column,
            max_right: name_right + 4.0 * column,
        }
    }

    /// The five sortable header cells, left to right.
    fn header_cells(&self, ruler: Rect) -> [(HierarchySort, Rect); 5] {
        let cell = |sort: HierarchySort, left: f32, right: f32| {
            (
                sort,
                Rect::from_min_max(
                    egui::pos2(left, ruler.top()),
                    egui::pos2(right, ruler.bottom()),
                ),
            )
        };
        [
            cell(HierarchySort::Name, self.name_left, self.name_right),
            cell(HierarchySort::Total, self.total_left, self.total_right),
            cell(HierarchySort::SelfTime, self.self_left, self.self_right),
            cell(HierarchySort::Calls, self.calls_left, self.calls_right),
            cell(HierarchySort::Max, self.max_left, self.max_right),
        ]
    }
}

/// The rows the collapse state shows, with the path keys that address them.
///
/// A collapsed row stays visible; its descendants (deeper rows that follow it)
/// are skipped until the next row at its depth or above.
pub(super) fn visible_hierarchy_rows(
    rows: &[HierarchyRow],
    collapsed: &BTreeSet<String>,
) -> Vec<VisibleHierarchyRow> {
    let mut visible = Vec::with_capacity(rows.len());
    let mut path: Vec<String> = Vec::new();
    let mut skip_under: Option<u32> = None;
    for (index, row) in rows.iter().enumerate() {
        if let Some(depth) = skip_under {
            if row.depth > depth {
                continue;
            }
            skip_under = None;
        }
        path.truncate(row.depth as usize);
        let key = hierarchy_row_key(row);
        path.push(key);
        let key = path.join("/");
        let has_children = rows
            .get(index + 1)
            .is_some_and(|next| next.depth > row.depth);
        if has_children && collapsed.contains(&key) {
            skip_under = Some(row.depth);
        }
        visible.push(VisibleHierarchyRow {
            index,
            key,
            has_children,
        });
    }
    visible
}


impl ProfilerPanel {
    /// Unity's Hierarchy view over the selected frame: the aggregated marker
    /// tree with Unity's columns, click-to-sort headers and click-to-collapse
    /// rows.
    ///
    /// This is where the whole frame's ranking lives: unlike the timeline (one
    /// track per thread) it merges every thread, so a system that ran on a
    /// worker is found here even though the stage wrapper that scheduled it has
    /// no child blocks on the main thread.
    pub(super) fn hierarchy(&mut self, ui: &mut Ui, layout: &TimelineLayout, session: &ProfileSession) {
        let body = layout.body;
        let row_height = self.style.level_height.max(1.0);
        ui.painter()
            .rect_filled(body, CornerRadius::ZERO, self.style.track_bg.to_egui());
        // The Hierarchy view has no time axis, but the band keeps the same
        // chrome rows as the timeline so switching views does not jump.
        ui.painter().rect_filled(
            Rect::from_min_max(
                egui::pos2(body.left(), body.bottom()),
                egui::pos2(layout.track_right, layout.horizontal_bar.bottom()),
            ),
            CornerRadius::ZERO,
            self.style.timeline_name_bg.to_egui(),
        );
        self.paint_hierarchy_header(ui, layout, session);

        let has_rows = !self.frame_rows.is_empty();
        if !has_rows {
            self.paint_empty_timeline(ui, body);
            return;
        }

        // The rows the collapse state shows. Their keys do not borrow the
        // cache, so the scroll state can be updated before the paint.
        let visible = {
            visible_hierarchy_rows(&self.frame_rows, &self.collapsed)
        };
        let content_height = visible.len() as f32 * row_height;
        let max_scroll = (content_height - body.height()).max(0.0);
        self.track_scroll = self.track_scroll.clamp(0.0, max_scroll);

        // The wheel over the table scrolls the rows, like the timeline names.
        if let Some(pos) = ui.input(|input| input.pointer.hover_pos()) {
            if body.contains(pos) {
                let wheel = ui.input(|input| input.smooth_scroll_delta.y);
                if wheel != 0.0 {
                    self.track_scroll = (self.track_scroll - wheel).clamp(0.0, max_scroll);
                    ui.input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
                    ui.ctx().request_repaint();
                }
            }
        }

        // The right-edge scroll bar, same widget as the timeline's.
        let scroll_style = self.scroll_style();
        let viewport_height = body.height().max(1.0);
        let content = content_height.max(viewport_height);
        let mut window = ScrollWindow::new(
            f64::from(self.track_scroll),
            f64::from(viewport_height.min(content)),
            (0.0, f64::from(content)),
        )
        .with_min_span(f64::from(viewport_height.min(content)));
        let scrolled = MinMaxScroll::vertical(&scroll_style)
            .show(
                ui,
                ui.id().with("profiler.hierarchy.vscroll"),
                layout.vertical_bar,
                &mut window,
            )
            .changed();
        if scrolled {
            self.track_scroll = window.value as f32;
            ui.ctx().request_repaint();
        }

        let columns = HierarchyColumns::new(body.left(), layout.track_right);
        let painter = ui.painter().with_clip_rect(body);
        let font = self.small_font();
        let text = self.style.timeline_text.to_egui();
        let dim = self.style.ruler_text.to_egui();
        let rule = self.style.separator.to_egui();
        let hover = self.style.hover_bg.to_egui();
        let separator_width = self.style.separator_width;
        let rule_columns = [
            columns.total_left,
            columns.self_left,
            columns.calls_left,
            columns.max_left,
        ];

        let mut toggled = None;
        let mut y = body.top() - self.track_scroll;
        let frame = self.frame.as_ref().expect("cached hierarchy rows");
        for entry in &visible {
            let row = &self.frame_rows[entry.index];
            let rect = Rect::from_min_size(
                egui::pos2(body.left(), y),
                egui::vec2(layout.track_right - body.left(), row_height),
            );
            y += row_height;
            if !rect.intersects(body) {
                continue;
            }

            let response = ui.interact(
                rect,
                ui.id().with(("profiler.hierarchy.row", entry.index)),
                Sense::click(),
            );
            if response.hovered() {
                painter.rect_filled(rect, CornerRadius::ZERO, hover);
            }
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - separator_width),
                    egui::vec2(rect.width(), separator_width),
                ),
                CornerRadius::ZERO,
                rule,
            );
            for x in rule_columns {
                painter.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(separator_width, rect.height()),
                    ),
                    CornerRadius::ZERO,
                    rule,
                );
            }

            let indent = row.depth as f32 * 12.0;
            let caret = egui::pos2(rect.left() + 9.0 + indent, rect.center().y);
            if entry.has_children {
                let collapsed = self.collapsed.contains(&entry.key);
                paint_triangle(
                    &painter,
                    caret,
                    7.0,
                    if collapsed {
                        Triangle::Right
                    } else {
                        Triangle::Down
                    },
                    dim,
                );
            }
            let chip = egui::pos2(caret.x + 12.0, rect.center().y);
            let category = hierarchy_row_category(row, &frame.scopes, &frame.system_categories);
            painter.rect_filled(
                Rect::from_center_size(chip, egui::vec2(8.0, 8.0)),
                CornerRadius::same(1),
                self.category_color(category),
            );
            painter
                .with_clip_rect(Rect::from_min_max(
                    egui::pos2(chip.x + 8.0, rect.top()),
                    egui::pos2(columns.name_right - 4.0, rect.bottom()),
                ))
                .text(
                    egui::pos2(chip.x + 9.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    hierarchy_row_label(row, &frame.scopes),
                    font.clone(),
                    text,
                );
            painter.text(
                egui::pos2(columns.total_right - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                duration_text(row.total_ns),
                font.clone(),
                dim,
            );
            painter.text(
                egui::pos2(columns.self_right - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                duration_text(row.self_ns),
                font.clone(),
                dim,
            );
            painter.text(
                egui::pos2(columns.calls_right - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                row.calls.to_string(),
                font.clone(),
                dim,
            );
            painter.text(
                egui::pos2(columns.max_right - 6.0, rect.center().y),
                Align2::RIGHT_CENTER,
                duration_text(row.max_ns),
                font.clone(),
                dim,
            );

            if response.clicked() && entry.has_children {
                toggled = Some(entry.key.clone());
            }
            let mut tooltip = format!(
                "{}\nTotal {}   Self {}   Max {}   Calls {}",
                hierarchy_row_full_name(row, &frame.scopes),
                duration_text(row.total_ns),
                duration_text(row.self_ns),
                duration_text(row.max_ns),
                row.calls,
            );
            if let Some(info) = scope_info(&frame.scopes, row.scope) {
                if !info.location.is_empty() {
                    tooltip.push_str(&format!("\nLocation: {}", info.location));
                }
            }
            response.on_hover_text(tooltip);
        }

        if let Some(key) = toggled {
            if !self.collapsed.remove(&key) {
                self.collapsed.insert(key);
            }
            ui.ctx().request_repaint();
        }
    }

    /// The Hierarchy view's pinned column header row (Unity's column titles).
    pub(super) fn paint_hierarchy_header(&mut self, ui: &mut Ui, layout: &TimelineLayout, session: &ProfileSession) {
        let ruler = layout.ruler;
        ui.painter().rect_filled(
            ruler,
            CornerRadius::ZERO,
            self.style.timeline_name_bg.to_egui(),
        );
        let columns = HierarchyColumns::new(layout.body.left(), layout.track_right);
        let painter = ui.painter().with_clip_rect(ruler);
        let font = self.small_font();
        let text = self.style.timeline_text.to_egui();
        let dim = self.style.ruler_text.to_egui();
        let rule = self.style.separator.to_egui();
        let separator_width = self.style.separator_width;

        for (sort, cell) in columns.header_cells(ruler) {
            let response = ui.interact(
                cell,
                ui.id().with(("profiler.hierarchy.head", sort.label())),
                Sense::click(),
            );
            if response.clicked() {
                if self.hierarchy_sort == sort {
                    self.hierarchy_descending = !self.hierarchy_descending;
                } else {
                    self.hierarchy_sort = sort;
                    self.hierarchy_descending = true;
                }
                self.sort_hierachy(session);
                ui.ctx().request_repaint();
            }
            let (label, x, align) = match sort {
                HierarchySort::Name => (
                    "Hierarchy".to_owned(),
                    cell.left() + 10.0,
                    Align2::LEFT_CENTER,
                ),
                HierarchySort::Total => (
                    sort.label().to_owned(),
                    columns.total_right - 6.0,
                    Align2::RIGHT_CENTER,
                ),
                HierarchySort::SelfTime => (
                    sort.label().to_owned(),
                    columns.self_right - 6.0,
                    Align2::RIGHT_CENTER,
                ),
                HierarchySort::Calls => (
                    sort.label().to_owned(),
                    columns.calls_right - 6.0,
                    Align2::RIGHT_CENTER,
                ),
                HierarchySort::Max => (
                    sort.label().to_owned(),
                    columns.max_right - 6.0,
                    Align2::RIGHT_CENTER,
                ),
            };
            let color = if sort == self.hierarchy_sort {
                text
            } else {
                dim
            };
            if sort == self.hierarchy_sort {
                // Unity's sort arrow points down for the default best-first
                // order and up after a second click on the same column.
                let width = painter
                    .layout_no_wrap(label.clone(), font.clone(), color)
                    .size()
                    .x;
                let arrow_x = match sort {
                    HierarchySort::Name => x + width + 6.0,
                    _ => x - width - 6.0,
                };
                let direction = if self.hierarchy_descending {
                    Triangle::Down
                } else {
                    Triangle::Up
                };
                paint_triangle(
                    &painter,
                    egui::pos2(arrow_x, cell.center().y),
                    6.0,
                    direction,
                    color,
                );
            }
            painter.text(
                egui::pos2(x, cell.center().y),
                align,
                label,
                font.clone(),
                color,
            );
            response.on_hover_text(format!("Sort by {}", sort.label()));
        }

        for x in [
            columns.total_left,
            columns.self_left,
            columns.calls_left,
            columns.max_left,
        ] {
            painter.rect_filled(
                Rect::from_min_size(
                    egui::pos2(x, ruler.top()),
                    egui::vec2(separator_width, ruler.height()),
                ),
                CornerRadius::ZERO,
                rule,
            );
        }
        painter.rect_filled(
            Rect::from_min_size(
                egui::pos2(ruler.left(), ruler.bottom() - separator_width),
                egui::vec2(ruler.width(), separator_width),
            ),
            CornerRadius::ZERO,
            rule,
        );
    }

    pub(super) fn sort_hierachy(&mut self, session: &ProfileSession) {
        let Some(frame) = &self.frame else {
            return;
        };
        let mut rows = hierarchy(&frame.snapshot);
        sort_hierarchy_rows(
            &mut rows,
            self.hierarchy_sort,
            self.hierarchy_descending,
            &session.scopes,
        );
        self.frame_rows = rows;
    }
}
