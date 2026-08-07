use std::ptr::NonNull;

use crate::{
    debug::{DebugCheckedUnwrap, MaybeLocation},
    ecs::{
        archetype::{
            Archetype, ArchetypeAfterBundleInsert, ArchetypeCreated, ArchetypeId, Archetypes,
            ComponentStatus,
        },
        bundle::{ArchetypeMoveType, Bundle, BundleId, BundleInfo, InsertMode},
        change_detection::Tick,
        component::{Components, StorageType},
        entity::{Entity, EntityLocation},
        observer::Observers,
        relationship::RelationshipHookMode,
        storage::{SparseSets, Storages, Table, TableRow},
        world::{World, unsafe_world_cell::UnsafeWorldCell},
    },
    ptr::ConstNonNull,
};

// SAFETY: We have exclusive world access so our pointers can't be invalidated externally
pub(crate) struct BundleInserter<'w> {
    world: UnsafeWorldCell<'w>,
    bundle_info: ConstNonNull<BundleInfo>,
    archetype_after_insert: ConstNonNull<ArchetypeAfterBundleInsert>,
    archetype: NonNull<Archetype>,
    archetype_move_type: ArchetypeMoveType,
    change_tick: Tick,
}

impl<'w> BundleInserter<'w> {
    pub(crate) unsafe fn new<T: Bundle>(
        world: &'w mut World,
        archetype_id: ArchetypeId,
        change_tick: Tick,
    ) -> Self {
        let bundle_id = world.register_bundle_info::<T>();

        // SAFETY: We just ensured this bundle exists
        unsafe { Self::new_with_id(world, archetype_id, bundle_id, change_tick) }
    }

    /// Creates a new [`BundleInserter`].
    ///
    /// # Safety
    /// - `bundle_id` must correspond to an existing bundle in `world`.
    /// - `archetype_id` must correspond to a valid archetype in `world`.
    #[inline]
    pub(crate) unsafe fn new_with_id(
        world: &'w mut World,
        archetype_id: ArchetypeId,
        bundle_id: BundleId,
        change_tick: Tick,
    ) -> Self {
        // SAFETY: We will not make any accesses to the command queue, component or resource data of this world
        let bundle_info = world.bundles.get_unchecked(bundle_id);
        let bundle_id = bundle_info.id();
        let (new_archetype_id, is_new_created) = bundle_info.insert_bundle_into_archetype(
            &mut world.archetypes,
            &mut world.storages,
            &world.components,
            &world.observers,
            archetype_id,
        );

        // SAFETY:
        // - The caller ensures `archetype_id` is valid.
        // - `new_archetype_id` was just created or fetched from the archetype graph.
        let (archetype, new_archetype) = unsafe {
            world
                .archetypes
                .get_maybe_disjoint_mut(archetype_id, new_archetype_id)
        };

        // SAFETY: The edge is assured to be initialized when we called insert_bundle_into_archetype
        let archetype_after_insert = unsafe {
            archetype
                .edges()
                .get_archetype_after_bundle_insert_internal(bundle_id)
                .debug_checked_unwrap()
                .into()
        };

        let archetype_move_type = if let Some(new_archetype) = new_archetype {
            if archetype.table_id() == new_archetype.table_id() {
                ArchetypeMoveType::NewArchetypeNewTable {
                    new_archetype: new_archetype.into(),
                }
            } else {
                ArchetypeMoveType::NewArchetypeNewTable {
                    new_archetype: new_archetype.into(),
                }
            }
        } else {
            ArchetypeMoveType::SameArchetype
        };

        let inserter = Self {
            archetype: archetype.into(),
            archetype_after_insert,
            bundle_info: bundle_info.into(),
            archetype_move_type,
            change_tick,
            world: world.as_unsafe_world_cell(),
        };

        if is_new_created {
            inserter
                .world
                .into_deferred()
                .trigger(ArchetypeCreated(new_archetype_id));
        }
        inserter
    }

    // A non-generic prelude to insert used to minimize duplicated monomorphized code.
    // In combination with after_insert, this can reduce compile time of bevy by 10%.
    // We inline in release to avoid a minor perf loss.
    #[cfg_attr(not(debug_assertions), inline(always))]
    unsafe fn before_insert<'a>(
        entity: Entity,
        location: EntityLocation,
        insert_mode: InsertMode,
        caller: MaybeLocation,
        relationship_hook_mode: RelationshipHookMode,
        mut archetype: NonNull<Archetype>,
        archetype_after_insert: &ArchetypeAfterBundleInsert,
        world: &'a UnsafeWorldCell<'w>,
        archetype_move_type: &'a mut ArchetypeMoveType,
    ) -> (
        &'a Archetype,
        EntityLocation,
        &'a mut SparseSets,
        &'a mut Table,
        TableRow,
    ) {
        // SAFETY: All components in the bundle are guaranteed to exist in the World
        // as they must be initialized before creating the BundleInfo.
        unsafe {
            // SAFETY: Mutable references do not alias and will be dropped after this block
            let mut deferred_world = world.into_deferred();

            if insert_mode == InsertMode::Replace {
                let archetype = archetype.as_ref();
                let new_archetype = match archetype_move_type {
                    ArchetypeMoveType::SameArchetype => archetype,
                    ArchetypeMoveType::NewArchetypeSameTable { new_archetype }
                    | ArchetypeMoveType::NewArchetypeNewTable { new_archetype } => {
                        new_archetype.as_ref()
                    }
                };
                if archetype.has_discard_observer() {
                    todo!()
                }
            }
        }

        todo!()
    }
}

