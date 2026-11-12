use egui::{Rect, pos2, vec2};
use kairos_editor_docking::{
    docker::DockState,
    split::Fraction,
    surface::SurfaceIndex,
    tree::{NodeIndex, TabIndex},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Id(u32);

#[test]
fn split_append_and_remove_by_id() {
    let mut dock: DockState<Id> = DockState::new();
    let [left, _right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.7), vec![Id(2)]);
    dock.main_surface_mut()[left].append_tab(Id(1));

    assert!(dock.contains_tab(Id(1)));
    assert!(dock.contains_tab(Id(2)));
    assert_eq!(dock.tabs().len(), 2);

    assert!(dock.remove_tab_by_id(Id(1)));
    assert!(!dock.contains_tab(Id(1)));
    assert_eq!(dock.tabs(), vec![Id(2)]);
}

#[test]
fn append_tab_goes_to_focused_leaf() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    assert!(dock.contains_tab(Id(1)));

    let [left, _right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.5), vec![Id(2)]);
    dock.set_focused_node_and_surface((SurfaceIndex::main(), left));
    dock.append_tab(Id(3));

    assert_eq!(dock.focused_tab(), Some(Id(3)));
    assert_eq!(dock.tabs().len(), 3);
}

#[test]
fn detach_creates_a_window_surface_and_keeps_the_tab() {
    let mut dock: DockState<Id> = DockState::new();
    let [left, _right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.5), vec![Id(1)]);
    dock.main_surface_mut()[left].append_tab(Id(2));

    let window = dock.detach_tab(
        (SurfaceIndex::main(), left, TabIndex(0)),
        Rect::from_min_size(pos2(10.0, 10.0), vec2(320.0, 240.0)),
    );

    assert!(window.is_some());
    assert!(dock.contains_tab(Id(2)));
    assert_eq!(dock.valid_surface_indices().len(), 2);
}

#[test]
fn focused_tab_is_the_active_tab_of_the_focused_leaf() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    dock.append_tab(Id(2));
    dock.set_focused_node_and_surface((SurfaceIndex::main(), NodeIndex::root()));

    assert_eq!(dock.focused_tab(), Some(Id(2)));
}
