//! Docked panel framework: stable ids, the [`Panel`] trait, the registry and the
//! host that draws the dock.
//!
//! # Where a panel is drawn (start here)
//!
//! There is exactly one call chain from the frame loop to a panel body:
//!
//! ```text
//! editor-app/src/app/mod.rs      impl eframe::App for EditorApp
//!   -> app/pipeline.rs           EditorApp::build_ui          (BuildUi phase)
//!     -> panel/host.rs           PanelHost::draw              (single drawing entry)
//!       -> panel/host.rs         PanelHost::draw_slot        (tab bar + active body)
//!         -> ui-widgets          TabBar::show                (tab chrome)
//!         -> panel/host.rs       PanelHost::draw_panel_body  (only Panel::draw call site)
//!           -> <dyn Panel>::draw concrete panel: project_panel.rs / viewport_panel.rs / ...
//! ```
//!
//! The app shell never calls a concrete panel directly; it hands the frame to
//! `PanelHost::draw`. A new panel needs no change here or in the app shell: it
//! only needs a `Panel` implementation and a `PanelRegistry` registration.
//!
//! # Pieces
//!
//! * [`Panel`], [`PanelInstanceId`], [`DockSlot`]: what a panel is (this file).
//! * [`PanelRegistry`]: `PanelId` -> (dock slot, factory) registration (module
//!   `registry`).
//! * [`PanelHost`]: reconciles `LayoutState::open_panels` into live instances and
//!   draws them (module `host`); [`PanelHostOutput`] is the per-frame drawing
//!   report (drawn panels, their rects, repaint request).

mod host;
mod registry;

pub use host::{PanelHost, PanelHostOutput};
pub use registry::{PanelFactory, PanelRegistry};

use editor_core_prototype::PanelId;
use egui::Ui;

use crate::frame::{Frame, UiOut};

/// Stable identity of one open panel instance.
///
/// kind names the panel implementation (a PanelId string, shared with the
/// store) and serial distinguishes several instances of the same kind. The host
/// currently opens one instance per open kind; the serial exists so view state
/// (ViewStates in the target architecture) can be keyed per instance without
/// changing this API when multi-instance docking lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PanelInstanceId {
    pub kind: PanelId,
    pub serial: u32,
}

impl PanelInstanceId {
    pub const fn new(kind: PanelId, serial: u32) -> Self {
        Self { kind, serial }
    }
}

/// One drawable panel body.
///
/// Implementations are owned by the PanelHost and live across frames. They may
/// keep UI-only view state (search text, expanded paths, ...) but never domain
/// state: reads go through Frame, writes go through UiOut as Actions.
///
/// The host owns the tab/title bar: Panel::title is drawn there, so draw must
/// render the body only.
pub trait Panel {
    /// Panel kind, matching the PanelId the factory was registered under.
    fn kind(&self) -> PanelId;

    /// Title for the host tab bar (e.g. "Project", or a document name).
    fn title(&self, frame: &Frame) -> String;

    /// Draw the panel body. Only the active panel of a slot is drawn.
    fn draw(&mut self, ui: &mut Ui, frame: &Frame, out: &mut UiOut);

    /// Whether the host tab bar shows a close affordance. Closing emits
    /// LayoutAction::ClosePanel; non-closable panels (e.g. a fixed viewport)
    /// override this to false.
    fn closable(&self) -> bool {
        true
    }

    /// Whether this panel needs a repaint every frame (animation, async data).
    /// Aggregated by PanelHost::wants_continuous_repaint.
    fn wants_continuous_repaint(&self) -> bool {
        false
    }
}

/// Docking slot of a panel kind.
///
/// Fixed four-way split for the prototype: the real dock tree can split
/// arbitrarily, but the panel framework only needs to know which edge a panel
/// is pinned to. Center is the leftover area (the 3D viewport in the app).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DockSlot {
    Left,
    Center,
    Right,
    Bottom,
}

impl DockSlot {
    /// Every slot, in host draw order: side panels first, central area last.
    pub const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Bottom, Self::Center];

    /// Side slots, drawn as egui side panels before the central area.
    const SIDES: [Self; 3] = [Self::Left, Self::Right, Self::Bottom];

    fn id_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }
}
