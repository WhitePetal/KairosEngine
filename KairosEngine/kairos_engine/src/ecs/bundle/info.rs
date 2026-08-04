use std::mem::MaybeUninit;

use indexmap::{IndexMap, IndexSet};

use crate::{
    collections::FixedHashSet,
    debug::{DebugCheckedUnwrap, MaybeLocation},
    ecs::{
        archetype::{BundleComponentStatus, ComponentStatus},
        change_detection::Tick,
        component::{ComponentId, Components, RequiredComponentConstructor, StorageType},
        entity::Entity,
        storage::{SparseSetIndex, SparseSets, Storages, Table, TableRow},
        world::EntityWorldMut,
    },
    hash::FixedHasher,
    ptr::{MovingPtr, OwningPtr},
};

/// For a specific [`World`], this stores a unique value identifying a type of a registered [`Bundle`].
///
/// [`World`]: crate::world::World
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct BundleId(usize);

impl BundleId {
    /// Returns the index of the associated [`Bundle`] type.
    ///
    /// Note that this is unique per-world, and should not be reused across them.
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}

impl SparseSetIndex for BundleId {
    #[inline]
    fn sparse_set_index(&self) -> usize {
        self.index()
    }

    #[inline]
    fn get_sparse_set_index(value: usize) -> Self {
        Self(value)
    }
}

/// What to do on insertion if a component already exists.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum InsertMode {
    /// Any existing components of a matching type will be overwritten.
    Replace,
    /// Any existing components of a matching type will be left unchanged.
    Keep,
}

/// Stores metadata associated with a specific type of [`Bundle`] for a given [`World`].
///
/// [`World`]: crate::world::World
pub struct BundleInfo {
    pub(super) id: BundleId,

    /// The list of all components contributed by the bundle (including Required Components). This is in
    /// the order `[EXPLICIT_COMPONENTS][REQUIRED_COMPONENTS]`
    ///
    /// # Safety
    /// Every ID in this list must be valid within the World that owns the [`BundleInfo`],
    /// must have its storage initialized (i.e. columns created in tables, sparse set created),
    /// and the range (0..`explicit_components_len`) must be in the same order as the source bundle
    /// type writes its components in.
    pub(super) contributed_component_ids: Box<[ComponentId]>,

    /// The list of constructors for all required components indirectly contributed by this bundle.
    pub(super) required_component_constructors: Box<[RequiredComponentConstructor]>,
}

impl BundleInfo {
    unsafe fn new(
        bundle_type_name: &'static str,
        storages: &mut Storages,
        components: &mut Components,
        mut component_ids: Vec<ComponentId>,
        id: BundleId,
    ) -> BundleInfo {
        let explicit_component_ids = component_ids
            .iter()
            .copied()
            .collect::<IndexSet<_, FixedHasher>>();

        // check for duplicates
        if explicit_component_ids.len() != component_ids.len() {
            let mut seen = <FixedHashSet<_>>::default();
            let mut dups = Vec::new();
            for id in component_ids {
                if !seen.insert(id) {
                    dups.push(id);
                }
            }

            let names = dups
                .into_iter()
                .map(|id| {
                    // SAFETY: the caller ensures component_id is valid.
                    unsafe { components.get_info_unchecked(id).name() }
                })
                .collect::<Vec<_>>();

            panic!("Bundle {bundle_type_name} has duplicate components: {names:?}");
        }

        let mut depth_first_components = IndexMap::<_, _, FixedHasher>::default();
        for &component_id in &component_ids {
            // SAFETY: caller has verified that all ids are valid
            let info = unsafe { components.get_info_unchecked(component_id) };

            for (&required_id, required_component) in &info.required_components().all {
                depth_first_components
                    .entry(required_id)
                    .or_insert_with(|| required_component.clone());
            }

            storages.prepare_component(info);
        }

        let required_components = depth_first_components
            .into_iter()
            .filter(|&(required_id, _)| !explicit_component_ids.contains(&required_id))
            .inspect(|&(required_id, _)| {
                // SAFETY: These ids came out of the passed `components`, so they must be valid.
                storages.prepare_component(unsafe { components.get_info_unchecked(required_id) });
                component_ids.push(required_id);
            })
            .map(|(_, required_component)| required_component.constructor)
            .collect::<Box<_>>();

        // SAFETY: The caller ensures that component_ids:
        // - is valid for the associated world
        // - has had its storage initialized
        // - is in the same order as the source bundle type
        BundleInfo {
            id,
            contributed_component_ids: component_ids.into(),
            required_component_constructors: required_components,
        }
    }

