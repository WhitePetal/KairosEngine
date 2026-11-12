//! Inspector feature component: renders the *active document from core state*
//! (Material/Toml/Text) with themed widgets and maps widget events to real
//! DocumentActions. No local document copy is held across frames except the
//! text-edit buffer that egui's TextEdit needs for the current pass.
//!
//! The host tab bar shows the panel title, so [`InspectorPanel::show`] draws the
//! body only (toolbar, path/status line, property form).

use editor_core_prototype::{
    Action, Document, DocumentAction, DocumentKind, EditData, MaterialData, PanelId,
};
use egui::{Rect, Ui};
use ui_widgets_prototype::{
    Badge, BadgeTone, ColorSwatch, EmptyState, FormSection, IconId, Label, PropertyRow, SliderRow,
    TextInput, Toolbar, ToolbarItem, ToolbarOutput,
};

use crate::frame::{Frame, UiOut};
use crate::panel::Panel;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectorPanelOutput {
    pub toolbar_rects: Vec<(&'static str, Rect)>,
    /// Editor hit rects keyed by property name (tests / overlay anchoring).
    pub field_rects: Vec<(String, Rect)>,
}

#[derive(Default)]
pub struct InspectorPanel;

impl InspectorPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) -> InspectorPanelOutput {
        let mut panel = InspectorPanelOutput::default();
        let Some(document) = frame.active_document() else {
            let mut empty =
                EmptyState::new("No document open", "Select an asset in the Project panel")
                    .icon(IconId::File);
            let _ = empty.show(ui);
            return panel;
        };

        // The host owns the tab/title bar (see crate::panel::Panel), so the body
        // starts with the panel toolbar instead of a PanelChrome title header.
        let mut toolbar = Toolbar::new(vec![
            ToolbarItem::button("save")
                .icon(IconId::Save)
                .tooltip("Save")
                .enabled(document.dirty && !document.is_saving()),
            ToolbarItem::button("undo")
                .icon(IconId::Revert)
                .tooltip("Undo")
                .enabled(document.can_undo()),
            ToolbarItem::button("redo")
                .icon(IconId::Check)
                .tooltip("Redo")
                .enabled(document.can_redo()),
            ToolbarItem::button("revert")
                .icon(IconId::Warning)
                .tooltip("Revert")
                .enabled(document.dirty),
        ]);
        let toolbar_output = toolbar.show(ui);
        panel.toolbar_rects = toolbar_output.rects.clone();
        out.extend(actions_from_toolbar(document, &toolbar_output));

        ui.horizontal(|ui| {
            ui.add(Label::new(&document.path).mono(true).muted());
            if document.dirty {
                ui.add(Badge::new("unsaved").tone(BadgeTone::Error));
            }
            if document.is_saving() {
                ui.add(Badge::new("saving").tone(BadgeTone::Accent));
            }
        });

        let title = match document.kind {
            DocumentKind::Material => "Material",
            DocumentKind::Toml => "Toml",
            DocumentKind::Text => "Text",
            DocumentKind::Binary => "Binary",
        };
        let mut section = FormSection::new(title);
        section.show(ui, |ui| match &document.edit {
            EditData::Material(material) => {
                let mut edited = material.clone();
                let mut changed = false;

                let mut color_row = PropertyRow::new("Base Color");
                let color_output = color_row.show(ui, |ui| {
                    ui.add(ColorSwatch::new(color32(material.base_color)))
                });
                panel
                    .field_rects
                    .push(("base_color".to_string(), color_output.editor_rect));

                let mut roughness_row = PropertyRow::new("Roughness").reset_enabled(true);
                let mut roughness = material.roughness;
                let output = roughness_row.show(ui, |ui| {
                    ui.add(SliderRow::new("", &mut roughness, 0.0..=1.0))
                });
                panel
                    .field_rects
                    .push(("roughness".to_string(), output.editor_rect));
                if output.changed {
                    edited.roughness = roughness;
                    changed = true;
                }

                let mut metallic_row = PropertyRow::new("Metallic").reset_enabled(true);
                let mut metallic = material.metallic;
                let output = metallic_row.show(ui, |ui| {
                    ui.add(SliderRow::new("", &mut metallic, 0.0..=1.0))
                });
                panel
                    .field_rects
                    .push(("metallic".to_string(), output.editor_rect));
                if output.changed {
                    edited.metallic = metallic;
                    changed = true;
                }

                if changed {
                    out.push(Action::Document(DocumentAction::SetMaterial {
                        document: document.id,
                        material: edited,
                    }));
                }
            }
            EditData::Text(text) | EditData::Toml(text) => {
                let mut text_row = PropertyRow::new("Text").reset_enabled(true);
                let mut buffer = text.clone();
                let output = text_row.show(ui, |ui| {
                    let mono = matches!(document.edit, EditData::Toml(_));
                    ui.add(TextInput::new(&mut buffer).mono(mono))
                });
                panel
                    .field_rects
                    .push(("text".to_string(), output.editor_rect));
                if output.changed {
                    out.push(Action::Document(DocumentAction::SetText {
                        document: document.id,
                        text: buffer,
                    }));
                }
            }
        });
        panel
    }
}

impl Panel for InspectorPanel {
    fn kind(&self) -> PanelId {
        PanelId::INSPECTOR
    }

    fn title(&self, _frame: &Frame) -> String {
        "Inspector".to_owned()
    }

    /// Draw the body only: the host tab bar already shows the title.
    fn draw(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut) {
        let _ = self.show(ui, frame, out);
    }
}

fn color32(rgb: [f32; 3]) -> egui::Color32 {
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Color32::from_rgb(channel(rgb[0]), channel(rgb[1]), channel(rgb[2]))
}

/// Pure mapping: inspector toolbar Output -> core DocumentActions.
pub fn actions_from_toolbar(document: &Document, output: &ToolbarOutput) -> Vec<Action> {
    match output.clicked {
        Some("save") => vec![Action::Document(DocumentAction::Save {
            document: document.id,
        })],
        Some("undo") => vec![Action::Document(DocumentAction::Undo {
            document: document.id,
        })],
        Some("redo") => vec![Action::Document(DocumentAction::Redo {
            document: document.id,
        })],
        Some("revert") => vec![Action::Document(DocumentAction::Revert {
            document: document.id,
        })],
        _ => Vec::new(),
    }
}

/// Unused-material helper kept public for component tests: build the full
/// material action a property edit would emit.
pub fn set_material_action(document: &Document, material: MaterialData) -> Action {
    Action::Document(DocumentAction::SetMaterial {
        document: document.id,
        material,
    })
}
