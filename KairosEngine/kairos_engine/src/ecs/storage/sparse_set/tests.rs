use crate::ecs::{
    component::{Component, ComponentDescriptor, ComponentId, ComponentInfo},
    entity::{Entity, EntityIndex},
    storage::{SparseSet, SparseSets},
};

// #[derive(Debug, Eq, PartialEq)]
// struct Foo(usize);

// #[test]
// fn sparse_set() {
//     let mut set = SparseSet::<Entity, Foo>::default();
//     let e0 = Entity::from_index(EntityIndex::from_raw_u32(0).unwrap());
//     let e1 = Entity::from_index(EntityIndex::from_raw_u32(1).unwrap());
//     let e2 = Entity::from_index(EntityIndex::from_raw_u32(2).unwrap());
//     let e3 = Entity::from_index(EntityIndex::from_raw_u32(3).unwrap());
//     let e4 = Entity::from_index(EntityIndex::from_raw_u32(4).unwrap());

//     set.insert(e1, Foo(1));
//     set.insert(e2, Foo(2));
//     set.insert(e3, Foo(3));

//     assert_eq!(set.get(e0), None);
//     assert_eq!(set.get(e1), Some(&Foo(1)));
//     assert_eq!(set.get(e2), Some(&Foo(2)));
//     assert_eq!(set.get(e3), Some(&Foo(3)));
//     assert_eq!(set.get(e4), None);

//     {
//         let iter_results = set.values().collect::<Vec<_>>();
//         assert_eq!(iter_results, vec![&Foo(1), &Foo(2), &Foo(3)]);
//     }

//     assert_eq!(set.remove(e2), Some(Foo(2)));
//     assert_eq!(set.remove(e2), None);

//     assert_eq!(set.get(e0), None);
//     assert_eq!(set.get(e1), Some(&Foo(1)));
//     assert_eq!(set.get(e2), None);
//     assert_eq!(set.get(e3), Some(&Foo(3)));
//     assert_eq!(set.get(e4), None);

//     assert_eq!(set.remove(e1), Some(Foo(1)));

//     assert_eq!(set.get(e0), None);
//     assert_eq!(set.get(e1), None);
//     assert_eq!(set.get(e2), None);
//     assert_eq!(set.get(e3), Some(&Foo(3)));
//     assert_eq!(set.get(e4), None);

//     set.insert(e1, Foo(10));

//     assert_eq!(set.get(e1), Some(&Foo(10)));

//     *set.get_mut(e1).unwrap() = Foo(11);
//     assert_eq!(set.get(e1), Some(&Foo(11)));
// }

// #[test]
// fn sparse_sets() {
//     let mut sets = SparseSets::default();

//     #[derive(Component, Default, Debug)]
//     struct TestComponent1;

//     #[derive(Component, Default, Debug)]
//     struct TestComponent2;

//     assert_eq!(sets.len(), 0);
//     assert!(sets.is_empty());

//     register_component::<TestComponent1>(&mut sets, 1);
//     assert_eq!(sets.len(), 1);

//     register_component::<TestComponent2>(&mut sets, 2);
//     assert_eq!(sets.len(), 2);

//     // check its shape by iter
//     let mut collected_sets = sets
//         .iter()
//         .map(|(id, set)| (id, set.len()))
//         .collect::<Vec<_>>();
//     collected_sets.sort();
//     assert_eq!(
//         collected_sets,
//         vec![(ComponentId::new(1), 0), (ComponentId::new(2), 0),]
//     );

//     fn register_component<T: Component>(sets: &mut SparseSets, id: usize) {
//         let descriptor = ComponentDescriptor::new::<T>();
//         let id = ComponentId::new(id);
//         let info = ComponentInfo::new(id, descriptor);
//         sets.get_or_insert(&info);
//     }
// }