    /// Returns a value identifying the associated [`Bundle`] type.
    #[inline]
    pub const fn id(&self) -> BundleId {
        self.id
    }

    /// Returns the length of the explicit components part of the [`contributed_components`](Self::contributed_components) list.
    #[inline]
    pub(super) fn explicit_components_len(&self) -> usize {
        self.contributed_component_ids.len() - self.required_component_constructors.len()
    }

    /// Returns the [ID](ComponentId) of each component explicitly defined in this bundle (ex: Required Components are excluded).
    ///
    /// For all components contributed by this bundle (including Required Components), see [`BundleInfo::contributed_components`]
    #[inline]
    pub fn explicit_components(&self) -> &[ComponentId] {
        &self.contributed_component_ids[0..self.explicit_components_len()]
    }

    /// Returns the [ID](ComponentId) of each Required Component needed by this bundle. This _does not include_ Required Components that are
    /// explicitly provided by the bundle.
    #[inline]
    pub fn required_components(&self) -> &[ComponentId] {
        &self.contributed_component_ids[self.explicit_components_len()..]
    }

    /// Returns the [ID](ComponentId) of each component contributed by this bundle. This includes Required Components.
    ///
    /// For only components explicitly defined in this bundle, see [`BundleInfo::explicit_components`]
    #[inline]
    pub fn contributed_components(&self) -> &[ComponentId] {
        &self.contributed_component_ids
    }

