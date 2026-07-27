use crate::kairos_editor::ui::docking_tab::dock_state::tree::Tree;

#[test]
fn test_tabs_iter() {
    fn tabs(tree: &Tree<i32>) -> Vec<i32> {
        tree.drawers().copied().collect()
    }

    let mut tree = Tree::new(vec![1, 2, 3]);
    assert_eq!(tabs(&tree), vec![1, 2, 3]);

    tree.push_to_first_leaf(4);
    assert_eq!(tabs(&tree), vec![1, 2, 3, 4]);

    tree.push_to_first_leaf(5);
    assert_eq!(tabs(&tree), vec![1, 2, 3, 4, 5]);

    tree.push_to_focused_leaf(6);
    assert_eq!(tabs(&tree), vec![1, 2, 3, 4, 5, 6]);

    assert_eq!(tree.num_drawers(), tree.drawers().count());
}
