// use std::sync::atomic::AtomicBool;

// use crate::ecs::{entity::Entity, hierarchy::{ChildOf, Children}, lifecycle::HookContext, relationship::RelationshipAccessor, world::{DeferredWorld, World}};

// #[test]
// fn custom_relationship() {
//     #[derive(Component)]
//     #[relationship(relationship_target = LikedBy)]
//     struct Likes(pub Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Likes)]
//     struct LikedBy(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     let b = world.spawn(Likes(a)).id();
//     let c = world.spawn(Likes(a)).id();
//     assert_eq!(world.entity(a).get::<LikedBy>().unwrap().0, &[b, c]);
// }

// #[test]
// fn self_relationship_fails_by_default() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     world.entity_mut(a).insert(Rel(a));
//     assert!(!world.entity(a).contains::<Rel>());
//     assert!(!world.entity(a).contains::<RelTarget>());
// }

// #[test]
// fn self_relationship_succeeds_with_allow_self_referential() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget, allow_self_referential)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     world.entity_mut(a).insert(Rel(a));
//     assert!(world.entity(a).contains::<Rel>());
//     assert!(world.entity(a).contains::<RelTarget>());
//     assert_eq!(world.entity(a).get::<Rel>().unwrap().get(), a);
//     assert_eq!(&*world.entity(a).get::<RelTarget>().unwrap().0, &[a]);
// }

// #[test]
// fn self_relationship_removal_with_allow_self_referential() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget, allow_self_referential)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     world.entity_mut(a).insert(Rel(a));
//     assert!(world.entity(a).contains::<Rel>());
//     assert!(world.entity(a).contains::<RelTarget>());

//     // Remove the relationship and verify cleanup
//     world.entity_mut(a).remove::<Rel>();
//     assert!(!world.entity(a).contains::<Rel>());
//     assert!(!world.entity(a).contains::<RelTarget>());
// }

// #[test]
// fn relationship_with_missing_target_fails() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Vec<Entity>);

//     let mut world = World::new();
//     let a = world.spawn_empty().id();
//     world.despawn(a);
//     let b = world.spawn(Rel(a)).id();
//     assert!(!world.entity(b).contains::<Rel>());
//     assert!(!world.entity(b).contains::<RelTarget>());
// }

// #[test]
// fn relationship_with_multiple_non_target_fields_compiles() {
//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship(relationship_target=Target)]
//     struct Source {
//         #[relationship]
//         target: Entity,
//         foo: u8,
//         bar: u8,
//     }

//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship_target(relationship=Source)]
//     struct Target(Vec<Entity>);

//     // No assert necessary, looking to make sure compilation works with the macros
// }
// #[test]
// fn relationship_target_with_multiple_non_target_fields_compiles() {
//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship(relationship_target=Target)]
//     struct Source(Entity);

//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship_target(relationship=Source)]
//     struct Target {
//         #[relationship]
//         target: Vec<Entity>,
//         foo: u8,
//         bar: u8,
//     }

//     // No assert necessary, looking to make sure compilation works with the macros
// }

// #[test]
// fn relationship_with_multiple_unnamed_non_target_fields_compiles() {
//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship(relationship_target=Target<T>)]
//     struct Source<T: Send + Sync + 'static>(#[relationship] Entity, PhantomData<T>);

//     #[expect(
//         dead_code,
//         reason = "This struct is used as a compilation test to test the derive macros, and as such is intentionally never constructed."
//     )]
//     #[derive(Component)]
//     #[relationship_target(relationship=Source<T>)]
//     struct Target<T: Send + Sync + 'static>(#[relationship] Vec<Entity>, PhantomData<T>);

//     // No assert necessary, looking to make sure compilation works with the macros
// }

// #[test]
// fn parent_child_relationship_with_custom_relationship() {
//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Entity);

//     let mut world = World::new();

//     // Rel on Parent
//     // Despawn Parent
//     let mut commands = world.commands();
//     let child = commands.spawn_empty().id();
//     let parent = commands.spawn(Rel(child)).add_child(child).id();
//     commands.entity(parent).despawn();
//     world.flush();

//     assert!(world.get_entity(child).is_err());
//     assert!(world.get_entity(parent).is_err());

//     // Rel on Parent
//     // Despawn Child
//     let mut commands = world.commands();
//     let child = commands.spawn_empty().id();
//     let parent = commands.spawn(Rel(child)).add_child(child).id();
//     commands.entity(child).despawn();
//     world.flush();

