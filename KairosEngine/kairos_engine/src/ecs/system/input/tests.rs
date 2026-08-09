use crate::ecs::{
    system::{In, InMut, InRef, IntoSystem, StaticSystemInput, assert_is_system},
    world::World,
};

#[test]
fn two_tuple() {
    fn by_value((In(a), In(b)): (In<usize>, In<usize>)) -> usize {
        a + b
    }
    fn by_ref((InRef(a), InRef(b)): (InRef<usize>, InRef<usize>)) -> usize {
        *a + *b
    }
    fn by_mut((InMut(a), In(b)): (InMut<usize>, In<usize>)) {
        *a += b;
    }

    let mut world = World::new();
    let mut by_value = IntoSystem::into_system(by_value);
    let mut by_ref = IntoSystem::into_system(by_ref);
    let mut by_mut = IntoSystem::into_system(by_mut);

    by_value.initialize(&mut world);
    by_ref.initialize(&mut world);
    by_mut.initialize(&mut world);

    let mut a = 12;
    let b = 24;

    assert_eq!(by_value.run((a, b), &mut world).unwrap(), 36);
    assert_eq!(by_ref.run((&a, &b), &mut world).unwrap(), 36);
    by_mut.run((&mut a, b), &mut world).unwrap();
    assert_eq!(a, 36);
}

#[test]
fn compatible_input() {
    fn takes_usize(In(a): In<usize>) -> usize {
        a
    }

    fn takes_static_usize(StaticSystemInput(b): StaticSystemInput<In<usize>>) -> usize {
        b
    }

    assert_is_system::<In<usize>, usize, _>(takes_usize);
    // test if StaticSystemInput is compatible with its inner type
    assert_is_system::<In<usize>, usize, _>(takes_static_usize);
}

#[test]
fn option_input() {
    fn takes_option_mut(a: Option<InMut<usize>>) -> usize {
        a.map(|InMut(x)| *x).unwrap_or(0)
    }

    let mut world = World::new();

    let mut system = IntoSystem::into_system(takes_option_mut);
    system.initialize(&mut world);

    let mut value = 12;
    assert_eq!(system.run(Some(&mut value), &mut world).unwrap(), 12);
    assert_eq!(system.run(None, &mut world).unwrap(), 0);
}
