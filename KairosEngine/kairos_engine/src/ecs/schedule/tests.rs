use std::sync::atomic::{AtomicU32, Ordering};

use kairos_ecs_macros::{Resource, SystemSet};

use crate::ecs::{
    change_detection::{Res, ResMut},
    world::World,
};

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
enum TestSystems {
    A,
    B,
    C,
    D,
    X,
}

#[derive(Resource, Default)]
struct SystemOrder(Vec<u32>);

#[derive(Resource, Default)]
struct RunConditionBool(bool);

#[derive(Resource, Default)]
struct Counter(AtomicU32);

fn make_exclusive_system(tag: u32) -> impl FnMut(&mut World) {
    move |world| world.resource_mut::<SystemOrder>().0.push(tag)
}

fn make_function_system(tag: u32) -> impl FnMut(ResMut<SystemOrder>) {
    move |mut resource: ResMut<SystemOrder>| resource.0.push(tag)
}

fn named_system(mut resource: ResMut<SystemOrder>) {
    resource.0.push(u32::MAX);
}

fn named_exclusive_system(world: &mut World) {
    world.resource_mut::<SystemOrder>().0.push(u32::MAX);
}

fn counting_system(counter: Res<Counter>) {
    counter.0.fetch_add(1, Ordering::Relaxed);
}

mod system_execution {
    use crate::ecs::schedule::Schedule;

    use super::*;

    #[test]
    fn run_system() {
        let mut world = World::default();
        let mut schedule = Schedule::default();

        world.init_resource::<SystemOrder>();

        schedule.add_systems(make_function_system(0));
        schedule.run(&mut world);

        assert_eq!(world.resource::<SystemOrder>().0, vec![0]);
    }

    #[test]
    fn run_exclusive_system() {
        let mut world = World::default();
        let mut schedule = Schedule::default();

        world.init_resource::<SystemOrder>();

        schedule.add_systems(make_exclusive_system(0));
        schedule.run(&mut world);

        assert_eq!(world.resource::<SystemOrder>().0, vec![0]);
    }

    #[test]
    #[cfg(not(miri))]
    fn parallel_execution() {
        use std::sync::{Arc, Barrier};

        use kairos_tasks::{ComputeTaskPool, TaskPool};

        use crate::ecs::schedule::Schedule;

        let mut world = World::default();
        let mut schedule = Schedule::default();
        let thread_count = ComputeTaskPool::get_or_init(TaskPool::default).thread_num();

        let barrier = Arc::new(Barrier::new(thread_count));

        for _ in 0..thread_count {
            let inner = barrier.clone();
            schedule.add_systems(move || {
                inner.wait();
            });
        }

        schedule.run(&mut world);
    }
}