//     assert!(world.get_entity(child).is_err());
//     assert!(!world.entity(parent).contains::<Rel>());

//     // Rel on Child
//     // Despawn Parent
//     let mut commands = world.commands();
//     let parent = commands.spawn_empty().id();
//     let child = commands.spawn((ChildOf(parent), Rel(parent))).id();
//     commands.entity(parent).despawn();
//     world.flush();

//     assert!(world.get_entity(child).is_err());
//     assert!(world.get_entity(parent).is_err());

//     // Rel on Child
//     // Despawn Child
//     let mut commands = world.commands();
//     let parent = commands.spawn_empty().id();
//     let child = commands.spawn((ChildOf(parent), Rel(parent))).id();
//     commands.entity(child).despawn();
//     world.flush();

//     assert!(world.get_entity(child).is_err());
//     assert!(!world.entity(parent).contains::<RelTarget>());
// }

// #[test]
// fn spawn_batch_with_relationship() {
//     let mut world = World::new();
//     let parent = world.spawn_empty().id();
//     let children = world
//         .spawn_batch((0..10).map(|_| ChildOf(parent)))
//         .collect::<Vec<_>>();

//     for &child in &children {
//         assert!(world
//             .get::<ChildOf>(child)
//             .is_some_and(|child_of| child_of.parent() == parent));
//     }
//     assert!(world
//         .get::<Children>(parent)
//         .is_some_and(|children| children.len() == 10));
// }

// #[test]
// fn insert_batch_with_relationship() {
//     let mut world = World::new();
//     let parent = world.spawn_empty().id();
//     let child = world.spawn_empty().id();
//     world.insert_batch([(child, ChildOf(parent))]);
//     world.flush();

//     assert!(world.get::<ChildOf>(child).is_some());
//     assert!(world.get::<Children>(parent).is_some());
// }

// #[test]
// fn dynamically_traverse_hierarchy() {
//     let mut world = World::new();
//     let child_of_id = world.register_component::<ChildOf>();
//     let children_id = world.register_component::<Children>();

//     let parent = world.spawn_empty().id();
//     let child = world.spawn_empty().id();
//     world.entity_mut(child).insert(ChildOf(parent));
//     world.flush();

//     let children_ptr = world.get_by_id(parent, children_id).unwrap();
//     let RelationshipAccessor::RelationshipTarget { iter, .. } = world
//         .components()
//         .get_info(children_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap()
//     else {
//         unreachable!()
//     };
//     // Safety: `children_ptr` contains value of the same type as the one this accessor was registered for.
//     let children: Vec<_> = unsafe { iter(children_ptr).collect() };
//     assert_eq!(children, vec![child]);

//     let child_of_ptr = world.get_by_id(child, child_of_id).unwrap();
//     let RelationshipAccessor::Relationship {
//         entity_field_offset,
//         ..
//     } = world
//         .components()
//         .get_info(child_of_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap()
//     else {
//         unreachable!()
//     };
//     // Safety:
//     // - offset is in bounds, aligned and has the same lifetime as the original pointer.
//     // - value at offset is guaranteed to be a valid Entity
//     let child_of_entity: Entity =
//         unsafe { *child_of_ptr.byte_add(*entity_field_offset).deref() };
//     assert_eq!(child_of_entity, parent);
// }

// #[test]
// fn relationship_accessor() {
//     #[derive(Component)]
//     #[relationship(relationship_target = LikedBy)]
//     struct Likes {
//         _a: u16,
//         #[relationship]
//         e: Entity,
//         _b: (i8, u8),
//     }

//     #[derive(Component)]
//     #[relationship_target(relationship = Likes)]
//     struct LikedBy(Vec<Entity>);

//     let mut world = World::new();
//     let likes_id = world.register_component::<Likes>();
//     let liked_by_id = world.register_component::<LikedBy>();

//     let likes_accessor = world
//         .components()
//         .get_info(likes_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap();
//     match *likes_accessor {
//         RelationshipAccessor::Relationship {
//             entity_field_offset,
//             linked_spawn,
//             allow_self_referential,
//             relationship_target,
//         } => {
//             assert_eq!(entity_field_offset, core::mem::offset_of!(Likes, e));
//             assert!(!linked_spawn);
//             assert!(!allow_self_referential);
//             assert_eq!(relationship_target, liked_by_id);
//         }
//         _ => {
//             panic!("Not a Relationship")
//         }
//     }

