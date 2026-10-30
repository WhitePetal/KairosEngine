//! # kairos-ui-widgets (throwaway prototype)
//!
//! **Pure appearance layer** for the proposed editor UI architecture
//! (`docs/plan/editor-ui-architecture/README.md` §3.1/`rewrite-plan.md` Phase 1).
//!
//! Hard rules this crate is built to prove:
//! 1. **No domain knowledge**: the only dependency is `egui`. It cannot name
//!    `ProjectState`, `Action`, `Effect` or any Kairos type — enforced by Cargo,
//!    not by discipline (see `tests/purity.rs`).
//! 2. **No IO**: widgets draw and return data; they never read/write files or
//!    spawn tasks.
//! 3. **Theme-driven**: every color/size comes from `theme::Theme` tokens.
//! 4. **Explicit ephemeral state only**: any state a widget needs across frames
//!    lives in `egui::Memory` under stable ids (built-in widgets do this too).
//!    Domain state and view state are passed in by the caller.
//!
//! Layer mapping:
//! ```text
//! StyleState --(host maps once/frame)--> Theme tokens
//!                                           |
//!          atoms (impl egui::Widget) <-----+
//!                                           |
//!          compounds (show(&mut self, ui) -> Output, plain data)
//!                                           |
//!          feature components (editor-components crate):
//!             domain state -> widget props   and   widget output -> Action
//! ```

pub mod atoms;
pub mod compounds;
pub mod icon;
pub mod testing;
pub mod theme;

pub use atoms::{
    Badge, BadgeTone, Button, ButtonSize, ButtonVariant, Checkbox, ColorSwatch, ComboSelect,
    Heading, IconButton, Label, LabelTone, NumberRow, Separator, SliderRow, Spinner, TextInput,
};
pub use compounds::{
    Breadcrumb, BreadcrumbOutput, ConfirmDialog, ConfirmDialogOutput, ContextMenu,
    ContextMenuItem, ContextMenuOutput, EmptyState, EmptyStateOutput, Expander, FormSection,
    FormSectionOutput, ListItem, ListItemOutput, PanelChrome, PanelChromeOutput, PropertyRow,
    PropertyRowOutput, SearchBox, SearchBoxOutput, StatusBar, StatusBarOutput, StatusTone, Toast,
    ToastOutput, ToastTone, Toolbar, ToolbarItem, ToolbarItemKind, ToolbarOutput, TreeOutput,
    TreeRow, TreeView,
};
pub use icon::{GlyphIcons, IconId, IconProvider};
pub use theme::Theme;
