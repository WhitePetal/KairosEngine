use egui::{Rect, pos2, vec2};
use kairos_editor_docking::{
    docker::DockState,
    split::Fraction,
    surface::{Surface, SurfaceIndex, WindowState},
    tree::{LeafNode, Node, NodeIndex, SplitNode, TabIndex, Tree},
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

#[test]
fn focus_tab_activates_the_leaf_and_the_surface() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    dock.append_tab(Id(2));
    assert_eq!(dock.focused_tab(), None);

    assert!(dock.focus_tab(Id(1)));
    assert_eq!(dock.focused_tab(), Some(Id(1)));

    assert!(dock.focus_tab(Id(2)));
    assert_eq!(dock.focused_tab(), Some(Id(2)));
    assert!(!dock.focus_tab(Id(3)));
    assert_eq!(dock.focused_tab(), Some(Id(2)));
}

#[test]
fn open_in_window_creates_a_focused_floating_surface() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    let rect = Rect::from_min_size(pos2(40.0, 50.0), vec2(320.0, 200.0));

    let window = dock.open_in_window(Id(2), rect);

    assert!(!window.is_main());
    assert_eq!(dock.valid_surface_indices().len(), 2);
    assert!(dock.contains_tab(Id(2)));
    assert_eq!(dock.focused_tab(), Some(Id(2)));
}

// ---------------------------------------------------------------------------
// layout snapshot / restore (the app serializes; these are the docks's half)
// ---------------------------------------------------------------------------

/// A leaf with one tab, as a restored surface's tree.
fn one_tab_tree(tab: Id) -> Tree<Id> {
    Tree::from_nodes(vec![Node::leaf(tab)]).expect("a leaf is a valid tree")
}

/// Compare two trees node by node: same holes, same tabs/active tabs, same
/// split axes and fractions. Rects are recomputed every frame and stay empty in
/// a restored tree, so they are not compared.
fn assert_same_tree(restored: &Tree<Id>, original: &Tree<Id>) {
    assert_eq!(restored.nodes.len(), original.nodes.len(), "node count");
    for (index, (restored, original)) in restored.nodes.iter().zip(&original.nodes).enumerate() {
        match (restored, original) {
            (Node::Empty, Node::Empty) => {}
            (Node::Leaf(restored), Node::Leaf(original)) => {
                assert_eq!(restored.tabs, original.tabs, "leaf {index}: tabs");
                assert_eq!(restored.active, original.active, "leaf {index}: active tab");
            }
            (Node::Horizontal(restored), Node::Horizontal(original))
            | (Node::Vertical(restored), Node::Vertical(original)) => {
                assert_eq!(
                    restored.fraction.value(),
                    original.fraction.value(),
                    "split {index}: fraction"
                );
            }
            _ => panic!("node {index}: the node kind changed across the round trip"),
        }
    }
}

#[test]
fn tree_from_nodes_round_trips_a_split_layout() {
    let mut dock: DockState<Id> = DockState::new();
    dock.append_tab(Id(1));
    dock.append_tab(Id(2));
    let [left, _right] =
        dock.main_surface_mut()
            .split_right(NodeIndex::root(), Fraction::new(0.3), vec![Id(3)]);
    dock.main_surface_mut()[left].append_tab(Id(4));
    if let Some(leaf) = dock.main_surface_mut()[left].get_leaf_mut() {
        leaf.active = TabIndex(2);
    }

    let restored =
        Tree::from_nodes(dock.main_surface().nodes.clone()).expect("valid nodes restore");

    assert_same_tree(&restored, dock.main_surface());
    assert_eq!(restored.tabs_count(), 4);
    assert_eq!(
        restored.focused_leaf(),
        None,
        "focus is restored through DockState::focus_tab, not the node table"
    );
}

#[test]
fn tree_from_nodes_accepts_an_empty_dock() {
    let tree: Tree<Id> = Tree::from_nodes(Vec::new()).expect("an empty layout is a layout");
    assert!(tree.is_empty());
}

#[test]
fn tree_from_nodes_rejects_corrupt_layouts() {
    // A root hole.
    assert!(Tree::from_nodes(vec![Node::<Id>::Empty]).is_none());
    // A leaf without a tab.
    assert!(Tree::from_nodes(vec![Node::<Id>::leaf_with(Vec::new())]).is_none());
    // An active index past the tab list.
    let mut leaf = LeafNode::new(vec![Id(1)]);
    leaf.active = TabIndex(4);
    assert!(Tree::from_nodes(vec![Node::Leaf(leaf)]).is_none());
    // A non-empty node no split points at.
    assert!(Tree::from_nodes(vec![Node::leaf(Id(1)), Node::leaf(Id(2))]).is_none());
    // A split whose children are missing.
    assert!(
        Tree::from_nodes(vec![Node::<Id>::Horizontal(SplitNode::new(
            Rect::NOTHING,
            Fraction::new(0.5)
        ))])
        .is_none()
    );
}

#[test]
fn dock_from_parts_requires_the_main_surface_first() {
    let main = Surface::Main(one_tab_tree(Id(1)));
    let mut window_state = WindowState::new();
    window_state.set_position(pos2(20.0, 30.0));
    window_state.set_size(vec2(320.0, 240.0));
    let window = Surface::Window(one_tab_tree(Id(2)), window_state);

    let dock = DockState::from_parts(vec![main, window]).expect("valid surfaces");
    assert_eq!(dock.surfaces().len(), 2);
    assert!(matches!(dock.surfaces()[0], Surface::Main(_)));
    assert!(dock.contains_tab(Id(1)));
    assert!(dock.contains_tab(Id(2)));
    assert_eq!(dock.focused_tab(), None, "focus is restored separately");

    let Surface::Window(_, restored) = &dock.surfaces()[1] else {
        panic!("the second surface must be the floating window");
    };
    assert_eq!(restored.position(), Some(pos2(20.0, 30.0)));
    assert_eq!(restored.size(), Some(vec2(320.0, 240.0)));

    assert!(
        DockState::from_parts(Vec::<Surface<Id>>::new()).is_none(),
        "a dock always has a main surface"
    );
    assert!(
        DockState::from_parts(vec![Surface::Window(
            one_tab_tree(Id(3)),
            WindowState::new()
        )])
        .is_none(),
        "the main surface is always index 0"
    );
}