//     let liked_by_accessor = world
//         .components()
//         .get_info(liked_by_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap();
//     match *liked_by_accessor {
//         RelationshipAccessor::RelationshipTarget {
//             iter,
//             linked_spawn,
//             allow_self_referential,
//             relationship,
//         } => {
//             let liked_by = LikedBy(vec![
//                 world.spawn_empty().id(),
//                 world.spawn_empty().id(),
//                 world.spawn_empty().id()
//             ]);
//             // SAFETY: liked_by is of type LikedBy
//             unsafe {
//                 assert_eq!(iter((&liked_by).into()).collect::<Vec<_>>(), liked_by.0);
//             }
//             assert!(!linked_spawn);
//             assert!(!allow_self_referential);
//             assert_eq!(relationship, likes_id);
//         }
//         _ => {
//             panic!("Not a RelationshipTarget")
//         }
//     }

//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget, allow_self_referential)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel, linked_spawn)]
//     struct RelTarget(Vec<Entity>);

//     let rel_id = world.register_component::<Rel>();
//     let rel_target_id = world.register_component::<RelTarget>();

//     let rel_accessor = world
//         .components()
//         .get_info(rel_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap();
//     assert!(rel_accessor.linked_spawn());
//     assert!(rel_accessor.allow_self_referential());
//     let rel_target_accessor = world
//         .components()
//         .get_info(rel_target_id)
//         .unwrap()
//         .relationship_accessor()
//         .unwrap();
//     assert!(rel_target_accessor.linked_spawn());
//     assert!(rel_target_accessor.allow_self_referential());
// }

// #[test]
// pub fn component_hooks_compatibility() {
//     static ADD_CALLED: AtomicBool = AtomicBool::new(false);
//     static INSERT_CALLED: AtomicBool = AtomicBool::new(false);
//     static DISCARD_CALLED: AtomicBool = AtomicBool::new(false);
//     static REMOVE_CALLED: AtomicBool = AtomicBool::new(false);
//     static DESPAWN_CALLED: AtomicBool = AtomicBool::new(false);

//     #[derive(Component)]
//     #[relationship(relationship_target = RelTarget)]
//     #[component(on_add, on_insert, on_discard, on_remove, on_despawn)]
//     struct Rel(Entity);

//     #[derive(Component)]
//     #[relationship_target(relationship = Rel)]
//     struct RelTarget(Entity);

//     impl Rel {
//         fn on_add(world: DeferredWorld, context: HookContext) {
//             let &Rel(target) = world.get(context.entity).unwrap();
//             assert!(!world.entity(target).contains::<RelTarget>());
//             ADD_CALLED.store(true, core::sync::atomic::Ordering::Relaxed);
//         }

//         fn on_insert(world: DeferredWorld, context: HookContext) {
//             let &Rel(target) = world.get(context.entity).unwrap();
//             assert!(!world.entity(target).contains::<RelTarget>());
//             INSERT_CALLED.store(true, core::sync::atomic::Ordering::Relaxed);
//         }

//         fn on_discard(world: DeferredWorld, context: HookContext) {
//             let &Rel(target) = world.get(context.entity).unwrap();
//             assert!(world.entity(target).contains::<RelTarget>());
//             DISCARD_CALLED.store(true, core::sync::atomic::Ordering::Relaxed);
//         }

//         fn on_remove(world: DeferredWorld, context: HookContext) {
//             let &Rel(target) = world.get(context.entity).unwrap();
//             assert!(world.entity(target).contains::<RelTarget>());
//             REMOVE_CALLED.store(true, core::sync::atomic::Ordering::Relaxed);
//         }

//         fn on_despawn(world: DeferredWorld, context: HookContext) {
//             let &Rel(target) = world.get(context.entity).unwrap();
//             assert!(world.entity(target).contains::<RelTarget>());
//             DESPAWN_CALLED.store(true, core::sync::atomic::Ordering::Relaxed);
//         }
//     }

//     let mut world = World::new();
//     let target = world.spawn_empty().id();
//     let source = world.spawn(Rel(target)).id();
//     assert!(world.entity(target).contains::<RelTarget>());
//     assert!(ADD_CALLED.load(core::sync::atomic::Ordering::Relaxed));
//     assert!(INSERT_CALLED.load(core::sync::atomic::Ordering::Relaxed));
//     world.despawn(source);
//     assert!(DISCARD_CALLED.load(core::sync::atomic::Ordering::Relaxed));
//     assert!(REMOVE_CALLED.load(core::sync::atomic::Ordering::Relaxed));
//     assert!(DESPAWN_CALLED.load(core::sync::atomic::Ordering::Relaxed));
// }
