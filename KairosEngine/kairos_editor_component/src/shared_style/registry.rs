//! The app-owned live shared styles.
//!
//! The composition root loads one value per registered style at boot and hands
//! [`SharedStyles`] to every panel through [`Frame::shared`](crate::Frame). The
//! registry is also the write target of the separate shared-style action path:
//! `SetField` writes the live value, `Reset` restores the code defaults, and
//! `Persist` writes the RON file. The app effect persists a reset in the same
//! click; the Preferences page marks its own snapshot stale.

use std::collections::BTreeMap;

use kairos_editor_core::{SharedStyleId, StyleValue};
use kairos_editor_widgets::color_picker::ColorPickerStyle;
use kairos_editor_widgets::modal::ModalStyle;

use crate::skin::StyleField;
use crate::toolbar::style::ToolBarStyle;

use super::catalog::{SharedStyleDef, StyleCatalog};
use super::color_picker::ColorPickerSkin;
use super::{
    COLOR_PICKER, EDITOR_FIELD, EditorFieldStyle, LIST, ListStyle, MODAL, ModalSkin, SharedStyle,
    TOOLBAR,
};

/// One shared style as the Preferences panel sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct SharedSkin {
    pub id: SharedStyleId,
    pub title: String,
    pub file: &'static str,
    pub fields: Vec<StyleField>,
}

struct SharedStyleEntry {
    def: &'static SharedStyleDef,
    value: Box<dyn SharedStyle>,
}

/// One live value per registered shared style.
///
/// The value is selected for the current theme at load time (the editor pins
/// dark today). The Preferences page keeps its own field snapshot, so a reset
/// marks that page stale; the registry only changes the live value.
pub struct SharedStyles {
    entries: BTreeMap<SharedStyleId, SharedStyleEntry>,
}

impl SharedStyles {
    /// Load every registered style from `Preferences/Styles`; a missing file is
    /// seeded from the code defaults, a broken one logs and uses them.
    pub fn load(catalog: &StyleCatalog, theme: egui::Theme) -> Self {
        let mut entries = BTreeMap::new();
        for def in catalog.defs() {
            entries.insert(
                def.id,
                SharedStyleEntry {
                    def,
                    value: (def.load)(theme),
                },
            );
        }
        Self { entries }
    }

    /// Code defaults for every registered style, without touching the disk.
    pub fn defaults(catalog: &StyleCatalog, theme: egui::Theme) -> Self {
        let mut entries = BTreeMap::new();
        for def in catalog.defs() {
            entries.insert(
                def.id,
                SharedStyleEntry {
                    def,
                    value: (def.defaults)(theme),
                },
            );
        }
        Self { entries }
    }

