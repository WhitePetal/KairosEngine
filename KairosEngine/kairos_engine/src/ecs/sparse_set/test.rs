use crate::ecs::{
    entity::{Entity, EntityFlag},
    id::Id,
    sparse_set::{EntityStorage, SparseSet},
};

impl EntityStorage {
    fn get_entity(&self, id: &Entity) -> &Entity {
        let sparse_pos = Self::get_sparse_pos(id.idx());
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
            sparse_value.version() == id.version(),
            "The id's version is invalided while Index the id! id: {:?}",
            id
        );

        &self.dense[sparse_value.idx() as usize]
    }
}

#[test]
fn sparse_set() {
    let mut entity_stroge = EntityStorage::new(128);
    let ea = entity_stroge.alloc();
    let ea_value = 10;
    let eb = entity_stroge.alloc();
    let eb_value = 20;
    let ec = entity_stroge.alloc();
    let ec_value = 30;
    let ed = entity_stroge.alloc();
    let ed_value = 40;

    let mut sparset_set = SparseSet::new(128);
    sparset_set.insert(ea, ea_value);
    sparset_set.insert(eb, eb_value);
    sparset_set.insert(ec, ec_value);
    sparset_set.insert(ed, ed_value);

    let moved = entity_stroge.free(eb).unwrap();
    sparset_set.remove(eb, moved);
    let moved = entity_stroge.free(ed).unwrap();
    sparset_set.remove(ed, moved);

    let ef = entity_stroge.alloc();
    let ef_value = 50;
    sparset_set.insert(ef, ef_value);

    debug_assert_eq!(ef, Entity::new(3, 1, EntityFlag::Default));
    debug_assert_eq!(entity_stroge.get_entity(&ea), &ea);
    debug_assert_eq!(entity_stroge.get_entity(&ec), &ec);

    debug_assert_eq!(sparset_set.get(ef), Some(&ef_value));
    debug_assert_eq!(sparset_set.get(ea), Some(&ea_value));
    debug_assert_eq!(sparset_set.get(ec), Some(&ec_value));
}
