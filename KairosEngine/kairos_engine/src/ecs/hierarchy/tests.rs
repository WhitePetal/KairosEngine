use crate::{
    children,
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        relationship::{RelationshipHookMode, RelationshipTarget},
        spawn::{Spawn, SpawnRelated},
        world::World,
    },
};

#[derive(PartialEq, Eq, Debug)]
struct Node {
    entity: Entity,
    children: Vec<Node>,
}

impl Node {
    fn new(entity: Entity) -> Self {
        Self {
            entity,
            children: Vec::new(),
        }
    }

    fn new_with(entity: Entity, children: Vec<Node>) -> Self {
        Self { entity, children }
    }
}

fn get_hierarchy(world: &World, entity: Entity) -> Node {
    Node {
        entity,
        children: world
            .entity(entity)
            .get::<Children>()
            .map_or_else(Default::default, |c| {
                c.iter().map(|e| get_hierarchy(world, e)).collect()
            }),
    }
}

#[test]
fn hierarchy() {
    let mut world = World::new();
    let root = world.spawn_empty().id();
    let child1 = world.spawn(ChildOf(root)).id();
    let grandchild = world.spawn(ChildOf(child1)).id();
    let child2 = world.spawn(ChildOf(root)).id();

    // Spawn
    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(
            root,
            vec![
                Node::new_with(child1, vec![Node::new(grandchild)]),
                Node::new(child2)
            ]
        )
    );

    // Removal
    world.entity_mut(child1).remove::<ChildOf>();
    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(hierarchy, Node::new_with(root, vec![Node::new(child2)]));

    // Insert
    world.entity_mut(child1).insert(ChildOf(root));
    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(
            root,
            vec![
                Node::new(child2),
                Node::new_with(child1, vec![Node::new(grandchild)])
            ]
        )
    );

    // Recursive Despawn
    world.entity_mut(root).despawn();
    assert!(world.get_entity(root).is_err());
    assert!(world.get_entity(child1).is_err());
    assert!(world.get_entity(child2).is_err());
    assert!(world.get_entity(grandchild).is_err());
}

#[test]
fn with_children() {
    let mut world = World::new();
    let mut child1 = Entity::PLACEHOLDER;
    let mut child2 = Entity::PLACEHOLDER;
    let root = world
        .spawn_empty()
        .with_children(|p| {
            child1 = p.spawn_empty().id();
            child2 = p.spawn_empty().id();
        })
        .id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(root, vec![Node::new(child1), Node::new(child2)])
    );
}

#[test]
fn add_children() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let root = world.spawn_empty().add_children(&[child1, child2]).id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(root, vec![Node::new(child1), Node::new(child2)])
    );
}

#[test]
fn insert_children() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let child3 = world.spawn_empty().id();
    let child4 = world.spawn_empty().id();

    let mut entity_world_mut = world.spawn_empty();

    let first_children = entity_world_mut.add_children(&[child1, child2]);

    let root = first_children.insert_children(1, &[child3, child4]).id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(
            root,
            vec![
                Node::new(child1),
                Node::new(child3),
                Node::new(child4),
                Node::new(child2)
            ]
        )
    );
}

#[test]
fn insert_child() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let child3 = world.spawn_empty().id();

    let mut entity_world_mut = world.spawn_empty();

    let first_children = entity_world_mut.add_children(&[child1, child2]);

    let root = first_children.insert_child(1, child3).id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(
            root,
            vec![Node::new(child1), Node::new(child3), Node::new(child2)]
        )
    );
}

// regression test for https://github.com/bevyengine/bevy/pull/19134
#[test]
fn insert_children_index_bound() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let child3 = world.spawn_empty().id();
    let child4 = world.spawn_empty().id();

    let mut entity_world_mut = world.spawn_empty();

    let first_children = entity_world_mut.add_children(&[child1, child2]).id();
    let hierarchy = get_hierarchy(&world, first_children);
    assert_eq!(
        hierarchy,
        Node::new_with(first_children, vec![Node::new(child1), Node::new(child2)])
    );

    let root = world
        .entity_mut(first_children)
        .insert_children(usize::MAX, &[child3, child4])
        .id();
    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(
            root,
            vec![
                Node::new(child1),
                Node::new(child2),
                Node::new(child3),
                Node::new(child4),
            ]
        )
    );
}

#[test]
fn detach_children() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let child3 = world.spawn_empty().id();
    let child4 = world.spawn_empty().id();

    let mut root = world.spawn_empty();
    root.add_children(&[child1, child2, child3, child4]);
    root.detach_children(&[child2, child3]);
    let root = root.id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(root, vec![Node::new(child1), Node::new(child4)])
    );
}