    /// A process-lifetime default registry, so [`Frame::with_host`](crate::Frame::with_host)
    /// can be used in tests and embedders that do not own shared styles.
    pub fn default_ref() -> &'static SharedStyles {
        static DEFAULT: std::sync::OnceLock<SharedStyles> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(SharedStyles::default)
    }

    pub fn get<T: SharedStyle>(&self, id: SharedStyleId) -> Option<&T> {
        self.entries.get(&id)?.value.as_any().downcast_ref::<T>()
    }

    pub fn toolbar(&self) -> Option<&ToolBarStyle> {
        self.get(TOOLBAR)
    }

    pub fn color_picker(&self) -> Option<&ColorPickerSkin> {
        self.get(COLOR_PICKER)
    }

    /// The color picker's runtime tokens; `None` when the style is not
    /// registered (a test catalog without it).
    pub fn editor_field(&self) -> Option<&EditorFieldStyle> {
        self.get(EDITOR_FIELD)
    }

    /// The reorderable list's live skin; `None` when it is not registered (a
    /// test catalog without it).
    pub fn list_style(&self) -> Option<&ListStyle> {
        self.get(LIST)
    }

    /// The modal dialog's live skin; `None` when it is not registered (a test
    /// catalog without it).
    pub fn modal(&self) -> Option<&ModalSkin> {
        self.get(MODAL)
    }

    pub fn color_picker_style(&self) -> Option<ColorPickerStyle> {
        self.color_picker().map(ColorPickerSkin::tokens)
    }

    /// The modal dialog's runtime tokens; `None` when the style is not
    /// registered (a test catalog without it).
    pub fn modal_style(&self) -> Option<ModalStyle> {
        self.modal().map(ModalSkin::tokens)
    }

    /// Field tables for the Preferences panel, in catalog order.
    pub fn skins(&self) -> Vec<SharedSkin> {
        self.entries
            .iter()
            .map(|(id, entry)| SharedSkin {
                id: *id,
                title: entry.def.title.to_owned(),
                file: entry.def.file,
                fields: entry.value.style_fields(),
            })
            .collect()
    }

    /// Write one field on one live style. Returns whether the path matched.
    pub fn set_field(&mut self, id: SharedStyleId, path: &str, value: &StyleValue) -> bool {
        self.entries
            .get_mut(&id)
            .is_some_and(|entry| entry.value.set_style_field(path, value))
    }

    /// Insert, remove or move one entry of a list token on the live style.
    pub fn edit_list(
        &mut self,
        id: SharedStyleId,
        path: &str,
        edit: kairos_editor_core::ListEdit,
    ) -> bool {
        self.entries
            .get_mut(&id)
            .is_some_and(|entry| entry.value.edit_style_list(path, edit))
    }

    /// Persist one live style into its RON file.
    pub fn save(&self, id: SharedStyleId, theme: egui::Theme) -> Result<(), String> {
        let Some(entry) = self.entries.get(&id) else {
            return Err(format!("shared style {id} is not registered"));
        };
        (entry.def.save)(&*entry.value, theme)
    }

    /// Replace one live style with its code defaults for `theme`.
    ///
    /// Returns whether the id is registered. The caller persists the result;
    /// this method only changes the live value.
    pub fn reset(&mut self, id: SharedStyleId, theme: egui::Theme) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        entry.value = (entry.def.defaults)(theme);
        true
    }

    /// Warm every style's runtime images (toolbar logo, eyedropper icon).
    pub fn preload(&self, ctx: &egui::Context) {
        for entry in self.entries.values() {
            entry.value.preload(ctx);
        }
    }
}

impl Default for SharedStyles {
    fn default() -> Self {
        Self::defaults(&StyleCatalog::builtin(), egui::Theme::Dark)
    }
}

#[cfg(test)]
mod tests {
    use kairos_color::Color;

    use kairos_editor_core::ListEdit;

    use super::*;
    use crate::shared_style::{
        COLOR_PICKER, EDITOR_FIELD, EditorFonts, FONTS, LIST, MODAL, TOOLBAR,
    };

    #[test]
    fn the_registry_exposes_a_page_for_every_shared_style() {
        let shared = SharedStyles::default();
        let skins = shared.skins();

        assert!(skins.iter().any(|skin| skin.id == TOOLBAR));
        assert!(skins.iter().any(|skin| skin.id == COLOR_PICKER));
        assert!(skins.iter().any(|skin| skin.id == EDITOR_FIELD));
        assert!(skins.iter().any(|skin| skin.id == LIST));
        assert!(skins.iter().any(|skin| skin.id == FONTS));
        assert!(shared.toolbar().is_some());
        assert!(shared.get::<EditorFonts>(FONTS).is_some());
        assert!(shared.color_picker().is_some());
        assert!(shared.color_picker_style().is_some());
        assert!(shared.list_style().is_some());
        assert!(shared.modal().is_some());
        assert!(shared.modal_style().is_some());
        assert!(
            skins
                .iter()
                .all(|skin| !skin.fields.is_empty() && skin.file.ends_with(".ron")),
            "every shared page has a file and at least one editable token"
        );

        let toolbar = skins
            .iter()
            .find(|skin| skin.id == TOOLBAR)
            .expect("toolbar page");
        assert!(toolbar.fields.iter().any(|field| field.path == "height"));
        assert!(
            toolbar
                .fields
                .iter()
                .any(|field| field.path == "icons.about"),
            "nested toolbar groups stay addressable"
        );

        let picker = skins
            .iter()
            .find(|skin| skin.id == COLOR_PICKER)
            .expect("picker page");
        assert!(picker.fields.iter().any(|field| field.path == "accent"));
        assert!(
            picker
                .fields
                .iter()
                .any(|field| field.path == "eyedropper_icon")
        );

        let modal = skins
            .iter()
            .find(|skin| skin.id == MODAL)
            .expect("modal page");
        assert_eq!(modal.title, "Modal");
        assert_eq!(modal.file, "Modal.ron");
        assert!(modal.fields.iter().any(|field| field.path == "title_bar"));
        assert!(modal.fields.iter().any(|field| field.path == "button_fill"));

        let fonts = skins
            .iter()
            .find(|skin| skin.id == FONTS)
            .expect("fonts page");
        assert!(
            fonts
                .fields
                .iter()
                .any(|field| field.path == "proportional.0.name"),
            "font entries are indexed token groups"
        );
    }

