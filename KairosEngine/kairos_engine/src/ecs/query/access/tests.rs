use fixedbitset::FixedBitSet;

use crate::ecs::{
    component::ComponentId,
    query::{
        Access, AccessConflicts, AccessFilters, ComponentAccessKind, ComponentIdSet,
        FilteredAccess, FilteredAccessSet, UnboundedAccessError,
        access::{invertible_difference_with, invertible_union_with},
    },
};

fn create_sample_access() -> Access {
    let mut access = Access::default();

    access.add_read(ComponentId::new(1));
    access.add_read(ComponentId::new(2));
    access.add_write(ComponentId::new(3));
    access.add_archetypal(ComponentId::new(5));
    access.read_all();

    access
}

fn create_sample_filtered_access() -> FilteredAccess {
    let mut filtered_access = FilteredAccess::default();

    filtered_access.add_write(ComponentId::new(1));
    filtered_access.add_read(ComponentId::new(2));
    filtered_access.add_required(ComponentId::new(3));
    filtered_access.and_with(ComponentId::new(4));

    filtered_access
}

fn create_sample_access_filters() -> AccessFilters {
    let mut access_filters = AccessFilters::default();

    access_filters.with.insert(ComponentId::new(3));
    access_filters.without.insert(ComponentId::new(5));

    access_filters
}

fn create_sample_filtered_access_set() -> FilteredAccessSet {
    let mut filtered_access_set = FilteredAccessSet::default();

    filtered_access_set.add_unfiltered_component_read(ComponentId::new(2));
    filtered_access_set.add_unfiltered_component_write(ComponentId::new(4));
    filtered_access_set.read_all();

    filtered_access_set
}