    /// Returns an iterator over the [ID](ComponentId) of each component explicitly defined in this bundle (ex: this excludes Required Components).
    /// To iterate all components contributed by this bundle (including Required Components), see [`BundleInfo::iter_contributed_components`]
    #[inline]
    pub fn iter_explicit_components(&self) -> impl Iterator<Item = ComponentId> + Clone + '_ {
        self.explicit_components().iter().copied()
    }

    /// Returns an iterator over the [ID](ComponentId) of each component explicitly defined in this bundle (ex: this excludes Required Components).
    /// To iterate all components contributed by this bundle (including Required Components), see [`BundleInfo::iter_contributed_components`]
    #[inline]
    pub fn iter_contributed_compoens(&self) -> impl Iterator<Item = ComponentId> + Clone + '_ {
        self.contributed_components().iter().copied()
    }

    /// Returns an iterator over the [ID](ComponentId) of each Required Component needed by this bundle. This _does not include_ Required Components that are
    /// explicitly provided by the bundle.
    pub fn iter_required_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.required_components().iter().copied()
    }

    /// This writes components from a given [`Bundle`] to the given entity.
    ///
    /// # Safety
    ///
    /// `bundle_component_status` must return the "correct" [`ComponentStatus`] for each component
    /// in the [`Bundle`], with respect to the entity's original archetype (prior to the bundle being added).
    ///
    /// For example, if the original archetype already has `ComponentA` and `T` also has `ComponentA`, the status
    /// should be `Existing`. If the original archetype does not have `ComponentA`, the status should be `Added`.
    ///
    /// When "inserting" a bundle into an existing entity, [`ArchetypeAfterBundleInsert`](crate::archetype::SpawnBundleStatus)
    /// should be used, which will report `Added` vs `Existing` status based on the current archetype's structure.
    ///
    /// When spawning a bundle, [`SpawnBundleStatus`](crate::archetype::SpawnBundleStatus) can be used instead,
    /// which removes the need to look up the [`ArchetypeAfterBundleInsert`](crate::archetype::ArchetypeAfterBundleInsert)
    /// in the archetype graph, which requires ownership of the entity's current archetype.
    ///
    /// Regardless of how this is used, [`apply_effect`] must be called at most once on `bundle` after this function is
    /// called if `T::Effect: !NoBundleEffect` before returning to user-space safe code before returning to user-space safe code.
    /// This is currently only doable via use of [`MovingPtr::partial_move`].
    ///
    /// `table` must be the "new" table for `entity`. `table_row` must have space allocated for the
    /// `entity`, `bundle` must match this [`BundleInfo`]'s type
    ///
    /// [`apply_effect`]: crate::bundle::DynamicBundle::apply_effect
    #[inline]
    pub(super) unsafe fn write_components<'a, T: DynamicBundle, S: BundleComponentStatus>(
        &self,
        table: &mut Table,
        sparse_sets: &mut SparseSets,
        bundle_component_status: &S,
        required_components: impl Iterator<Item = &'a RequiredComponentConstructor>,
        entity: Entity,
        table_row: TableRow,
        change_tick: Tick,
        bundle: MovingPtr<'_, T>,
        insert_mode: InsertMode,
        caller: MaybeLocation,
    ) {
        // NOTE: get_components calls this closure on each component in "bundle order".
        // bundle_info.component_ids are also in "bundle order"
        let mut bundle_component = 0;
        T::get_components(bundle, &mut |storage_type, component_ptr| {
            let component_id = *self
                .contributed_component_ids
                .get_unchecked(bundle_component);
            // SAFETY: bundle_component is a valid index for this bundle
            let status = unsafe { bundle_component_status.get_status(bundle_component) };
            match storage_type {
                StorageType::Table => {
                    // SAFETY: If component_id is in self.component_ids, BundleInfo::new ensures that
                    // the target table contains the component.
                    let column =
                        unsafe { table.get_column_mut(component_id).debug_checked_unwrap() };
                    match (status, insert_mode) {
                        (ComponentStatus::Added, _) => {
                            column.initialize(table_row, component_ptr, change_tick, caller);
                        }
                        (ComponentStatus::Existing, InsertMode::Replace) => {
                            column.replace(table_row, component_ptr, change_tick, caller);
                        }
                        (ComponentStatus::Existing, InsertMode::Keep) => {
                            if let Some(drop_fn) = table.get_drop_for(component_id) {
                                drop_fn(component_ptr);
                            }
                        }
                    }
                }
                StorageType::SparseSet => {
                    // SAFETY: If component_id is in self.component_ids, BundleInfo::new ensures that
                    // a sparse set exists for the component.
                    let sparse_set =
                        unsafe { sparse_sets.get_mut(component_id).debug_checked_unwrap() };
                    match (status, insert_mode) {
                        (ComponentStatus::Added, _) | (_, InsertMode::Replace) => {
                            sparse_set.insert(entity, component_ptr, change_tick, caller);
                        }
                        (ComponentStatus::Existing, InsertMode::Keep) => {
                            if let Some(drop_fn) = sparse_set.get_drop() {
                                drop_fn(component_ptr);
                            }
                        }
                    }
                }
            }
            bundle_component += 1;
        });

        for required_component in required_components {
            required_component.initialize(
                table,
                sparse_sets,
                change_tick,
                table_row,
                entity,
                caller,
            );
        }
    }

    /// Internal method to initialize a required component from an [`OwningPtr`]. This should ultimately be called
    /// in the context of [`BundleInfo::write_components`], via [`RequiredComponentConstructor::initialize`].
    ///
    /// # Safety
    ///
    /// `component_ptr` must point to a required component value that matches the given `component_id`. The `storage_type` must match
    /// the type associated with `component_id`. The `entity` and `table_row` must correspond to an entity with an uninitialized
    /// component matching `component_id`.
    ///
    /// This method _should not_ be called outside of [`BundleInfo::write_components`].
    /// For more information, read the [`BundleInfo::write_components`] safety docs.
    /// This function inherits the safety requirements defined there.
    pub(crate) unsafe fn initialize_required_component(
        table: &mut Table,
        sparse_sets: &mut SparseSets,
        change_tick: Tick,
        table_row: TableRow,
        entity: Entity,
        component_id: ComponentId,
        storage_type: StorageType,
        component_ptr: OwningPtr,
        caller: MaybeLocation,
    ) {
        match storage_type {
            StorageType::Table => {
                // SAFETY: If component_id is in required_components, BundleInfo::new requires that
                // the target table contains the component.
                let column = unsafe { table.get_column_mut(component_id).debug_checked_unwrap() };
                column.initialize(table_row, component_ptr, change_tick, caller);
            }
            StorageType::SparseSet => {
                // SAFETY: If component_id is in required_components, BundleInfo::new requires that
                // a sparse set exists for the component.
                let sparse_set =
                    unsafe { sparse_sets.get_mut(component_id).debug_checked_unwrap() };
                sparse_set.insert(entity, component_ptr, change_tick, caller);
            }
        }
    }
}

