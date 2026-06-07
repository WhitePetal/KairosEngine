use crate::ecs::{
    entity::EntityFlag,
    id::Id,
    sparse_set::{EntityStorage, SparseSet},
};

#[test]
fn entity_stroge() {
    let mut entity_stroge = EntityStorage::new(128);
    let ea = entity_stroge.next();
    let eb = entity_stroge.next();
    let ec = entity_stroge.next();
    let ed = entity_stroge.next();
    entity_stroge.remove(eb);
    let removed_ed = entity_stroge.remove(ed);
    let ef = entity_stroge.next();

    debug_assert_eq!(ef, removed_ed.get_next_version(EntityFlag::Default));
    debug_assert_eq!(entity_stroge.get_value(&ea), ea);
    debug_assert_eq!(entity_stroge.get_value(&ec), ec);
}

#[test]
fn sparse_set() {
    let mut entity_stroge = EntityStorage::new(128);
    let ea = entity_stroge.next();
    let ea_value = 10;
    let eb = entity_stroge.next();
    let eb_value = 20;

    let mut sparset_set = SparseSet::new(128);
    sparset_set.insert(&ea, ea_value);
}
