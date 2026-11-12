//! Open/focus intent semantics, independent of egui and the dock.

use kairos_editor_core::{LayoutState, PanelId, PanelInstanceId};

const SCENE: PanelId = PanelId("scene");
const INSPECTOR: PanelId = PanelId("inspector");

#[test]
fn open_close_toggle_track_the_open_set() {
    let mut layout = LayoutState::default();

    assert!(!layout.is_open(SCENE));
    layout.open(SCENE);
    assert!(layout.is_open(SCENE));

    layout.toggle(INSPECTOR);
    assert!(layout.is_open(INSPECTOR));
    layout.toggle(INSPECTOR);
    assert!(!layout.is_open(INSPECTOR));

    layout.close(SCENE);
    assert!(!layout.is_open(SCENE));
}

#[test]
fn closing_the_focused_panel_clears_focus() {
    let mut layout = LayoutState::default();
    layout.open(SCENE);
    layout.open(INSPECTOR);
    layout.set_focus(Some(PanelInstanceId::new(INSPECTOR, 3)));

    // Closing another kind keeps the focus.
    layout.close(SCENE);
    assert_eq!(layout.focus(), Some(PanelInstanceId::new(INSPECTOR, 3)));

    layout.close(INSPECTOR);
    assert_eq!(layout.focus(), None);
}
