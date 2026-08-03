use crate::{
    debug::MaybeLocation,
    ecs::{
        change_detection::Tick,
        component::{ComponentIds, Components, ComponentsRegistrator},
        entity::{Entity, EntityIndex},
        storage::{TableBuilder, TableId, TableRow, Tables},
    },
    ptr::OwningPtr,
};

// #[derive(Component)]
struct W<T>(T);

#[test]
fn only_one_empty_table() {
    let components = Components::default();
    let mut tables = Tables::default();

    let component_ids = &[];
    // SAFETY: component_ids is empty, so we know it cannot reference invalid component IDs
    let table_id = unsafe { tables.get_id_or_insert(component_ids, &components) };

    assert_eq!(table_id, TableId::empty());
}

#[test]
fn table() {
    let mut components = Components::default();
    let mut componentids = ComponentIds::default();
    // SAFETY: They are both new.
    let mut registrator = unsafe { ComponentsRegistrator::new(&mut components, &mut componentids) };
    let component_id = registrator.register_component::<W<TableRow>>();
    let columns = &[component_id];
    let mut table = TableBuilder::with_capacity(0, columns.len())
        .add_column(components.get_info(component_id).unwrap())
        .build();
    let entities = (0..200)
        .map(|index| Entity::from_index(EntityIndex::from_raw_u32(index).unwrap()))
        .collect::<Vec<_>>();
    for entity in &entities {
        // SAFETY: we allocate and immediately set data afterwards
        unsafe {
            let row = table.allocate(*entity);
            let value: W<TableRow> = W(row);
            OwningPtr::make(value, |value_ptr| {
                table.get_column_mut(component_id).unwrap().initialize(
                    row,
                    value_ptr,
                    Tick::new(0),
                    MaybeLocation::caller(),
                );
            });
        };
    }

    assert_eq!(table.entity_capacity(), 256);
    assert_eq!(table.entity_count(), 200);
}
