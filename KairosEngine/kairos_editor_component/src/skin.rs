//! Style tokens the editor can edit: the vocabulary a style type and its
//! Preferences page share.
//!
//! A style is a plain Rust value that draws itself
//! ([`StyleRow`](crate::style_row::StyleRow) walking into
//! `kairos_editor_widgets::form::FormRows`); the Preferences panel holds a typed
//! copy and hands the whole value back when it changed. This module keeps the
//! pieces *around* that value:
//!
//! * [`SkinId`] — which owner a page belongs to (a panel instance or a shared
//!   style), so one page list can hold both kinds;
//! * [`PageMeta`] — the owner id, the title and the file shown in the page
//!   header, without a style copy: what the page list is built from and what
//!   the app compares to keep its snapshot current;
//! * [`PageSkin`] — one page: the metadata plus the self-drawing copy of the
//!   style, which is what the panel clones when the user switches pages;
//! * [`StyleEnum`] — the token protocol an enum needs to be a variant menu; the
//!   trait itself is a row-protocol concept and lives with the form widget,
//!   re-exported here so the derive's generated path stays stable;
//! * [`LIST_MAX`] — a list token's growth cap;
//! * [`save_skin`] — persist one live style into its RON file.
//!
//! Live apply and persistence are two separate steps: the page hands the value
//! over as it changes (live preview), and the app writes the file once per
//! gesture (`Commit` on pointer release, typed edit end, page switch, close).
//! This module only describes and persists; it does not know about the
//! Preferences panel.

use kairos_editor_core::{PanelId, SharedStyleId};
use kairos_editor_widgets::style::{Style, StyleFile};

/// An enum token: its variants, the active one, and how to rebuild a variant.
///
/// The trait is the row protocol's (`kairos_editor_widgets::form`, the same
/// module that owns [`StyleRow`](crate::style_row::StyleRow) and the menu row),
/// because a variant menu is a widget concern. It is re-exported under this path
/// because `#[derive(StyleRow)]` generates its impl there, next to the style
/// vocabulary the menu labels come from.
pub use kairos_editor_widgets::form::StyleEnum;

/// Which owner a typed page belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkinId {
    Panel(PanelId),
    Shared(SharedStyleId),
}

impl From<PanelId> for SkinId {
    fn from(id: PanelId) -> Self {
        Self::Panel(id)
    }
}

impl From<SharedStyleId> for SkinId {
    fn from(id: SharedStyleId) -> Self {
        Self::Shared(id)
    }
}

/// One page's owner, heading and file, without the style value.
///
/// Building this costs no style copy, so the app can rebuild the list every
/// frame and compare it with the snapshot it holds; only a changed list makes it
/// copy a value. The panel needs the same three pieces to draw the page list and
/// the heading, so the snapshot carries them next to the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageMeta {
    pub id: SkinId,
    /// Page heading: the panel's `Panel::title`, or the shared style's title.
    pub title: String,
    /// File name under `Preferences/Styles/`.
    pub file: &'static str,
}

/// One page a style draws itself: its metadata and a typed copy of the owner's
/// style.
///
/// The app holds one of these per page while Preferences is open and hands the
/// list to the panel; the panel copies the one page the user selected and hands
/// a new value back when it changes (`StyleAction::Replace`). Nothing here
/// describes tokens.
pub struct PageSkin {
    pub id: SkinId,
    /// Page heading: the panel's `Panel::title`, or the shared style's title.
    pub title: String,
    /// File name under `Preferences/Styles/`.
    pub file: &'static str,
    /// The style value the page draws.
    pub page: Box<dyn crate::style_row::StylePage>,
}

/// Persist one panel's live style into its RON file.
///
/// The panel keeps only the skin it draws with. The other half is read back
/// from the file first, so editing the dark skin does not throw away a
/// hand-edited light one; a missing file is seeded from the code defaults.
pub fn save_skin<S: Style>(style: &S, theme: egui::Theme) -> Result<(), String> {
    let path = StyleFile::<S>::path();
    let mut file = if path.is_file() {
        StyleFile::<S>::load_from(&path)?
    } else {
        StyleFile::<S>::defaults()
    };
    match theme {
        egui::Theme::Light => file.light = style.clone(),
        egui::Theme::Dark => file.dark = style.clone(),
    }
    file.save_to(&path)
}