#[test]
fn detach_child() {
    let mut world = World::new();
    let child1 = world.spawn_empty().id();
    let child2 = world.spawn_empty().id();
    let child3 = world.spawn_empty().id();

    let mut root = world.spawn_empty();
    root.add_children(&[child1, child2, child3]);
    root.detach_child(child2);
    let root = root.id();

    let hierarchy = get_hierarchy(&world, root);
    assert_eq!(
        hierarchy,
        Node::new_with(root, vec![Node::new(child1), Node::new(child3)])
    );
}

#[test]
fn self_parenting_invalid() {
    let mut world = World::new();
    let id = world.spawn_empty().id();
    world.entity_mut(id).insert(ChildOf(id));
    assert!(
        world.entity(id).get::<ChildOf>().is_none(),
        "invalid ChildOf relationships should self-remove"
    );
}

#[test]
fn missing_parent_invalid() {
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    world.entity_mut(parent).despawn();
    let id = world.spawn(ChildOf(parent)).id();
    assert!(
        world.entity(id).get::<ChildOf>().is_none(),
        "invalid ChildOf relationships should self-remove"
    );
}

#[test]
fn reinsert_same_parent() {
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    let id = world.spawn(ChildOf(parent)).id();
    world.entity_mut(id).insert(ChildOf(parent));
    assert_eq!(
        Some(&ChildOf(parent)),
        world.entity(id).get::<ChildOf>(),
        "ChildOf should still be there"
    );
}

#[test]
fn spawn_children() {
    let mut world = World::new();
    let id = world.spawn(Children::spawn((Spawn(()), Spawn(())))).id();
    assert_eq!(world.entity(id).get::<Children>().unwrap().len(), 2,);
}

#[test]
fn spawn_many_children() {
    let mut world = World::new();

    // ensure an empty set can be mentioned
    world.spawn(children![]);

    // 12 children should result in a flat tuple
    let id = world
        .spawn(children![(), (), (), (), (), (), (), (), (), (), (), ()])
        .id();

    assert_eq!(world.entity(id).get::<Children>().unwrap().len(), 12,);

    // 13 will start nesting, but should nonetheless produce a flat hierarchy
    let id = world
        .spawn(children![
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
            (),
        ])
        .id();

    assert_eq!(world.entity(id).get::<Children>().unwrap().len(), 13,);
}

#[test]
fn replace_children() {
    let mut world = World::new();
    let parent = world.spawn(Children::spawn((Spawn(()), Spawn(())))).id();
    let &[child_a, child_b] = &world.entity(parent).get::<Children>().unwrap().0[..] else {
        panic!("Tried to spawn 2 children on an entity and didn't get 2 children");
    };

    let child_c = world.spawn_empty().id();

    world
        .entity_mut(parent)
        .replace_children(&[child_a, child_c]);

    let children = world.entity(parent).get::<Children>().unwrap();

    assert!(children.contains(&child_a));
    assert!(children.contains(&child_c));
    assert!(!children.contains(&child_b));

    assert_eq!(
        world.entity(child_a).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_c).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert!(world.entity(child_b).get::<ChildOf>().is_none());
}

#[test]
fn replace_children_with_nothing() {
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();

    world.entity_mut(parent).add_children(&[child_a, child_b]);

    assert_eq!(world.entity(parent).get::<Children>().unwrap().len(), 2);

    world.entity_mut(parent).replace_children(&[]);

    assert!(world.entity(child_a).get::<ChildOf>().is_none());
    assert!(world.entity(child_b).get::<ChildOf>().is_none());
}

#[test]
fn insert_same_child_twice() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child = world.spawn_empty().id();

    world.entity_mut(parent).add_child(child);
    world.entity_mut(parent).add_child(child);

    let children = world.get::<Children>(parent).unwrap();
    assert_eq!(children.0, [child]);
    assert_eq!(
        world.entity(child).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
}

#[test]
fn replace_with_difference() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();
    let child_c = world.spawn_empty().id();
    let child_d = world.spawn_empty().id();

    // Test inserting new relations
    world.entity_mut(parent).replace_children_with_difference(
        &[],
        &[child_a, child_b],
        &[child_a, child_b],
    );

    assert_eq!(
        world.entity(child_a).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_b).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(parent).get::<Children>().unwrap().0,
        [child_a, child_b]
    );

    // Test replacing relations and changing order
    world.entity_mut(parent).replace_children_with_difference(
        &[child_b],
        &[child_d, child_c, child_a],
        &[child_c, child_d],
    );
    assert_eq!(
        world.entity(child_a).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_c).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_d).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(parent).get::<Children>().unwrap().0,
        [child_d, child_c, child_a]
    );
    assert!(!world.entity(child_b).contains::<ChildOf>());

    // Test removing relationships
    world.entity_mut(parent).replace_children_with_difference(
        &[child_a, child_d, child_c],
        &[],
        &[],
    );
    assert!(!world.entity(parent).contains::<Children>());
    assert!(!world.entity(child_a).contains::<ChildOf>());
    assert!(!world.entity(child_b).contains::<ChildOf>());
    assert!(!world.entity(child_c).contains::<ChildOf>());
    assert!(!world.entity(child_d).contains::<ChildOf>());
}

