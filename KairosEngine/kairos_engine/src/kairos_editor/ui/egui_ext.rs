use std::ops::RangeInclusive;

use egui::{DragValue, Response, Sense, StrokeKind, Ui};

/// egui [`Ui`] 扩展 trait，提供原生没有的控件。
pub trait UiExt {
    /// 双端范围滑块。
    ///
    /// 水平布局：`[low DragValue] [====轨道====] [high DragValue]`
    /// - 拖动轨道上的手柄或点击轨道来调整 low/high
    /// - 所有颜色取自 `ui.visuals()`，自动适配明暗主题
    /// - 手柄/轨道样式匹配 egui 原生 [`egui::Slider`]
    fn range_slider(
        &mut self,
        low: &mut f32,
        high: &mut f32,
        range: RangeInclusive<f32>,
    ) -> Response;

    fn truncate_text_to_height(
        &self,
        text: &str,
        font_size: f32,
        max_width: f32,
        max_height: f32,
    ) -> String;
}

impl UiExt for Ui {
    fn range_slider(
        &mut self,
        low: &mut f32,
        high: &mut f32,
        range: RangeInclusive<f32>,
    ) -> Response {
        let start = *range.start();
        let end = *range.end();

        // 边界保护 & 顺序保证
        *low = low.clamp(start, end);
        *high = high.clamp(start, end);
        if *low > *high {
            std::mem::swap(low, high);
        }

        self.horizontal(|ui| {
            let item_spacing_x = ui.spacing().item_spacing.x;
            let slider_rail_height = ui.spacing().slider_rail_height;
            let visuals = ui.visuals().clone();

            // --- 布局计算：确保左右 DragValue 等宽 ---
            let drag_width = 50.0;
            let max_track_width = 150.0; // 防止滑块条过长
            let min_track_width = 20.0;

            let track_width = (ui.available_width() - drag_width * 2.0 - item_spacing_x * 2.0)
                .min(max_track_width)
                .max(min_track_width);

            // --- 左侧 DragValue ---
            ui.add_sized(
                [drag_width, ui.available_height()],
                DragValue::new(low).range(start..=end),
            );

            // --- 轨道 ---
            let rail_height = slider_rail_height; // 匹配 egui Slider 的轨道高度
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(track_width, rail_height),
                Sense::click_and_drag(),
            );

            if ui.is_rect_visible(rect) {
                let corner_radius = visuals.widgets.inactive.corner_radius;

                // 轨道背景（仅填充，不描边，匹配 egui Slider）
                ui.painter()
                    .rect_filled(rect, corner_radius, visuals.widgets.inactive.bg_fill);

                // 选中范围填充
                let range_span = end - start;
                if range_span > 0.0 {
                    let norm_low = (*low - start) / range_span;
                    let norm_high = (*high - start) / range_span;
                    let fill_rect = egui::Rect::from_min_max(
                        rect.lerp_inside(egui::vec2(norm_low, 0.0)),
                        rect.lerp_inside(egui::vec2(norm_high, 1.0)),
                    );
                    // selection.bg_fill：与 egui Slider 的 trailing_fill 一致
                    ui.painter()
                        .rect_filled(fill_rect, corner_radius, visuals.selection.bg_fill);
                }

                // 手柄：圆角矩形，匹配 egui 默认 HandleShape::Rect { aspect_ratio: 0.75 }
                let handle_color = if response.dragged() {
                    visuals.widgets.active.bg_fill
                } else if response.hovered() {
                    visuals.widgets.hovered.bg_fill
                } else {
                    visuals.widgets.inactive.bg_fill
                };
                let handle_stroke = visuals.widgets.inactive.fg_stroke;

                // 手柄尺寸：比轨道略大，宽度 = 高度 * aspect_ratio
                let handle_h = rail_height * 1.5;
                let aspect_ratio = 0.75;
                let handle_w = handle_h * aspect_ratio;
                let handle_corner_radius = visuals.widgets.inactive.corner_radius;

                if range_span > 0.0 {
                    let norm_low = (*low - start) / range_span;
                    let norm_high = (*high - start) / range_span;
                    let cx_low = rect.left() + rect.width() * norm_low;
                    let cx_high = rect.left() + rect.width() * norm_high;
                    let cy = rect.center().y;

                    let handle_rect_low = egui::Rect::from_center_size(
                        egui::pos2(cx_low, cy),
                        egui::vec2(handle_w, handle_h),
                    );
                    let handle_rect_high = egui::Rect::from_center_size(
                        egui::pos2(cx_high, cy),
                        egui::vec2(handle_w, handle_h),
                    );

                    ui.painter().rect(
                        handle_rect_low,
                        handle_corner_radius,
                        handle_color,
                        handle_stroke,
                        StrokeKind::Inside,
                    );
                    ui.painter().rect(
                        handle_rect_high,
                        handle_corner_radius,
                        handle_color,
                        handle_stroke,
                        StrokeKind::Inside,
                    );
                }
            }

            // --- 轨道拖拽交互 ---
            if response.dragged() || response.clicked() {
                if let Some(pos) = response.hover_pos() {
                    let range_span = end - start;
                    if range_span > 0.0 {
                        let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                        let value = start + t * range_span;
                        let dist_low = (value - *low).abs();
                        let dist_high = (value - *high).abs();
                        if dist_low <= dist_high {
                            *low = value.clamp(start, *high);
                        } else {
                            *high = value.clamp(*low, end);
                        }
                    }
                }
            }

            // --- 右侧 DragValue ---
            ui.add_sized(
                [drag_width, ui.available_height()],
                DragValue::new(high).range(start..=end),
            );
        })
        .response
    }

    /// 将文本截断到指定高度内，超出部分用 "..." 替代
    fn truncate_text_to_height(
        &self,
        text: &str,
        font_size: f32,
        max_width: f32,
        max_height: f32,
    ) -> String {
        let font_id = egui::TextStyle::Body.resolve(self.style());
        let font_id = egui::FontId::new(font_size, font_id.family.clone());

        let line_height = self.ctx().fonts_mut(|f| f.row_height(&font_id));
        let max_lines = (max_height / line_height).floor().max(1.0) as usize;

        // 逐行截断：保留前 max_lines 行，最后一行加 "..."
        let galley = self.ctx().fonts_mut(|fonts| {
            fonts.layout(
                text.to_string(),
                font_id.clone(),
                egui::Color32::WHITE,
                max_width,
            )
        });

        if galley.rows.len() <= max_lines {
            return text.to_string();
        }

        // 取前 max_lines 行的 glyphs，从最后一行末尾逐步回退
        let ellipsis = "...";
        let mut candidate = String::new();
        for row in galley.rows.iter().take(max_lines) {
            for glyph in &row.row.glyphs {
                candidate.push(glyph.chr);
            }
        }

        // 从末尾回退直到 "..." + candidate 能放进 max_width
        while !candidate.is_empty() {
            let test = format!("{}{}", candidate, ellipsis);
            let w = self.ctx().fonts_mut(|f| {
                f.layout(test, font_id.clone(), egui::Color32::WHITE, f32::INFINITY)
                    .rect
                    .width()
            });
            if w <= max_width {
                return format!("{}{}", candidate, ellipsis);
            }
            candidate.pop();
        }

        ellipsis.to_string()
    }
}
