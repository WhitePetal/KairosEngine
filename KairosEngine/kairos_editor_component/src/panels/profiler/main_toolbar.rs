use egui::{Response, Sense, Ui};
use kairos_editor_widgets::{
    button::{Button, ButtonStyle},
    color::Color32Ext,
    divider::Divider,
};
use kairos_profile::{ProfileSession, Profiler};

use crate::panels::profiler::ProfilerPanel;

impl ProfilerPanel {
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
    pub(super) fn main_toolbar(
        &mut self,
        ui: &mut Ui,
        profiler: &Profiler,
        session: &ProfileSession,
    ) {
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
        let separator_color = self.style.separator.to_egui();
        // Every icon toggle is the same square; the rules match the row it forms.
        let row_height = self
            .style
            .toolbar_height
            .max(self.style.toolbar_icon_size + 2.0 * button_style.padding.y);
        let last = session.frames.len().saturating_sub(1);
        let has_frames = !session.frames.is_empty();

        ui.scope(|ui| {
            // Toolbar segments sit edge to edge: the rules are the
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
                self.toolbar_vseparator(ui, separator_color, row_height);

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
                    self.reveal_frame = true;
                }
                self.toolbar_vseparator(ui, separator_color, row_height);

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
                    self.reveal_frame = true;
                }
                self.toolbar_vseparator(ui, separator_color, row_height);

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
                self.toolbar_vseparator(ui, separator_color, row_height);

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
                self.toolbar_vseparator(ui, separator_color, row_height);

                let clear = ui.add(
                    Button::new("Clear", &button_style)
                        .height(row_height)
                        .enabled(has_frames),
                );
                if clear.clicked() {
                    profiler.clear();
                    self.reset_view();
                }
                self.toolbar_vseparator(ui, separator_color, row_height);

                // Unity's Save writes the capture; the files named in the
                // result are shown next to the button and in the hover text, so
                // "where did it go" is never a guess.
                let save = ui.add(
                    Button::new("Save", &button_style)
                        .height(row_height)
                        .enabled(has_frames)
                        .tooltip("Save the capture (JSON, Chrome trace and puffin)"),
                );
                if save.clicked() {
                    let written = profiler.save_capture(kairos_profile::DEFAULT_CAPTURE_DIR);
                    match &written {
                        Ok(files) => {
                            kairos_log::info!(
                                target: "kairos_editor",
                                "profiler capture saved: {} (trace {}, puffin {})",
                                files.json.display(),
                                files.chrome_trace.display(),
                                files.puffin.display(),
                            );
                        }
                        Err(error) => {
                            kairos_log::error!(
                                target: "kairos_editor",
                                "profiler capture save failed, error: {}",
                                error
                            );
                        }
                    }
                    self.save = Some(written.map_err(|error| error.to_string()));
                }
                self.toolbar_vseparator(ui, separator_color, row_height);

                // TODO!: save 改为打开文件创建/保存窗口，由用户手动保存，去掉这里的 saved 提示
                if let Some(result) = &self.save {
                    let (label, tooltip) = match result {
                        Ok(files) => (
                            "Saved".to_owned(),
                            format!(
                                "{}\n{}\n{}",
                                files.json.display(),
                                files.chrome_trace.display(),
                                files.puffin.display(),
                            ),
                        ),
                        Err(error) => ("Save failed".to_owned(), error.clone()),
                    };
                    self.toolbar_label(ui, &label, row_height)
                        .on_hover_text(tooltip);
                }

                // TODO!: Right-hand group — `Clear on Play`,
                // `Deep Profile`, `Call Stacks` and the help/overflow icon
                // cluster. It needs recording options the host does not have.
            });
            ui.add(
                Divider::horizontal()
                    .thickness(self.style.separator_width)
                    .color(separator_color),
            );
        });
    }

    /// Close one toolbar segment with 1 px vertical separator.
    fn toolbar_vseparator(&self, ui: &mut Ui, color: egui::Color32, height: f32) {
        ui.add(
            Divider::vertical()
                .length(height)
                .thickness(self.style.separator_width)
                .color(color),
        );
    }

    /// A non-interactive toolbar segment, e.g. Unity's `Frame: 29 / 53`.
    fn toolbar_label(&self, ui: &mut Ui, text: &str, height: f32) -> Response {
        let style = self.toolbar_text();
        let galley = style.layout(ui, text);
        let padding = self.style.toolbar_item_padding;
        let (rect, response) = ui.allocate_exact_size(
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
        response
    }
}
