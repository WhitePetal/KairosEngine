//! Widget gallery panel: a host-contributed [`Panel`].
//!
//! This lives in `editor-app` (the host crate) on purpose: it proves that a panel
//! does **not** have to be known by `EditorApp` or by `editor-core`. The app
//! registers a factory in the `PanelRegistry` exactly like it does for the
//! project/inspector/console panels that live in `editor-components`.
//!
//! All state in here is purely local demo state; it has no domain meaning and
//! never leaves the panel.

use editor_components_prototype::{Frame, Panel, PanelId, UiOut};
use egui::Ui;
use ui_widgets_prototype::{
    Badge, BadgeTone, Breadcrumb, Button, ButtonVariant, Checkbox, ColorSwatch, ComboSelect,
    Expander, FormSection, Heading, IconId, Label, ListItem, NumberRow, PropertyRow, SearchBox,
    Separator, SliderRow, Spinner, TextInput, TreeRow, TreeView,
};

/// Stable panel kind of the gallery. It is a string id, not a core enum variant:
/// the store treats it like any other panel and knows nothing about widgets.
pub const GALLERY_PANEL: PanelId = PanelId("gallery");

/// Widget gallery panel.
pub struct GalleryPanel {
    demo_text: String,
    demo_search: String,
    demo_slider: f32,
    demo_number: f64,
    demo_combo: usize,
    demo_checkbox: bool,
}

impl GalleryPanel {
    pub fn new() -> Self {
        Self {
            demo_text: "hero.scene".to_string(),
            demo_search: String::new(),
            demo_slider: 0.45,
            demo_number: 128.0,
            demo_combo: 0,
            demo_checkbox: true,
        }
    }
}

impl Default for GalleryPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for GalleryPanel {
    fn kind(&self) -> PanelId {
        GALLERY_PANEL
    }

    fn title(&self, _frame: &Frame) -> String {
        "Widget gallery".to_string()
    }

    fn draw(&mut self, ui: &mut Ui, _frame: &Frame, _out: &mut UiOut) {
        ui.add(Heading::new("Widget gallery"));
        ui.add(
            Label::new("The pure ui-widgets kit, theme-driven. This panel is registered by the host, not hardcoded in EditorApp.")
                .muted(),
        );
        ui.add(Separator);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let atoms = FormSection::new("Atoms");
                atoms.default_open(true).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let _ = ui.add(Button::primary("Primary"));
                        let _ = ui.add(Button::new("Secondary").variant(ButtonVariant::Secondary));
                        let _ = ui.add(Button::new("Ghost").variant(ButtonVariant::Ghost));
                        let _ = ui.add(Button::new("Danger").variant(ButtonVariant::Danger));
                        let _ = ui.add(Button::new("Disabled").enabled(false));
                        let _ = ui.add(Spinner::new());
                        let _ = ui.add(Badge::new("Accent").tone(BadgeTone::Accent));
                        let _ = ui.add(Badge::new("Error").tone(BadgeTone::Error));
                        let _ = ui.add(ColorSwatch::new(egui::Color32::from_rgb(210, 140, 70)));
                    });
                    ui.add(TextInput::new(&mut self.demo_text).hint("asset name"));
                    if ui
                        .add(Checkbox::new(self.demo_checkbox, "Wireframe preview"))
                        .clicked()
                    {
                        self.demo_checkbox = !self.demo_checkbox;
                    }
                    ui.add(SliderRow::new(
                        "Roughness",
                        &mut self.demo_slider,
                        0.0..=1.0,
                    ));
                    ui.add(NumberRow::new("Instances", &mut self.demo_number).range(1.0..=4096.0));
                    ui.add(ComboSelect::new(
                        &["Metal", "Plastic", "Glass"],
                        &mut self.demo_combo,
                    ));
                    let mut search = SearchBox::new("Filter properties...");
                    search.show(ui, &mut self.demo_search);
                });

                let compounds = FormSection::new("Compounds");
                compounds.default_open(true).show(ui, |ui| {
                    let mut breadcrumb =
                        Breadcrumb::new(vec!["assets", "models", "crate.material"]);
                    breadcrumb.show(ui);

                    let mut item = ListItem::new("hero.scene")
                        .icon(IconId::Scene)
                        .suffix(".scene");
                    let _ = item.show(ui);

                    let rows = vec![
                        TreeRow::new(1, "assets")
                            .indent(0)
                            .expander(Expander::Expanded)
                            .icon(IconId::FolderOpen),
                        TreeRow::new(2, "models")
                            .indent(1)
                            .expander(Expander::Expanded)
                            .icon(IconId::FolderOpen),
                        TreeRow::new(3, "hero.scene")
                            .indent(2)
                            .icon(IconId::Scene)
                            .suffix(".scene"),
                        TreeRow::new(4, "crate.material")
                            .indent(2)
                            .selected(true)
                            .icon(IconId::Material)
                            .suffix(".material"),
                    ];
                    let mut tree = TreeView::new("gallery_tree").rows(rows);
                    let _ = tree.show(ui);

                    let mut row = PropertyRow::new("Base color").reset_enabled(true);
                    let _ = row.show(ui, |ui| {
                        ui.add(ColorSwatch::new(egui::Color32::from_rgb(200, 120, 60)))
                    });
                });
            });
    }
}
