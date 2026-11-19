//! The app-owned live shared styles.
//!
//! The composition root loads one value per registered style at boot and hands
//! [`SharedStyles`] to every panel through [`Frame::shared`](crate::Frame). The
//! registry is also the write target of the separate shared-style action path:
//! `SetField` writes the live value, `Persist` writes the RON file, `Reload`
//! re-reads it and bumps [`SharedStyles::revision`].

use std::collections::BTreeMap;

use kairos_editor_core::{SharedStyleId, StyleValue};
use kairos_editor_widgets::color_picker::ColorPickerStyle;

use crate::skin::StyleField;
use crate::toolbar::style::ToolBarStyle;

use super::catalog::{SharedStyleDef, StyleCatalog};
use super::color_picker::ColorPickerSkin;
use super::{COLOR_PICKER, SharedStyle, TOOLBAR};

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
/// dark today). `revision` changes only when a file is re-read, so the
/// Preferences page can tell "my own edit" from "the file changed under me"
/// and refresh its local field copy only for the latter.
pub struct SharedStyles {
    entries: BTreeMap<SharedStyleId, SharedStyleEntry>,
    revision: u64,
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
        Self {
            entries,
            revision: 0,
        }
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
        Self {
            entries,
            revision: 0,
        }
    }

    /// A process-lifetime default registry, so [`Frame::with_host`](crate::Frame::with_host)
    /// can be used in tests and embedders that do not own shared styles.
    pub fn default_ref() -> &'static SharedStyles {
        static DEFAULT: std::sync::OnceLock<SharedStyles> = std::sync::OnceLock::new();
        DEFAULT.get_or_init(SharedStyles::default)
    }

    /// Changes when a shared style file is re-read.
    pub fn revision(&self) -> u64 {
        self.revision
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
    pub fn color_picker_style(&self) -> Option<ColorPickerStyle> {
        self.color_picker().map(ColorPickerSkin::tokens)
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

    /// Persist one live style into its RON file.
    pub fn save(&self, id: SharedStyleId, theme: egui::Theme) -> Result<(), String> {
        let Some(entry) = self.entries.get(&id) else {
            return Err(format!("shared style {id} is not registered"));
        };
        (entry.def.save)(&*entry.value, theme)
    }

    /// Re-read one style's file into the live value. Returns whether the id is
    /// registered; a broken file logs and falls back, like the initial load.
    pub fn reload(&mut self, id: SharedStyleId, theme: egui::Theme) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        let load = entry.def.load;
        entry.value = load(theme);
        self.revision = self.revision.wrapping_add(1);
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

    use super::*;
    use crate::shared_style::{COLOR_PICKER, TOOLBAR};

    #[test]
    fn the_registry_exposes_both_field_tables() {
        let shared = SharedStyles::default();
        let skins = shared.skins();

        assert!(skins.iter().any(|skin| skin.id == TOOLBAR));
        assert!(skins.iter().any(|skin| skin.id == COLOR_PICKER));
        assert!(shared.toolbar().is_some());
        assert!(shared.color_picker().is_some());
        assert!(shared.color_picker_style().is_some());
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
    }

    #[test]
    fn a_field_write_lands_on_the_live_style() {
        let mut shared = SharedStyles::default();
        let accent = Color::from_srgb8([1, 2, 3, 255]);

        assert!(shared.set_field(COLOR_PICKER, "accent", &StyleValue::Color(accent)));
        assert_eq!(shared.color_picker().expect("picker").accent, accent);
        assert!(!shared.set_field(COLOR_PICKER, "missing", &StyleValue::Float(1.0)));
        assert!(
            !shared.set_field(TOOLBAR, "accent", &StyleValue::Color(accent)),
            "a path from another style does not match"
        );
    }

    /// Save writes the live value, reload reads it back and bumps the revision
    /// the Preferences page uses to refresh its local field copy.
    #[test]
    fn save_and_reload_round_trip_through_the_style_root() {
        let root = std::env::temp_dir().join(format!(
            "kairos-shared-style-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        kairos_editor_widgets::style::set_style_root(&root);

        let mut shared = SharedStyles::default();
        assert!(shared.set_field(TOOLBAR, "height", &StyleValue::Float(30.0)));
        shared
            .save(TOOLBAR, egui::Theme::Dark)
            .expect("save toolbar");
        assert!(root.join("ToolBar.ron").is_file());

        assert!(shared.set_field(TOOLBAR, "height", &StyleValue::Float(10.0)));
        let revision = shared.revision();
        assert!(shared.reload(TOOLBAR, egui::Theme::Dark));
        assert_eq!(shared.toolbar().expect("toolbar").height, 30.0);
        assert_ne!(
            shared.revision(),
            revision,
            "a reload invalidates the page copy"
        );

        kairos_editor_widgets::style::set_style_root("Preferences/Styles");
        let _ = std::fs::remove_dir_all(&root);
    }
}
