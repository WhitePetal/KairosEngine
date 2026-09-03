use kairos_ecs_macros::SystemSet;

use crate::ecs::{schedule::{SystemSet, SystemSets, Systems}, system::IntoSystem, world::World};

#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct TestSet;

#[test]
fn systems() {
    fn empty_system() {}

    let mut systems = Systems::default();
    assert!(systems.is_empty());
    assert_eq!(systems.len(), 0);

    let system = Box::new(IntoSystem::into_system(empty_system));
    let key = systems.insert(system, vec![]);

    assert!(!systems.is_empty());
    assert_eq!(systems.len(), 1);
    assert!(systems.get(key).is_some());
    assert!(systems.get_conditions(key).is_some());
    assert!(systems.get_conditions(key).unwrap().is_empty());
    assert!(systems.get_mut(key).is_some());
    assert!(!systems.is_initialized());
    assert!(systems.iter().next().is_some());

    let mut world = World::new();
    systems.initialize(&mut world);
    assert!(systems.is_initialized());
}

#[test]
fn system_sets() {
    fn always_true() -> bool {
        true
    }

    let mut sets = SystemSets::default();
    assert!(sets.is_empty());
    assert_eq!(sets.len(), 0);

    let condition = Box::new(IntoSystem::into_system(always_true));
    let key = sets.insert(TestSet.intern(), vec![condition]);

    assert!(!sets.is_empty());
    assert_eq!(sets.len(), 1);
    assert!(sets.get(key).is_some());
    assert!(sets.get_conditions(key).is_some());
    assert!(!sets.get_conditions(key).unwrap().is_empty());
    assert!(!sets.is_initialized());
    assert!(sets.iter().next().is_some());

    let mut world = World::new();
    sets.initialize(&mut world);
    assert!(sets.is_initialized());
}
