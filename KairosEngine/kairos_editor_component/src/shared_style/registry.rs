//! The app-owned live shared styles.
//!
//! The composition root loads one value per registered style at boot and hands
//! [`SharedStyles`] to every panel through [`Frame::shared`](crate::Frame). The
//! registry is also the write target of the separate shared-style action path:
//! `Replace` swaps the live value for the page's typed copy, `Reset` restores
//! the code defaults, and `Persist` writes the RON file. The app effect persists
//! a reset in the same click; the Preferences page re-reads the owner's value on
//! the next pass.

use std::collections::BTreeMap;

use kairos_editor_core::SharedStyleId;
use kairos_editor_widgets::color_picker::ColorPickerStyle;
use kairos_editor_widgets::modal::ModalStyle;

use crate::toolbar::style::ToolBarStyle;

use super::catalog::{SharedStyleDef, StyleCatalog};
use super::color_picker::ColorPickerSkin;
use super::{
    COLOR_PICKER, EDITOR_FIELD, EditorFieldStyle, LIST, ListStyle, MODAL, ModalSkin, SharedStyle,
    TOOLBAR,
};

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

    /// One page's metadata per registered style, in catalog order.
    ///
    /// No value is copied: the app rebuilds this every frame while Preferences
    /// is open and compares it with the snapshot it holds.
    pub fn page_metas(&self) -> Vec<crate::skin::PageMeta> {
        self.entries
            .iter()
            .map(|(id, entry)| crate::skin::PageMeta {
                id: crate::skin::SkinId::Shared(*id),
                title: entry.def.title.to_owned(),
                file: entry.def.file,
            })
            .collect()
    }

    /// One registered style, copied as a self-drawing page.
    ///
    /// `None` when `id` is not registered. The app takes this copy when it
    /// builds the Preferences snapshot, and again after a reset replaced the
    /// live value.
    pub fn page(&self, id: SharedStyleId) -> Option<Box<dyn crate::style_row::StylePage>> {
        self.entries.get(&id).map(|entry| entry.value.page())
    }

    /// Typed pages for every registered style, in catalog order.
    ///
    /// Built on [`Self::page_metas`] and [`Self::page`], so the list the app
    /// compares and the one it snapshots cannot disagree. Copying every style is
    /// what makes this a rebuild-only call, not a per-frame one.
    pub fn pages(&self) -> Vec<crate::skin::PageSkin> {
        self.page_metas()
            .into_iter()
            .filter_map(|meta| {
                let crate::skin::SkinId::Shared(id) = meta.id else {
                    return None;
                };
                Some(crate::skin::PageSkin {
                    id: meta.id,
                    title: meta.title,
                    file: meta.file,
                    page: self.page(id)?,
                })
            })
            .collect()
    }

    /// Replace one live style with the page's whole typed copy.
    ///
    /// Returns whether the id is registered and the box held its type. The
    /// page draws its own copy, so this is the one write for a style page.
    pub fn replace(&mut self, id: SharedStyleId, skin: Box<dyn std::any::Any>) -> bool {
        self.entries
            .get_mut(&id)
            .is_some_and(|entry| entry.value.replace(skin))
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
    use super::*;
    use crate::shared_style::{
        COLOR_PICKER, EDITOR_FIELD, EditorFonts, FONTS, LIST, MODAL, TOOLBAR,
    };
    use crate::skin::SkinId;

    /// The page hands back a whole typed value; the registry swaps the live
    /// style and rejects a box of the wrong type.
    #[test]
    fn a_whole_skin_replaces_the_live_value() {
        let mut shared = SharedStyles::default();
        let mut toolbar = shared.toolbar().expect("the toolbar is registered").clone();
        toolbar.height += 10.0;

        assert!(shared.replace(TOOLBAR, Box::new(toolbar.clone())));
        assert_eq!(
            shared.toolbar().map(|style| style.height),
            Some(toolbar.height)
        );
        assert!(
            !shared.replace(TOOLBAR, Box::new(0u8)),
            "a box of the wrong type is a no-op"
        );
        assert_eq!(
            shared.toolbar().map(|style| style.height),
            Some(toolbar.height)
        );
    }

    /// Every registered style exposes one self-drawing page, in catalog order,
    /// with the file its edits are saved to.
    #[test]
    fn the_registry_exposes_a_page_for_every_shared_style() {
        let shared = SharedStyles::default();
        let pages = shared.pages();

        for id in [TOOLBAR, COLOR_PICKER, EDITOR_FIELD, LIST, MODAL, FONTS] {
            assert!(
                pages.iter().any(|page| page.id == SkinId::Shared(id)),
                "no page for {id}"
            );
        }
        assert!(
            pages
                .iter()
                .all(|page| !page.title.is_empty() && page.file.ends_with(".ron")),
            "every page has a heading and a file"
        );
        assert!(shared.get::<EditorFonts>(FONTS).is_some());
        assert!(shared.list_style().is_some());
        assert!(shared.modal_style().is_some());
    }

    /// The list the app compares and the pages it snapshots are built from the
    /// same metadata, so they cannot disagree; a value is copied per call, on
    /// rebuild and on a page switch, not for the whole list per frame.
    #[test]
    fn the_page_list_and_its_per_page_copies_share_their_metadata() {
        let shared = SharedStyles::default();
        let metas = shared.page_metas();
        let pages = shared.pages();

        assert_eq!(metas.len(), pages.len());
        for (meta, page) in metas.iter().zip(&pages) {
            assert_eq!(meta.id, page.id);
            assert_eq!(meta.title, page.title);
            assert_eq!(meta.file, page.file);
        }
        assert!(
            shared.page(TOOLBAR).is_some(),
            "a registered style is copied on demand for the page snapshot"
        );
        assert!(
            shared
                .page(kairos_editor_core::SharedStyleId("not-registered"))
                .is_none()
        );
    }

    /// Reset restores the code defaults; the Preferences page re-reads the
    /// live value on the next pass.
    #[test]
    fn reset_restores_the_code_defaults() {
        let mut shared = SharedStyles::default();
        let default_height = shared.toolbar().expect("toolbar").height;
        let mut toolbar = shared.toolbar().expect("toolbar").clone();
        toolbar.height = 99.0;
        assert!(shared.replace(TOOLBAR, Box::new(toolbar)));
        assert_eq!(shared.toolbar().expect("toolbar").height, 99.0);

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
        let mut toolbar = shared.toolbar().expect("toolbar").clone();
        toolbar.height = 30.0;
        assert!(shared.replace(TOOLBAR, Box::new(toolbar)));
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

    /// [`Frame`](crate::Frame) borrows a style the live registry omits from
    /// this registry and expects every builtin id to be there. A filtered
    /// default registry would turn that borrow into a panic, so pin it.
    #[test]
    fn the_default_registry_holds_every_builtin_style() {
        let ids: Vec<_> = SharedStyles::default()
            .pages()
            .into_iter()
            .map(|page| page.id)
            .collect();

        for def in StyleCatalog::builtin().defs() {
            assert!(
                ids.contains(&SkinId::Shared(def.id)),
                "the default registry lacks {}",
                def.id
            );
        }
    }
}
