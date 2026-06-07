use crate::ecs::{
    entity::{Entity, EntityFlag},
    id::Id,
    sparse_set::{EntityStorage, SparseSet},
};

impl EntityStorage {
    fn get_entity(&self, id: &Entity) -> &Entity {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when index id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Index the id is not alive! id: {:?}",
            id
        );

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value.get_version() == id.get_version(),
            "The id's version is invalided while Index the id! id: {:?}",
            id
        );

        &self.dense[sparse_value.get_idx() as usize]
    }
}

#[test]
fn sparse_set() {
    let mut entity_stroge = EntityStorage::new(128);
    let ea = entity_stroge.next();
    let ea_value = 10;
    let eb = entity_stroge.next();
    let eb_value = 20;
    let ec = entity_stroge.next();
    let ec_value = 30;
    let ed = entity_stroge.next();
    let ed_value = 40;

    let mut sparset_set = SparseSet::new(128);
    sparset_set.insert(&ea, ea_value);
    sparset_set.insert(&eb, eb_value);
    sparset_set.insert(&ec, ec_value);
    sparset_set.insert(&ed, ed_value);

    sparset_set.remove(eb.clone());
    entity_stroge.remove(eb);
    sparset_set.remove(ed.clone());
    entity_stroge.remove(ed);

    let ef = entity_stroge.next();
    let ef_value = 50;
    sparset_set.insert(&ef, ef_value);

    debug_assert_eq!(ef, Entity::new(3, 1, EntityFlag::Default));
    debug_assert_eq!(entity_stroge.get_entity(&ea), &ea);
    debug_assert_eq!(entity_stroge.get_entity(&ec), &ec);

    debug_assert_eq!(sparset_set.get_value(&ef), ef_value);
    debug_assert_eq!(sparset_set.get_value(&ea), ea_value);
    debug_assert_eq!(sparset_set.get_value(&ec), ec_value);
}
