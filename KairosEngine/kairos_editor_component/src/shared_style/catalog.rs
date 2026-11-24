//! Shared-style catalog: the shell-owned styles and the RON files behind them.
//!
//! Every style module submits a [`SharedStyleDef`] through
//! `#[SharedStyle(...)]`; the linker concatenates the entries and
//! [`StyleCatalog::builtin`] collects them once, sorted by id. The same
//! inventory mechanism as `#[panel]`.
//!
//! Collection order is link order, which is not source order, so `builtin`
//! sorts. Tests and embedders that need a different set pass an explicit slice
//! to [`StyleCatalog::new`].

use std::sync::OnceLock;

use kairos_editor_core::SharedStyleId;
use kairos_editor_widgets::style::{Style, StyleFile};

use super::SharedStyle;

/// One registered shared style. The `StyleRow` derive submits it for a
/// `#[SharedStyle(...)]` declaration.
#[derive(Clone, Copy)]
pub struct SharedStyleDef {
    /// Stable key used by actions and the registry.
    pub id: SharedStyleId,
    /// Page heading in Preferences.
    pub title: &'static str,
    /// File under `Preferences/Styles`, from [`Style::FILE`].
    pub file: &'static str,
    /// Load the file, falling back to the code defaults on error.
    pub load: fn(egui::Theme) -> Box<dyn SharedStyle>,
    /// Code defaults, for tests and the no-IO fallback.
    pub defaults: fn(egui::Theme) -> Box<dyn SharedStyle>,
    /// Persist the live value into its file.
    pub save: fn(&dyn SharedStyle, egui::Theme) -> Result<(), String>,
}

impl SharedStyleDef {
    pub const fn new<S: Style + SharedStyle>(id: SharedStyleId, title: &'static str) -> Self {
        Self {
            id,
            title,
            file: S::FILE,
            load: load_shared::<S>,
            defaults: default_shared::<S>,
            save: save_shared::<S>,
        }
    }
}

// Bare type: `#[SharedStyle(...)]` submits a definition from the style module.
inventory::collect!(SharedStyleDef);

/// The shared styles this build declares, sorted by id.
///
/// [`StyleCatalog::builtin`] returns them sorted; a catalog built with
/// [`StyleCatalog::new`] keeps the slice order.
#[derive(Clone, Copy)]
pub struct StyleCatalog {
    defs: &'static [SharedStyleDef],
}

impl StyleCatalog {
    pub const fn new(defs: &'static [SharedStyleDef]) -> Self {
        Self { defs }
    }

    /// Every shared style submitted through the derive macro.
    ///
    /// Collected once per process and sorted by id, because inventory
    /// iteration follows link order, which is not source order. Two
    /// submissions with the same id are a bug; debug builds assert it.
    pub fn builtin() -> Self {
        static COLLECTED: OnceLock<Vec<SharedStyleDef>> = OnceLock::new();
        let defs = COLLECTED.get_or_init(|| {
            let mut defs: Vec<SharedStyleDef> = inventory::iter::<SharedStyleDef>
                .into_iter()
                .copied()
                .collect();
            defs.sort_by_key(|def| def.id);
            debug_assert!(
                defs.windows(2).all(|pair| pair[0].id != pair[1].id),
                "two shared styles submitted the same SharedStyleId"
            );
            defs
        });
        Self::new(defs.as_slice())
    }

    pub fn get(&self, id: SharedStyleId) -> Option<&'static SharedStyleDef> {
        self.defs.iter().find(|def| def.id == id)
    }

    pub fn defs(&self) -> impl Iterator<Item = &'static SharedStyleDef> + '_ {
        self.defs.iter()
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

fn load_shared<S: Style + SharedStyle>(theme: egui::Theme) -> Box<dyn SharedStyle> {
    let file = match StyleFile::<S>::load_or_default() {
        Ok(file) => file,
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "shared style {}: {error}",
                S::FILE
            );
            StyleFile::<S>::defaults()
        }
    };
    match theme {
        egui::Theme::Light => Box::new(file.light),
        egui::Theme::Dark => Box::new(file.dark),
    }
}

fn default_shared<S: Style + SharedStyle>(theme: egui::Theme) -> Box<dyn SharedStyle> {
    match theme {
        egui::Theme::Light => Box::new(S::light()),
        egui::Theme::Dark => Box::new(S::dark()),
    }
}

fn save_shared<S: Style + SharedStyle>(
    style: &dyn SharedStyle,
    theme: egui::Theme,
) -> Result<(), String> {
    let value = style
        .as_any()
        .downcast_ref::<S>()
        .ok_or_else(|| format!("shared style {}: wrong value type", S::FILE))?;
    crate::skin::save_skin(value, theme)
}

#[cfg(test)]
mod tests {
    use crate::shared_style::{COLOR_PICKER, EDITOR_FIELD, FONTS, LIST, TOOLBAR};

    use super::StyleCatalog;

    #[test]
    fn the_builtin_catalog_registers_the_editor_chrome() {
        let catalog = StyleCatalog::builtin();

        for id in [TOOLBAR, COLOR_PICKER, EDITOR_FIELD, LIST, FONTS] {
            assert!(
                catalog.get(id).is_some(),
                "shared style {id} not registered"
            );
        }
    }
}