/// The parts from [`Bundle`] that don't require statically knowing the components of the bundle.
pub trait DynamicBundle: Sized {
    /// An operation on the entity that happens _after_ inserting this bundle.
    type Effect;

    /// Moves the components out of the bundle.
    ///
    /// # Safety
    /// For callers:
    /// - Must be called exactly once before `apply_effect`
    /// - The `StorageType` argument passed into `func` must be correct for the component being fetched.
    /// - `apply_effect` must be called exactly once after this has been called if `Effect: !NoBundleEffect`
    ///
    /// For implementors:
    ///  - Implementors of this function must convert `ptr` into pointers to individual components stored within
    ///    `Self` and call `func` on each of them in exactly the same order as [`Bundle::get_component_ids`] and
    ///    [`BundleFromComponents::from_components`].
    ///  - If any part of `ptr` is to be accessed in `apply_effect`, it must *not* be dropped at any point in this
    ///    function. Calling [`bevy_ptr::deconstruct_moving_ptr`] in this function automatically ensures this.
    ///
    /// [`Component`]: crate::component::Component
    // This function explicitly uses `MovingPtr` to avoid potentially large stack copies of the bundle
    // when inserting into ECS storage. See https://github.com/bevyengine/bevy/issues/20571 for more
    // information.
    unsafe fn get_components(
        ptr: MovingPtr<'_, Self>,
        func: &mut impl FnMut(StorageType, OwningPtr<'_>),
    );

    /// Applies the after-effects of spawning this bundle.
    ///
    /// This is applied after all residual changes to the [`World`], including flushing the internal command
    /// queue.
    ///
    /// # Safety
    /// For callers:
    /// - Must be called exactly once after `get_components` has been called.
    /// - `ptr` must point to the instance of `Self` that `get_components` was called on,
    ///   all of fields that were moved out of in `get_components` will not be valid anymore.
    ///
    /// For implementors:
    ///  - If any part of `ptr` is to be accessed in this function, it must *not* be dropped at any point in
    ///    `get_components`. Calling [`bevy_ptr::deconstruct_moving_ptr`] in `get_components` automatically
    ///    ensures this is the case.
    ///  - Note that `entity` may already have been despawned by hooks or observers at this point,
    ///    so check [`EntityWorldMut::is_spawned`] before trusting it.
    ///
    /// [`World`]: crate::world::World
    // This function explicitly uses `MovingPtr` to avoid potentially large stack copies of the bundle
    // when inserting into ECS storage. See https://github.com/bevyengine/bevy/issues/20571 for more
    // information.
    unsafe fn apply_effect(ptr: MovingPtr<'_, MaybeUninit<Self>>, entity: &mut EntityWorldMut);
}

/// A trait implemented for [`DynamicBundle::Effect`] implementations that do nothing. This is used as a type constraint for
/// [`Bundle`] APIs that do not / cannot run [`DynamicBundle::Effect`], such as "batch spawn" APIs.
pub trait NoBundleEffect {}