#[test]
fn replace_with_difference_on_empty() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();

    world
        .entity_mut(parent)
        .replace_children_with_difference(&[child_a], &[], &[]);

    assert!(!world.entity(parent).contains::<Children>());
    assert!(!world.entity(child_a).contains::<ChildOf>());
}

#[test]
fn replace_with_difference_totally_new_children() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();
    let child_c = world.spawn_empty().id();
    let child_d = world.spawn_empty().id();

    // Test inserting new relations
    world.entity_mut(parent).replace_children_with_difference(
        &[],
        &[child_a, child_b],
        &[child_a, child_b],
    );

    assert_eq!(
        world.entity(child_a).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_b).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(parent).get::<Children>().unwrap().0,
        [child_a, child_b]
    );

    // Test replacing relations and changing order
    world.entity_mut(parent).replace_children_with_difference(
        &[child_b, child_a],
        &[child_d, child_c],
        &[child_c, child_d],
    );
    assert_eq!(
        world.entity(child_c).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(child_d).get::<ChildOf>().unwrap(),
        &ChildOf(parent)
    );
    assert_eq!(
        world.entity(parent).get::<Children>().unwrap().0,
        [child_d, child_c]
    );
    assert!(!world.entity(child_a).contains::<ChildOf>());
    assert!(!world.entity(child_b).contains::<ChildOf>());
}

#[test]
fn replace_children_order() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();
    let child_c = world.spawn_empty().id();
    let child_d = world.spawn_empty().id();

    let initial_order = [child_a, child_b, child_c, child_d];
    world.entity_mut(parent).add_children(&initial_order);

    assert_eq!(
        world.entity_mut(parent).get::<Children>().unwrap().0,
        initial_order
    );

    let new_order = [child_d, child_b, child_a, child_c];
    world.entity_mut(parent).replace_children(&new_order);

    assert_eq!(world.entity(parent).get::<Children>().unwrap().0, new_order);
}

#[test]
#[should_panic]
#[cfg_attr(
    not(debug_assertions),
    ignore = "we don't check invariants if debug assertions are off"
)]
fn replace_diff_invariant_overlapping_unrelate_with_relate() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();

    world
        .entity_mut(parent)
        .replace_children_with_difference(&[], &[child_a], &[child_a]);

    // This should panic
    world
        .entity_mut(parent)
        .replace_children_with_difference(&[child_a], &[child_a], &[]);
}

#[test]
#[should_panic]
#[cfg_attr(
    not(debug_assertions),
    ignore = "we don't check invariants if debug assertions are off"
)]
fn replace_diff_invariant_overlapping_unrelate_with_newly() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();

    world
        .entity_mut(parent)
        .replace_children_with_difference(&[], &[child_a], &[child_a]);

    // This should panic
    world.entity_mut(parent).replace_children_with_difference(
        &[child_b],
        &[child_a, child_b],
        &[child_b],
    );
}

#[test]
#[should_panic]
#[cfg_attr(
    not(debug_assertions),
    ignore = "we don't check invariants if debug assertions are off"
)]
fn replace_diff_invariant_newly_not_subset() {
    let mut world = World::new();

    let parent = world.spawn_empty().id();
    let child_a = world.spawn_empty().id();
    let child_b = world.spawn_empty().id();

    // This should panic
    world
        .entity_mut(parent)
        .replace_children_with_difference(&[], &[child_a, child_b], &[child_a]);
}

#[test]
fn child_replace_hook_skip() {
    let mut world = World::new();
    let parent = world.spawn_empty().id();
    let other = world.spawn_empty().id();
    let child = world.spawn(ChildOf(parent)).id();
    world
        .entity_mut(child)
        .insert_with_relationship_hook_mode(ChildOf(other), RelationshipHookMode::Skip);
    assert_eq!(
        &**world.entity(parent).get::<Children>().unwrap(),
        &[child],
        "Children should still have the old value, as on_insert/on_discard didn't run"
    );
}
