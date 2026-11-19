use egui::{Align2, Rect, Stroke, Ui};
use kairos_editor_widgets::color::ColorExt;

use crate::panels::profiler::ProfilerPanel;

impl ProfilerPanel {
    /// The GPU Usage card: a title row with Unity's warning and an empty body
    /// until the capture records GPU timings.
    ///
    /// The card keeps its fixed height so the module pane can reveal it by
    /// scrolling; the body is the slot a graph will occupy once the renderer
    /// reports GPU frame timings into the capture.
    pub(super) fn paint_gpu_module(&self, ui: &Ui, row: Rect) {
        let name_width = self.style.module_name_width.min(row.width());
        let header = Rect::from_min_size(
            row.min,
            egui::vec2(row.width(), self.style.module_header_height),
        );
        let name_col = Rect::from_min_max(
            row.min,
            egui::pos2(row.left() + name_width, header.bottom()),
        );
        let separator_color = self.style.separator.to_egui();

        // Column rule between the legend and the chart, inside the name column.
        self.paint_vertical_rule(
            ui,
            egui::pos2(name_col.right() - self.style.separator_width, row.top()),
            row.height(),
            separator_color,
        );
        self.paint_module_header(
            ui,
            header,
            name_col,
            &self.style.icons.gpu_badge,
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

        let body = Rect::from_min_max(egui::pos2(row.left(), header.bottom()), row.max);
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
}