    /// A list token grows, shrinks and reorders on the live style; the panel
    /// path and the shared-style path share the same validation.
    #[test]
    fn a_list_edit_lands_on_the_live_style() {
        let mut shared = SharedStyles::default();
        let entries = shared
            .get::<EditorFonts>(FONTS)
            .expect("fonts style")
            .proportional
            .len();

        assert!(shared.edit_list(FONTS, "proportional", ListEdit::Insert { index: 0 }));
        assert_eq!(
            shared
                .get::<EditorFonts>(FONTS)
                .expect("fonts")
                .proportional
                .len(),
            entries + 1
        );
        assert!(shared.edit_list(FONTS, "proportional", ListEdit::Remove { index: 0 }));
        assert_eq!(
            shared
                .get::<EditorFonts>(FONTS)
                .expect("fonts")
                .proportional
                .len(),
            entries
        );
        assert!(!shared.edit_list(FONTS, "proportional", ListEdit::Remove { index: 99 }));
        assert!(!shared.edit_list(FONTS, "missing", ListEdit::Insert { index: 0 }));
    }

    #[test]
    fn a_field_write_lands_on_the_live_style() {
        let mut shared = SharedStyles::default();
        let accent = Color::from_srgb8([1, 2, 3, 255]);

        assert!(shared.set_field(COLOR_PICKER, "accent", &StyleValue::Color(accent)));
        assert_eq!(shared.color_picker().expect("picker").accent, accent);
        assert!(!shared.set_field(COLOR_PICKER, "missing", &StyleValue::F32(1.0)));
        assert!(
            !shared.set_field(TOOLBAR, "accent", &StyleValue::Color(accent)),
            "a path from another style does not match"
        );
    }

    /// Reset restores the code defaults; the Preferences page marks its own
    /// snapshot stale so the next pass shows them.
    #[test]
    fn reset_restores_the_code_defaults() {
        let mut shared = SharedStyles::default();
        let default_height = shared.toolbar().expect("toolbar").height;

        assert!(shared.set_field(TOOLBAR, "height", &StyleValue::F32(99.0)));

        assert!(shared.reset(TOOLBAR, egui::Theme::Dark));
        assert_eq!(shared.toolbar().expect("toolbar").height, default_height);
        assert!(
            !shared.reset(
                kairos_editor_core::SharedStyleId("not-registered"),
                egui::Theme::Dark
            ),
            "an unknown id is a no-op"
        );
    }

    /// Save writes the live value to the style's RON file.
    #[test]
    fn save_writes_the_live_style_to_its_file() {
        let root = std::env::temp_dir().join(format!(
            "kairos-shared-style-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        kairos_editor_widgets::style::set_style_root(&root);

        let mut shared = SharedStyles::default();
        assert!(shared.set_field(TOOLBAR, "height", &StyleValue::F32(30.0)));
        shared
            .save(TOOLBAR, egui::Theme::Dark)
            .expect("save toolbar");
        let text = std::fs::read_to_string(root.join("ToolBar.ron")).expect("read toolbar");
        assert!(
            text.contains("height: 30.0"),
            "the saved value is on disk: {text}"
        );

        kairos_editor_widgets::style::set_style_root("Preferences/Styles");
        let _ = std::fs::remove_dir_all(&root);
    }
}