impl BundleInfo {
    /// Inserts a bundle into the given archetype and returns the resulting archetype and whether a new archetype was created.
    /// This could be the same [`ArchetypeId`], in the event that inserting the given bundle
    /// does not result in an [`Archetype`] change.
    ///
    /// Results are cached in the [`Archetype`] graph to avoid redundant work.
    ///
    /// # Safety
    /// `components` must be the same components as passed in [`Self::new`]
    pub(crate) unsafe fn insert_bundle_into_archetype(
        &self,
        archetypes: &mut Archetypes,
        storages: &mut Storages,
        components: &Components,
        observers: &Observers,
        archetype_id: ArchetypeId,
    ) -> (ArchetypeId, bool) {
        if let Some(archetype_after_insert_id) = archetypes[archetype_id]
            .edges()
            .get_archetype_after_bundle_insert(self.id)
        {
            return (archetype_after_insert_id, false);
        }
        let mut new_table_components = Vec::new();
        let mut new_sparse_set_components = Vec::new();
        let mut bundle_status = Vec::with_capacity(self.explicit_components_len());
        let mut added_required_components = Vec::new();
        let mut added = Vec::new();
        let mut existing = Vec::new();

        let current_archetype = &mut archetypes[archetype_id];
        for component_id in self.iter_explicit_components() {
            if current_archetype.contains(component_id) {
                bundle_status.push(ComponentStatus::Existing);
            } else {
                bundle_status.push(ComponentStatus::Added);
                added.push(component_id);
                // SAFETY: component_id exists
                let component_info = unsafe { components.get_info_unchecked(component_id) };
                match component_info.storage_type() {
                    StorageType::Table => new_table_components.push(component_id),
                    StorageType::SparseSet => new_sparse_set_components.push(component_id),
                }
            }
        }

        for (index, component_id) in self.iter_required_components().enumerate() {
            if !current_archetype.contains(component_id) {
                added_required_components.push(self.required_component_constructors[index].clone());
                added.push(component_id);
                // SAFETY: component_id exists
                let component_info = unsafe { components.get_info_unchecked(component_id) };
                match component_info.storage_type() {
                    StorageType::Table => new_table_components.push(component_id),
                    StorageType::SparseSet => new_sparse_set_components.push(component_id),
                }
            }
        }

        if new_table_components.is_empty() && new_sparse_set_components.is_empty() {
            let edges = current_archetype.edges_mut();
            // The archetype does not change when we insert this bundle.
            edges.cache_archetype_after_bundle_insert(
                self.id,
                archetype_id,
                bundle_status,
                added_required_components,
                added,
                existing,
            );
            (archetype_id, false)
        } else {
            let table_id;
            let table_components;
            let sparse_set_components;
            // The archetype changes when we insert this bundle. Prepare the new archetype and storages.
            {
                let current_archetype = &archetypes[archetype_id];
                table_components = if new_table_components.is_empty() {
                    // If there are no new table components, we can keep using this table.
                    table_id = current_archetype.table_id();
                    current_archetype.table_components().collect()
                } else {
                    new_table_components.extend(current_archetype.table_components());
                    // Sort to ignore order while hashing.
                    new_table_components.sort_unstable();

                    table_id = unsafe {
                        storages
                            .tables
                            .get_id_or_insert(&new_table_components, components)
                    };

                    new_table_components
                };

                sparse_set_components = if new_sparse_set_components.is_empty() {
                    current_archetype.sparse_set_components().collect()
                } else {
                    new_sparse_set_components.extend(current_archetype.sparse_set_components());
                    // Sort to ignore order while hashing.
                    new_sparse_set_components.sort_unstable();
                    new_sparse_set_components
                };
            };
            // SAFETY: ids in self must be valid
            let (new_archetype_id, is_new_created) = unsafe {
                archetypes.get_id_or_insert(
                    components,
                    observers,
                    table_id,
                    table_components,
                    sparse_set_components,
                )
            };

            // Add an edge from the old archetype to the new archetype.
            archetypes[archetype_id]
                .edges_mut()
                .cache_archetype_after_bundle_insert(
                    self.id(),
                    new_archetype_id,
                    bundle_status,
                    added_required_components,
                    added,
                    existing,
                );
            (new_archetype_id, is_new_created)
        }
    }
}
