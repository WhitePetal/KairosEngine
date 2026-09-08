use kairos_ecs_macros::{Resource, ScheduleLabel, SystemSet};

use crate::{
    change_detection::ResMut,
    schedule::Schedule,
    system::{IntoSystem, System},
    world::World,
};

use super::*;

#[test]
fn test_schedule_label() {
    #[derive(Resource)]
    struct Flag(bool);

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct A;

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct B;

    let mut world = World::new();

    let mut schedule = Schedule::new(A);
    schedule.add_systems(|mut flag: ResMut<Flag>| flag.0 = true);
    world.add_schedule(schedule);

    let interned = A.intern();

    world.insert_resource(Flag(false));
    world.run_schedule(interned);
    assert!(world.resource::<Flag>().0);

    world.insert_resource(Flag(false));
    world.run_schedule(interned);
    assert!(world.resource::<Flag>().0);

    assert_ne!(A.intern(), B.intern());
}

#[test]
fn test_derive_schedule_label() {
    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct UnitLabel;

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct TupleLabel(u32, u32);

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct StructLabel {
        a: u32,
        b: u32,
    }

    #[expect(
        dead_code,
        reason = "This is a derive macro compilation test. It won't be constructed."
    )]
    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyTupleLabel();

    #[expect(
        dead_code,
        reason = "This is a derive macro compilation test. It won't be constructed."
    )]
    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyStructLabel {}

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    enum EnumLabel {
        #[default]
        Unit,
        Tuple(u32, u32),
        Struct {
            a: u32,
            b: u32,
        },
    }

    #[derive(ScheduleLabel, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct GenericLabel<T>(PhantomData<T>);

    assert_eq!(UnitLabel.intern(), UnitLabel.intern());
    assert_eq!(EnumLabel::Unit.intern(), EnumLabel::Unit.intern());
    assert_ne!(UnitLabel.intern(), EnumLabel::Unit.intern());
    assert_ne!(UnitLabel.intern(), TupleLabel(0, 0).intern());
    assert_ne!(EnumLabel::Unit.intern(), EnumLabel::Tuple(0, 0).intern());

    assert_eq!(TupleLabel(0, 0).intern(), TupleLabel(0, 0).intern());
    assert_eq!(
        EnumLabel::Tuple(0, 0).intern(),
        EnumLabel::Tuple(0, 0).intern()
    );
    assert_ne!(TupleLabel(0, 0).intern(), TupleLabel(0, 1).intern());
    assert_ne!(
        EnumLabel::Tuple(0, 0).intern(),
        EnumLabel::Tuple(0, 1).intern()
    );
    assert_ne!(TupleLabel(0, 0).intern(), EnumLabel::Tuple(0, 0).intern());
    assert_ne!(
        TupleLabel(0, 0).intern(),
        StructLabel { a: 0, b: 0 }.intern()
    );
    assert_ne!(
        EnumLabel::Tuple(0, 0).intern(),
        EnumLabel::Struct { a: 0, b: 0 }.intern()
    );

    assert_eq!(
        StructLabel { a: 0, b: 0 }.intern(),
        StructLabel { a: 0, b: 0 }.intern()
    );
    assert_eq!(
        EnumLabel::Struct { a: 0, b: 0 }.intern(),
        EnumLabel::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(
        StructLabel { a: 0, b: 0 }.intern(),
        StructLabel { a: 0, b: 1 }.intern()
    );
    assert_ne!(
        EnumLabel::Struct { a: 0, b: 0 }.intern(),
        EnumLabel::Struct { a: 0, b: 1 }.intern()
    );
    assert_ne!(
        StructLabel { a: 0, b: 0 }.intern(),
        EnumLabel::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(
        StructLabel { a: 0, b: 0 }.intern(),
        EnumLabel::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(StructLabel { a: 0, b: 0 }.intern(), UnitLabel.intern(),);
    assert_ne!(
        EnumLabel::Struct { a: 0, b: 0 }.intern(),
        EnumLabel::Unit.intern()
    );

    assert_eq!(
        GenericLabel::<u32>(PhantomData).intern(),
        GenericLabel::<u32>(PhantomData).intern()
    );
    assert_ne!(
        GenericLabel::<u32>(PhantomData).intern(),
        GenericLabel::<u64>(PhantomData).intern()
    );
}

#[test]
fn test_derive_system_set() {
    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct UnitSet;

    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct TupleSet(u32, u32);

    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct StructSet {
        a: u32,
        b: u32,
    }

    #[expect(
        dead_code,
        reason = "This is a derive macro compilation test. It won't be constructed."
    )]
    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyTupleSet();

    #[expect(
        dead_code,
        reason = "This is a derive macro compilation test. It won't be constructed."
    )]
    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct EmptyStructSet {}

    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    enum EnumSet {
        #[default]
        Unit,
        Tuple(u32, u32),
        Struct {
            a: u32,
            b: u32,
        },
    }

    #[derive(SystemSet, Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    struct GenericSet<T>(PhantomData<T>);

    assert_eq!(UnitSet.intern(), UnitSet.intern());
    assert_eq!(EnumSet::Unit.intern(), EnumSet::Unit.intern());
    assert_ne!(UnitSet.intern(), EnumSet::Unit.intern());
    assert_ne!(UnitSet.intern(), TupleSet(0, 0).intern());
    assert_ne!(EnumSet::Unit.intern(), EnumSet::Tuple(0, 0).intern());

    assert_eq!(TupleSet(0, 0).intern(), TupleSet(0, 0).intern());
    assert_eq!(EnumSet::Tuple(0, 0).intern(), EnumSet::Tuple(0, 0).intern());
    assert_ne!(TupleSet(0, 0).intern(), TupleSet(0, 1).intern());
    assert_ne!(EnumSet::Tuple(0, 0).intern(), EnumSet::Tuple(0, 1).intern());
    assert_ne!(TupleSet(0, 0).intern(), EnumSet::Tuple(0, 0).intern());
    assert_ne!(TupleSet(0, 0).intern(), StructSet { a: 0, b: 0 }.intern());
    assert_ne!(
        EnumSet::Tuple(0, 0).intern(),
        EnumSet::Struct { a: 0, b: 0 }.intern()
    );

    assert_eq!(
        StructSet { a: 0, b: 0 }.intern(),
        StructSet { a: 0, b: 0 }.intern()
    );
    assert_eq!(
        EnumSet::Struct { a: 0, b: 0 }.intern(),
        EnumSet::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(
        StructSet { a: 0, b: 0 }.intern(),
        StructSet { a: 0, b: 1 }.intern()
    );
    assert_ne!(
        EnumSet::Struct { a: 0, b: 0 }.intern(),
        EnumSet::Struct { a: 0, b: 1 }.intern()
    );
    assert_ne!(
        StructSet { a: 0, b: 0 }.intern(),
        EnumSet::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(
        StructSet { a: 0, b: 0 }.intern(),
        EnumSet::Struct { a: 0, b: 0 }.intern()
    );
    assert_ne!(StructSet { a: 0, b: 0 }.intern(), UnitSet.intern(),);
    assert_ne!(
        EnumSet::Struct { a: 0, b: 0 }.intern(),
        EnumSet::Unit.intern()
    );

    assert_eq!(
        GenericSet::<u32>(PhantomData).intern(),
        GenericSet::<u32>(PhantomData).intern()
    );
    assert_ne!(
        GenericSet::<u32>(PhantomData).intern(),
        GenericSet::<u64>(PhantomData).intern()
    );
}

#[test]
fn system_set_matches_default_system_set() {
    fn system() {}
    let set_from_into_system_set = IntoSystemSet::into_system_set(system).intern();
    let system = IntoSystem::into_system(system);
    let set_from_system = system.default_system_sets()[0];
    assert_eq!(set_from_into_system_set, set_from_system);
}

#[test]
fn system_set_matches_default_system_set_exclusive() {
    fn system(_: &mut World) {}
    let set_from_into_system_set = IntoSystemSet::into_system_set(system).intern();
    let system = IntoSystem::into_system(system);
    let set_from_system = system.default_system_sets()[0];
    assert_eq!(set_from_into_system_set, set_from_system);
}