#[test]
fn test_access_clone() {
    let original = create_sample_access();
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_access_clone_from() {
    let original = create_sample_access();
    let mut cloned = Access::default();

    cloned.add_write(ComponentId::new(7));
    cloned.add_read(ComponentId::new(4));
    cloned.add_archetypal(ComponentId::new(8));
    cloned.write_all();

    cloned.clone_from(&original);

    assert_eq!(original, cloned);
}

#[test]
fn test_filtered_access_clone() {
    let original = create_sample_filtered_access();
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_filtered_access_clone_from() {
    let original = create_sample_filtered_access();
    let mut cloned = FilteredAccess::default();

    cloned.add_write(ComponentId::new(7));
    cloned.add_read(ComponentId::new(4));
    cloned.append_or(&FilteredAccess::default());

    cloned.clone_from(&original);

    assert_eq!(original, cloned);
}

#[test]
fn test_access_filters_clone() {
    let original = create_sample_access_filters();
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_access_filters_clone_from() {
    let original = create_sample_access_filters();
    let mut cloned = AccessFilters::default();

    cloned.with.insert(ComponentId::new(1));
    cloned.without.insert(ComponentId::new(2));

    cloned.clone_from(&original);

    assert_eq!(original, cloned);
}

#[test]
fn test_filtered_access_set_clone() {
    let original = create_sample_filtered_access_set();
    let cloned = original.clone();

    assert_eq!(original, cloned);
}

#[test]
fn test_filtered_access_set_from() {
    let original = create_sample_filtered_access_set();
    let mut cloned = FilteredAccessSet::default();

    cloned.add_unfiltered_component_read(ComponentId::new(7));
    cloned.add_unfiltered_component_write(ComponentId::new(9));
    cloned.write_all();

    cloned.clone_from(&original);

    assert_eq!(original, cloned);
}

#[test]
fn read_all_access_conflicts() {
    // read_all / single write
    let mut access_a = Access::default();
    access_a.add_write(ComponentId::new(0));

    let mut access_b = Access::default();
    access_b.read_all();

    assert!(!access_b.is_compatible(&access_a));

    // read_all / read_all
    let mut access_a = Access::default();
    access_a.read_all();

    let mut access_b = Access::default();
    access_b.read_all();

    assert!(access_b.is_compatible(&access_a));
}

#[test]
fn access_get_conflicts() {
    let mut access_a = Access::default();
    access_a.add_read(ComponentId::new(0));
    access_a.add_read(ComponentId::new(1));

    let mut access_b = Access::default();
    access_b.add_read(ComponentId::new(0));
    access_b.add_write(ComponentId::new(1));

    assert_eq!(
        access_a.get_conflicts(&access_b),
        vec![ComponentId::new(1)].into()
    );

    let mut access_c = Access::default();
    access_c.add_write(ComponentId::new(0));
    access_c.add_write(ComponentId::new(1));

    assert_eq!(
        access_a.get_conflicts(&access_c),
        vec![ComponentId::new(0), ComponentId::new(1)].into()
    );
    assert_eq!(
        access_b.get_conflicts(&access_c),
        vec![ComponentId::new(0), ComponentId::new(1)].into()
    );

    let mut access_d = Access::default();
    access_d.add_read(ComponentId::new(0));

    assert_eq!(access_d.get_conflicts(&access_a), AccessConflicts::empty());
    assert_eq!(access_d.get_conflicts(&access_b), AccessConflicts::empty());
    assert_eq!(
        access_d.get_conflicts(&access_c),
        vec![ComponentId::new(0)].into()
    );
}

#[test]
fn filtered_combined_access() {
    let mut access_a = FilteredAccessSet::default();
    access_a.add_unfiltered_component_read(ComponentId::new(1));

    let mut filter_b = FilteredAccess::default();
    filter_b.add_write(ComponentId::new(1));

    let conflicts = access_a.get_conflicts_single(&filter_b);
    assert_eq!(
        &conflicts,
        &AccessConflicts::from(vec![ComponentId::new(1)]),
        "access_a: {access_a:?}, filter_b: {filter_b:?}"
    );
}

#[test]
fn filtered_access_extend() {
    let mut access_a = FilteredAccess::default();
    access_a.add_read(ComponentId::new(0));
    access_a.add_read(ComponentId::new(1));
    access_a.and_with(ComponentId::new(2));

    let mut access_b = FilteredAccess::default();
    access_b.add_read(ComponentId::new(0));
    access_b.add_write(ComponentId::new(3));
    access_b.and_without(ComponentId::new(4));

    access_a.extend(&access_b);

    let mut expected = FilteredAccess::default();
    expected.add_read(ComponentId::new(0));
    expected.add_read(ComponentId::new(1));
    expected.and_with(ComponentId::new(2));
    expected.add_write(ComponentId::new(3));
    expected.and_without(ComponentId::new(4));

    assert!(access_a.eq(&expected));
}

#[test]
fn filtered_access_extend_or() {
    let mut access_a = FilteredAccess::default();
    // Exclusive access to `(&mut A, &mut B)`.
    access_a.add_write(ComponentId::new(0));
    access_a.add_write(ComponentId::new(1));

    // Filter by `With<C>`.
    let mut access_b = FilteredAccess::default();
    access_b.and_with(ComponentId::new(2));

    // Filter by `(With<D>, Without<E>)`.
    let mut access_c = FilteredAccess::default();
    access_c.and_with(ComponentId::new(3));
    access_c.and_without(ComponentId::new(4));

    // Turns `access_b` into `Or<(With<C>, (With<D>, Without<D>))>`.
    access_b.append_or(&access_c);
    // Applies the filters to the initial query, which corresponds to the FilteredAccess'
    // representation of `Query<(&mut A, &mut B), Or<(With<C>, (With<D>, Without<E>))>>`.
    access_a.extend(&access_b);

    // Construct the expected `FilteredAccess` struct.
    // The intention here is to test that exclusive access implied by `add_write`
    // forms correct normalized access structs when extended with `Or` filters.
    let mut expected = FilteredAccess::default();
    expected.add_write(ComponentId::new(0));
    expected.add_write(ComponentId::new(1));
    // The resulted access is expected to represent `Or<((With<A>, With<B>, With<C>), (With<A>, With<B>, With<D>, Without<E>))>`.
    expected.filter_sets = vec![
        AccessFilters {
            with: ComponentIdSet::from_bits(FixedBitSet::with_capacity_and_blocks(3, [0b111])),
            without: ComponentIdSet::default(),
        },
        AccessFilters {
            with: ComponentIdSet::from_bits(FixedBitSet::with_capacity_and_blocks(4, [0b1011])),
            without: ComponentIdSet::from_bits(FixedBitSet::with_capacity_and_blocks(5, [0b10000])),
        },
    ];

    assert_eq!(access_a, expected);
}

#[test]
fn try_iter_component_access_simple() {
    let mut access = Access::default();

    access.add_read(ComponentId::new(1));
    access.add_read(ComponentId::new(2));
    access.add_write(ComponentId::new(3));
    access.add_archetypal(ComponentId::new(5));

    let result = access.try_iter_access().map(Iterator::collect::<Vec<_>>);

    assert_eq!(
        result,
        Ok(vec![
            ComponentAccessKind::Shared(ComponentId::new(1)),
            ComponentAccessKind::Shared(ComponentId::new(2)),
            ComponentAccessKind::Exclusive(ComponentId::new(3)),
            ComponentAccessKind::Archetypal(ComponentId::new(5)),
        ]),
    );
}

#[test]
fn try_iter_component_access_unbounded_write_all() {
    let mut access = Access::default();

    access.add_read(ComponentId::new(1));
    access.add_read(ComponentId::new(2));
    access.write_all();

    let result = access.try_iter_access().map(Iterator::collect::<Vec<_>>);

    assert_eq!(
        result,
        Err(UnboundedAccessError {
            writes_inverted: true,
            read_and_writes_inverted: true
        }),
    );
}

#[test]
fn try_iter_component_access_unbounded_read_all() {
    let mut access = Access::default();

    access.add_read(ComponentId::new(1));
    access.add_read(ComponentId::new(2));
    access.read_all();

    let result = access.try_iter_access().map(Iterator::collect::<Vec<_>>);

    assert_eq!(
        result,
        Err(UnboundedAccessError {
            writes_inverted: false,
            read_and_writes_inverted: true
        }),
    );
}

/// Create a `ComponentIdSet` with a given number of total bits and a given list of bits to set.
/// Setting the number of bits is important in tests since the `PartialEq` impl checks that the length matches.
fn bit_set(bits: usize, iter: impl IntoIterator<Item = usize>) -> ComponentIdSet {
    let mut result = FixedBitSet::with_capacity(bits);
    result.extend(iter);
    ComponentIdSet::from_bits(result)
}

#[test]
fn invertible_union_with_tests() {
    let invertible_union = |mut self_inverted: bool, other_inverted: bool| {
        // Check all four possible bit states: In both sets, the first, the second, or neither
        let mut self_set = bit_set(4, [0, 1]);
        let other_set = bit_set(4, [0, 2]);
        invertible_union_with(
            &mut self_set,
            &mut self_inverted,
            &other_set,
            other_inverted,
        );
        (self_set, self_inverted)
    };

    // Check each combination of `inverted` flags
    let (s, i) = invertible_union(false, false);
    // [0, 1] | [0, 2] = [0, 1, 2]
    assert_eq!((s, i), (bit_set(4, [0, 1, 2]), false));

    let (s, i) = invertible_union(false, true);
    // [0, 1] | [1, 3, ...] = [0, 1, 3, ...]
    assert_eq!((s, i), (bit_set(4, [2]), true));

    let (s, i) = invertible_union(true, false);
    // [2, 3, ...] | [0, 2] = [0, 2, 3, ...]
    assert_eq!((s, i), (bit_set(4, [1]), true));

    let (s, i) = invertible_union(true, true);
    // [2, 3, ...] | [1, 3, ...] = [1, 2, 3, ...]
    assert_eq!((s, i), (bit_set(4, [0]), true));
}

#[test]
fn invertible_union_with_different_lengths() {
    // When adding a large inverted set to a small normal set,
    // make sure we invert the bits beyond the original length.
    // Failing to call `grow` before `toggle_range` would cause bit 1 to be zero,
    // which would incorrectly treat it as included in the output set.
    let mut self_set = bit_set(1, [0]);
    let mut self_inverted = false;
    let other_set = bit_set(3, [0, 1]);
    let other_inverted = true;
    invertible_union_with(
        &mut self_set,
        &mut self_inverted,
        &other_set,
        other_inverted,
    );

    // [0] | [2, ...] = [0, 2, ...]
    assert_eq!((self_set, self_inverted), (bit_set(3, [1]), true));
}

#[test]
fn invertible_difference_with_tests() {
    let invertible_difference = |mut self_inverted: bool, other_inverted: bool| {
        // Check all four possible bit states: In both sets, the first, the second, or neither
        let mut self_set = bit_set(4, [0, 1]);
        let other_set = bit_set(4, [0, 2]);
        invertible_difference_with(
            &mut self_set,
            &mut self_inverted,
            &other_set,
            other_inverted,
        );
        (self_set, self_inverted)
    };

    // Check each combination of `inverted` flags
    let (s, i) = invertible_difference(false, false);
    // [0, 1] - [0, 2] = [1]
    assert_eq!((s, i), (bit_set(4, [1]), false));

    let (s, i) = invertible_difference(false, true);
    // [0, 1] - [1, 3, ...] = [0]
    assert_eq!((s, i), (bit_set(4, [0]), false));

    let (s, i) = invertible_difference(true, false);
    // [2, 3, ...] - [0, 2] = [3, ...]
    assert_eq!((s, i), (bit_set(4, [0, 1, 2]), true));

    let (s, i) = invertible_difference(true, true);
    // [2, 3, ...] - [1, 3, ...] = [2]
    assert_eq!((s, i), (bit_set(4, [2]), false));
}

#[test]
fn component_id_set_insert_remove_clear() {
    let mut set = ComponentIdSet::new();
    assert!(!set.contains(ComponentId::new(0)));
    assert!(!set.contains(ComponentId::new(1)));
    assert!(!set.contains(ComponentId::new(2)));
    assert!(set.is_clear());
    set.insert(ComponentId::new(2));
    set.insert(ComponentId::new(1));
    assert!(!set.contains(ComponentId::new(0)));
    assert!(set.contains(ComponentId::new(1)));
    assert!(set.contains(ComponentId::new(2)));
    assert!(!set.is_clear());
    set.remove(ComponentId::new(1));
    assert!(!set.contains(ComponentId::new(0)));
    assert!(!set.contains(ComponentId::new(1)));
    assert!(set.contains(ComponentId::new(2)));
    assert!(!set.is_clear());
    set.insert(ComponentId::new(2));
    set.insert(ComponentId::new(1));
    assert!(!set.contains(ComponentId::new(0)));
    assert!(set.contains(ComponentId::new(1)));
    assert!(set.contains(ComponentId::new(2)));
    assert!(!set.is_clear());
    set.clear();
    assert!(!set.contains(ComponentId::new(0)));
    assert!(!set.contains(ComponentId::new(1)));
    assert!(!set.contains(ComponentId::new(2)));
    assert!(set.is_clear());
}

#[test]
fn component_id_set_remove_out_of_range() {
    let mut set = ComponentIdSet::new();
    set.remove(ComponentId::new(3));
    set.insert(ComponentId::new(1));
    set.remove(ComponentId::new(4));
    assert!(set.iter().eq([1].map(ComponentId::new)));
}

#[test]
fn component_id_set_is_subset_is_disjoint() {
    let set_1234 = ComponentIdSet::from_iter([1, 2, 3, 4].map(ComponentId::new));
    let set_23 = ComponentIdSet::from_iter([2, 3].map(ComponentId::new));
    let set_45 = ComponentIdSet::from_iter([4, 5].map(ComponentId::new));
    assert!(set_23.is_subset(&set_1234));
    assert!(!set_1234.is_subset(&set_23));
    assert!(set_23.is_disjoint(&set_45));
    assert!(set_45.is_disjoint(&set_23));
    assert!(!set_1234.is_disjoint(&set_23));
    assert!(!set_23.is_disjoint(&set_1234));
}

#[test]
fn component_id_set_union_intersection_difference() {
    let set_13 = ComponentIdSet::from_iter([1, 3].map(ComponentId::new));
    let set_23 = ComponentIdSet::from_iter([2, 3].map(ComponentId::new));

    assert!(set_13.union(&set_23).eq([1, 3, 2].map(ComponentId::new)));
    assert!(set_23.union(&set_13).eq([2, 3, 1].map(ComponentId::new)));
    assert!(set_13.intersection(&set_23).eq([3].map(ComponentId::new)));
    assert!(set_23.intersection(&set_13).eq([3].map(ComponentId::new)));
    assert!(set_13.difference(&set_23).eq([1].map(ComponentId::new)));
    assert!(set_23.difference(&set_13).eq([2].map(ComponentId::new)));
}

#[test]
fn component_id_set_union_intersection_difference_with() {
    let set_13 = ComponentIdSet::from_iter([1, 3].map(ComponentId::new));
    let set_23 = ComponentIdSet::from_iter([2, 3].map(ComponentId::new));

    let mut s = set_13.clone();
    s.union_with(&set_23);
    assert!(s.iter().eq([1, 2, 3].map(ComponentId::new)));

    let mut s = set_23.clone();
    s.union_with(&set_13);
    assert!(s.iter().eq([1, 2, 3].map(ComponentId::new)));

    let mut s = set_13.clone();
    s.intersect_with(&set_23);
    assert!(s.iter().eq([3].map(ComponentId::new)));

    let mut s = set_23.clone();
    s.intersect_with(&set_13);
    assert!(s.iter().eq([3].map(ComponentId::new)));

    let mut s = set_13.clone();
    s.difference_with(&set_23);
    assert!(s.iter().eq([1].map(ComponentId::new)));

    let mut s = set_23.clone();
    s.difference_with(&set_13);
    assert!(s.iter().eq([2].map(ComponentId::new)));

    let mut s = set_13.clone();
    s.difference_from(&set_23);
    assert!(s.iter().eq([2].map(ComponentId::new)));

    let mut s = set_23.clone();
    s.difference_from(&set_13);
    assert!(s.iter().eq([1].map(ComponentId::new)));
}
