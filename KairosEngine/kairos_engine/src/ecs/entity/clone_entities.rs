use std::{any::TypeId, cell::LazyCell, collections::VecDeque, ops::Range};

use bumpalo::Bump;
use derive_more::From;

use crate::{
    collections::{FixedHashMap, FixedHashSet},
    debug::{DebugCheckedUnwrap, DebugName, MaybeLocation},
    ecs::{
        archetype::Archetype,
        bundle::{BundleRemover, InsertMode},
        component::{
            Component, ComponentCloneBehavior, ComponentCloneFn, ComponentId, ComponentInfo,
        },
        entity::{Entity, EntityAllocator, EntityHashMap, EntityMapper},
        relationship::RelationshipHookMode,
        world::World,
    },
    ptr::{Ptr, PtrMut},
};

/// Provides read access to the source component (the component being cloned) in a [`ComponentCloneFn`].
pub struct SourceComponent<'a> {
    ptr: Ptr<'a>,
    info: &'a ComponentInfo,
}

impl<'a> SourceComponent<'a> {
    /// Returns a reference to the component on the source entity.
    ///
    /// Will return `None` if `ComponentId` of requested component does not match `ComponentId` of source component
    pub fn read<C: Component>(&self) -> Option<&C> {
        if self
            .info
            .type_id()
            .is_some_and(|id| id == TypeId::of::<C>())
        {
            // SAFETY:
            // - Components and ComponentId are from the same world
            // - source_component_ptr holds valid data of the type referenced by ComponentId
            unsafe { Some(self.ptr.deref::<C>()) }
        } else {
            None
        }
    }

    /// Returns the "raw" pointer to the source component.
    pub fn ptr(&self) -> Ptr<'a> {
        self.ptr
    }

    // TODO!: need relfect
    // /// Returns a reference to the component on the source entity as [`&dyn Reflect`](bevy_reflect::Reflect).
    // ///
    // /// Will return `None` if:
    // /// - World does not have [`AppTypeRegistry`](`crate::reflect::AppTypeRegistry`).
    // /// - Component does not implement [`ReflectFromPtr`](bevy_reflect::ReflectFromPtr).
    // /// - Component is not registered.
    // /// - Component does not have [`TypeId`]
    // /// - Registered [`ReflectFromPtr`](bevy_reflect::ReflectFromPtr)'s [`TypeId`] does not match component's [`TypeId`]
    // #[cfg(feature = "bevy_reflect")]
    // pub fn read_reflect(
    //     &self,
    //     registry: &bevy_reflect::TypeRegistry,
    // ) -> Option<&dyn bevy_reflect::Reflect> {
    //     let type_id = self.info.type_id()?;
    //     let reflect_from_ptr = registry.get_type_data::<bevy_reflect::ReflectFromPtr>(type_id)?;
    //     if reflect_from_ptr.type_id() != type_id {
    //         return None;
    //     }
    //     // SAFETY: `source_component_ptr` stores data represented by `component_id`, which we used to get `ReflectFromPtr`.
    //     unsafe { Some(reflect_from_ptr.as_reflect(self.ptr)) }
    // }
}

/// Context for component clone handlers.
///
/// Provides fast access to useful resources like [`AppTypeRegistry`](crate::reflect::AppTypeRegistry)
/// and allows component clone handler to get information about component being cloned.
pub struct ComponentCloneCtx<'a, 'b> {
    component_id: ComponentId,
    target_component_written: bool,
    target_component_moved: bool,
    bundle_scratch: &'a mut BundleScratchSpace<'b>,
    bundle_scratch_allocator: &'b Bump,
    allocator: &'a EntityAllocator,
    source: Entity,
    target: Entity,
    component_info: &'a ComponentInfo,
    state: &'a mut EntityClonerState,
    mapper: &'a mut dyn EntityMapper,
    // #[cfg(feature = "bevy_reflect")]
    // type_registry: Option<&'a crate::reflect::AppTypeRegistry>,
    // #[cfg(not(feature = "bevy_reflect"))]
    #[expect(dead_code, reason = "type_registry is only used with kairos_reflect")]
    type_registry: Option<&'a ()>,
}

impl<'a, 'b> ComponentCloneCtx<'a, 'b> {
    /// Create a new instance of `ComponentCloneCtx` that can be passed to component clone handlers.
    ///
    /// # Safety
    /// Caller must ensure that:
    /// - `component_info` corresponds to the `component_id` in the same world,.
    /// - `source_component_ptr` points to a valid component of type represented by `component_id`.
    unsafe fn new(
        component_id: ComponentId,
        source: Entity,
        target: Entity,
        bundle_scratch_allocator: &'b Bump,
        bundle_scratch: &'a mut BundleScratchSpace<'b>,
        allocator: &'a EntityAllocator,
        component_info: &'a ComponentInfo,
        entity_cloner: &'a mut EntityClonerState,
        mapper: &'a mut dyn EntityMapper,
        // #[cfg(feature = "bevy_reflect")]
        // type_registry: Option<&'a crate::reflect::AppTypeRegistry>,
        // #[cfg(not(feature = "bevy_reflect"))]
        type_registry: Option<&'a ()>,
    ) -> Self {
        Self {
            component_id,
            source,
            target,
            bundle_scratch,
            target_component_written: false,
            target_component_moved: false,
            bundle_scratch_allocator,
            allocator,
            mapper,
            component_info,
            state: entity_cloner,
            type_registry,
        }
    }

    /// Returns true if [`write_target_component`](`Self::write_target_component`) was called before.
    pub fn target_component_written(&self) -> bool {
        self.target_component_written
    }

    /// Returns `true` if used in moving context
    pub fn moving(&self) -> bool {
        self.state.move_components
    }

    /// Returns the current source entity.
    pub fn source(&self) -> Entity {
        self.source
    }

    /// Returns the current target entity.
    pub fn target(&self) -> Entity {
        self.target
    }

    /// Returns the [`ComponentInfo`] of the component being cloned.
    pub fn component_info(&self) -> &ComponentInfo {
        self.component_info
    }

    /// Returns true if the [`EntityCloner`] is configured to recursively clone entities. When this is enabled,
    /// entities stored in a cloned entity's [`RelationshipTarget`](crate::relationship::RelationshipTarget) component with
    /// [`RelationshipTarget::LINKED_SPAWN`](crate::relationship::RelationshipTarget::LINKED_SPAWN) will also be cloned.
    #[inline]
    pub fn linked_cloning(&self) -> bool {
        self.state.linked_cloning
    }

    /// Returns this context's [`EntityMapper`].
    pub fn entity_mapper(&mut self) -> &mut dyn EntityMapper {
        self.mapper
    }

    /// Writes component data to target entity.
    ///
    /// # Panics
    /// This will panic if:
    /// - Component has already been written once.
    /// - Component being written is not registered in the world.
    /// - `ComponentId` of component being written does not match expected `ComponentId`.
    pub fn write_target_component<C: Component>(&mut self, mut component: C) {
        C::map_entities(&mut component, &mut self.mapper);
        let debug_name = DebugName::type_name::<C>();
        let short_name = debug_name.shortname();
        if self.target_component_written {
            panic!("Trying to write component `{short_name}` multiple times")
        }
        if self
            .component_info
            .type_id()
            .is_none_or(|id| id != TypeId::of::<C>())
        {
            panic!("TypeId of component `{short_name}` does not match source component TypeId")
        };

        unsafe {
            self.bundle_scratch
                .push(self.bundle_scratch_allocator, self.component_id, component);
        }
        self.target_component_written = true;
    }

    /// Writes component data to target entity by providing a pointer to source component data.
    ///
    /// # Safety
    /// Caller must ensure that the passed in `ptr` references data that corresponds to the type of the source / target [`ComponentId`].
    /// `ptr` must also contain data that the written component can "own" (for example, this should not directly copy non-Copy data).
    ///
    /// # Panics
    /// This will panic if component has already been written once.
    pub unsafe fn write_target_component_ptr(&mut self, ptr: Ptr) {
        if self.target_component_written {
            panic!("Trying to write component multiple times")
        }
        let layout = self.component_info.layout();
        let target_ptr = self.bundle_scratch_allocator.alloc_layout(layout);
        unsafe {
            std::ptr::copy_nonoverlapping(ptr.as_ptr(), target_ptr.as_ptr(), layout.size());
            self.bundle_scratch
                .push_ptr(self.component_id, PtrMut::new(target_ptr));
        }
        self.target_component_written = true;
    }

    // /// Writes component data to target entity.
    // ///
    // /// # Panics
    // /// This will panic if:
    // /// - World does not have [`AppTypeRegistry`](`crate::reflect::AppTypeRegistry`).
    // /// - Component does not implement [`ReflectFromPtr`](bevy_reflect::ReflectFromPtr).
    // /// - Source component does not have [`TypeId`].
    // /// - Passed component's [`TypeId`] does not match source component [`TypeId`].
    // /// - Component has already been written once.
    // #[cfg(feature = "bevy_reflect")]
    // pub fn write_target_component_reflect(&mut self, component: Box<dyn bevy_reflect::Reflect>) {
    //     if self.target_component_written {
    //         panic!("Trying to write component multiple times")
    //     }
    //     let source_type_id = self
    //         .component_info
    //         .type_id()
    //         .expect("Source component must have TypeId");
    //     let component_type_id = component.type_id();
    //     if source_type_id != component_type_id {
    //         panic!("Passed component TypeId does not match source component TypeId")
    //     }
    //     let component_layout = self.component_info.layout();

    //     let component_data_ptr = Box::into_raw(component).cast::<u8>();
    //     let target_component_data_ptr =
    //         self.bundle_scratch_allocator.alloc_layout(component_layout);
    //     // SAFETY:
    //     // - target_component_data_ptr and component_data have the same data type.
    //     // - component_data_ptr has layout of component_layout
    //     unsafe {
    //         core::ptr::copy_nonoverlapping(
    //             component_data_ptr,
    //             target_component_data_ptr.as_ptr(),
    //             component_layout.size(),
    //         );
    //         self.bundle_scratch
    //             .push_ptr(self.component_id, PtrMut::new(target_component_data_ptr));

    //         if component_layout.size() > 0 {
    //             // Ensure we don't attempt to deallocate zero-sized components
    //             alloc::alloc::dealloc(component_data_ptr, component_layout);
    //         }
    //     }

    //     self.target_component_written = true;
    // }

    // /// Returns [`AppTypeRegistry`](`crate::reflect::AppTypeRegistry`) if it exists in the world.
    // ///
    // /// NOTE: Prefer this method instead of manually reading the resource from the world.
    // #[cfg(feature = "bevy_reflect")]
    // pub fn type_registry(&self) -> Option<&crate::reflect::AppTypeRegistry> {
    //     self.type_registry
    // }

    /// Queues the `entity` to be cloned by the current [`EntityCloner`]
    pub fn queue_entity_clone(&mut self, entity: Entity) {
        let target = self.allocator.alloc();
        self.mapper.set_mapped(entity, target);
        self.state.clone_queue.push_back(entity);
    }

    /// Queues a deferred clone operation, which will run with exclusive [`World`] access immediately after calling the clone handler for each component on an entity.
    /// This exists, despite its similarity to [`Commands`](crate::system::Commands), to provide access to the entity mapper in the current context.
    pub fn queue_deferred(
        &mut self,
        deferred: impl FnOnce(&mut World, &mut dyn EntityMapper) + 'static,
    ) {
        self.state.deferred_commands.push_back(Box::new(deferred));
    }

    /// Marks component as moved and it's `drop` won't run.
    fn move_component(&mut self) {
        self.target_component_moved = true;
        self.target_component_written = true;
    }
}

/// A configuration determining how to clone entities. This can be built using [`EntityCloner::build_opt_out`]/
/// [`opt_in`](EntityCloner::build_opt_in), which
/// returns an [`EntityClonerBuilder`].
///
/// After configuration is complete an entity can be cloned using [`Self::clone_entity`].
///
///```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::entity::EntityCloner;
///
/// #[derive(Component, Clone, PartialEq, Eq)]
/// struct A {
///     field: usize,
/// }
///
/// let mut world = World::default();
///
/// let component = A { field: 5 };
///
/// let entity = world.spawn(component.clone()).id();
/// let entity_clone = world.spawn_empty().id();
///
/// EntityCloner::build_opt_out(&mut world).clone_entity(entity, entity_clone);
///
/// assert!(world.get::<A>(entity_clone).is_some_and(|c| *c == component));
///```
///
/// # Default cloning strategy
/// By default, all types that derive [`Component`] and implement either [`Clone`] or `Reflect` (with `ReflectComponent`) will be cloned
/// (with `Clone`-based implementation preferred in case component implements both).
///
/// It should be noted that if `Component` is implemented manually or if `Clone` implementation is conditional
/// (like when deriving `Clone` for a type with a generic parameter without `Clone` bound),
/// the component will be cloned using the [default cloning strategy](crate::component::ComponentCloneBehavior::global_default_fn).
/// To use `Clone`-based handler ([`ComponentCloneBehavior::clone`]) in this case it should be set manually using one
/// of the methods mentioned in the [Clone Behaviors](#Clone-Behaviors) section
///
/// Here's an example of how to do it using [`clone_behavior`](Component::clone_behavior):
/// ```
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::component::{StorageType, ComponentCloneBehavior, Mutable};
/// #[derive(Clone, Component)]
/// #[component(clone_behavior = clone::<Self>())]
/// struct SomeComponent;
///
/// ```
///
/// # Clone Behaviors
/// [`EntityCloner`] clones entities by cloning components using [`ComponentCloneBehavior`], and there are multiple layers
/// to decide which handler to use for which component. The overall hierarchy looks like this (priority from most to least):
/// 1. local overrides using [`EntityClonerBuilder::override_clone_behavior`]
/// 2. component-defined handler using [`Component::clone_behavior`]
/// 3. default handler override using [`EntityClonerBuilder::with_default_clone_fn`].
/// 4. reflect-based or noop default clone handler depending on if `bevy_reflect` feature is enabled or not.
///
/// # Moving components
/// [`EntityCloner`] can be configured to move components instead of cloning them by using [`EntityClonerBuilder::move_components`].
/// In this mode components will be moved - removed from source entity and added to the target entity.
///
/// Components with [`ComponentCloneBehavior::Ignore`] clone behavior will not be moved, while components that
/// have a [`ComponentCloneBehavior::Custom`] clone behavior will be cloned using it and then removed from the source entity.
/// All other components will be bitwise copied from the source entity onto the target entity and then removed without dropping.
///
/// Choosing to move components instead of cloning makes [`EntityClonerBuilder::with_default_clone_fn`] ineffective since it's replaced by
/// move handler for components that have [`ComponentCloneBehavior::Default`] clone behavior.
///
/// Note that moving components still triggers `on_remove` hooks/observers on source entity and `on_insert`/`on_add` hooks/observers on the target entity.
pub struct EntityCloner {
    filter: EntityClonerFilter,
    state: EntityClonerState,
}

/// An expandable scratch space for defining a dynamic bundle.
struct BundleScratchSpace<'a> {
    component_ids: Vec<ComponentId>,
    component_ptrs: Vec<PtrMut<'a>>,
}

impl<'a> BundleScratchSpace<'a> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            component_ids: Vec::with_capacity(capacity),
            component_ptrs: Vec::with_capacity(capacity),
        }
    }

    /// Pushes the `ptr` component onto this storage with the given `id` [`ComponentId`].
    ///
    /// # Safety
    /// The `id` [`ComponentId`] must match the component `ptr` for whatever [`World`] this scratch will
    /// be written to. `ptr` must contain valid uniquely-owned data that matches the type of component referenced
    /// in `id`.
    pub(crate) unsafe fn push_ptr(&mut self, id: ComponentId, ptr: PtrMut<'a>) {
        self.component_ids.push(id);
        self.component_ptrs.push(ptr);
    }

    /// Pushes the `C` component onto this storage with the given `id` [`ComponentId`], using the given `bump` allocator.
    ///
    /// # Safety
    /// The `id` [`ComponentId`] must match the component `C` for whatever [`World`] this scratch will
    /// be written to.
    pub(crate) unsafe fn push<C: Component>(
        &mut self,
        allocator: &'a Bump,
        id: ComponentId,
        component: C,
    ) {
        let component_ref = allocator.alloc(component);
        self.component_ids.push(id);
        self.component_ptrs.push(PtrMut::from(component_ref));
    }

    pub(crate) unsafe fn write(
        self,
        world: &mut World,
        entity: Entity,
        relationship_hook_insert_mode: RelationshipHookMode,
    ) {
        // SAFETY:
        // - All `component_ids` are from the same world as `entity`
        // - All `component_data_ptrs` are valid types represented by `component_ids`
        unsafe {
            world.entity_mut(entity).insert_by_ids_internal(
                &self.component_ids,
                self.component_ptrs.into_iter().map(|ptr| ptr.promote()),
                relationship_hook_insert_mode,
            );
        }
    }
}

impl EntityCloner {
    /// Returns a new [`EntityClonerBuilder`] using the given `world` with the [`OptOut`] configuration.
    ///
    /// This builder tries to clone every component from the source entity except for components that were
    /// explicitly denied, for example by using the [`deny`](EntityClonerBuilder<OptOut>::deny) method.
    ///
    /// Required components are not considered by denied components and must be explicitly denied as well if desired.
    pub fn build_opt_out(world: &mut World) -> EntityClonerBuilder<'_, OptOut> {
        EntityClonerBuilder {
            world,
            filter: Default::default(),
            state: Default::default(),
        }
    }

    /// Returns a new [`EntityClonerBuilder`] using the given `world` with the [`OptIn`] configuration.
    ///
    /// This builder tries to clone every component that was explicitly allowed from the source entity,
    /// for example by using the [`allow`](EntityClonerBuilder<OptIn>::allow) method.
    ///
    /// Components allowed to be cloned through this builder would also allow their required components,
    /// which will be cloned from the source entity only if the target entity does not contain them already.
    /// To skip adding required components see [`without_required_components`](EntityClonerBuilder<OptIn>::without_required_components).
    pub fn build_opt_in(world: &mut World) -> EntityClonerBuilder<'_, OptIn> {
        EntityClonerBuilder {
            world,
            filter: Default::default(),
            state: Default::default(),
        }
    }

    /// Returns `true` if this cloner is configured to clone entities referenced in cloned components via [`RelationshipTarget::LINKED_SPAWN`](crate::relationship::RelationshipTarget::LINKED_SPAWN).
    /// This will produce "deep" / recursive clones of relationship trees that have "linked spawn".
    #[inline]
    pub fn linked_cloning(&self) -> bool {
        self.state.linked_cloning
    }

    /// Clones and inserts components from the `source` entity into `target` entity using the stored configuration.
    /// If this [`EntityCloner`] has [`EntityCloner::linked_cloning`], then it will recursively spawn entities as defined
    /// by [`RelationshipTarget`](crate::relationship::RelationshipTarget) components with
    /// [`RelationshipTarget::LINKED_SPAWN`](crate::relationship::RelationshipTarget::LINKED_SPAWN)
    #[track_caller]
    pub fn clone_entity(&mut self, world: &mut World, source: Entity, target: Entity) {
        let mut map = EntityHashMap::<Entity>::new();
        map.set_mapped(source, target);
        self.clone_entity_mapped(world, source, &mut map);
    }

    /// Clones the entity into whatever entity `mapper` chooses for it.
    pub fn clone_entity_mapped(
        &mut self,
        world: &mut World,
        source: Entity,
        mapper: &mut dyn EntityMapper,
    ) -> Entity {
        Self::clone_entity_mapped_internal(&mut self.state, &mut self.filter, world, source, mapper)
    }

    #[track_caller]
    #[inline]
    fn clone_entity_mapped_internal(
        state: &mut EntityClonerState,
        fillter: &mut impl CloneByFilter,
        world: &mut World,
        source: Entity,
        mapper: &mut dyn EntityMapper,
    ) -> Entity {
        let target = Self::clone_entity_internal(
            state,
            fillter,
            world,
            source,
            mapper,
            RelationshipHookMode::Run,
        );
        let child_hook_insert_mode = if state.linked_cloning {
            // When spawning "linked relationships", we want to ignore hooks for relationships we are spawning, while
            // still registering with original relationship targets that are "not linked" to the current recursive spawn.
            RelationshipHookMode::RunIfNotLinked
        } else {
            // If we are not cloning "linked relationships" recursively, then we want any cloned relationship components to
            // register themselves with their original relationship target.
            RelationshipHookMode::Run
        };
        loop {
            let queued = state.clone_queue.pop_front();
            if let Some(queued) = queued {
                Self::clone_entity_internal(
                    state,
                    fillter,
                    world,
                    queued,
                    mapper,
                    child_hook_insert_mode,
                );
            } else {
                break;
            }
        }
        target
    }

    /// Clones and inserts components from the `source` entity into the entity mapped by `mapper` from `source` using the stored configuration.
    #[track_caller]
    fn clone_entity_internal(
        state: &mut EntityClonerState,
        filter: &mut impl CloneByFilter,
        world: &mut World,
        source: Entity,
        mapper: &mut dyn EntityMapper,
        relationship_hook_insert_mode: RelationshipHookMode,
    ) -> Entity {
        let target = mapper.get_mapped(source);
        // The target may need to be constructed if it hasn't been already.
        // If this fails, it either didn't need to be constructed (ok) or doesn't exist (caught better later).
        let _ = world.spawn_empty_at(target);

        // PERF: reusing allocated space across clones would be more efficient. Consider an allocation model similar to `Commands`.
        let bundle_scratch_allocator = Bump::new();
        let mut bundle_scratch: BundleScratchSpace;
        let mut moved_components: Vec<ComponentId> = Vec::new();
        let mut deferred_cloned_component_ids: Vec<ComponentId> = Vec::new();
        {
            let world = world.as_unsafe_world_cell();
            let source_entity = world
                .get_entity(source)
                .expect("Source entity must be valid and spawned.");
            let source_archetype = source_entity.archetype();

            // #[cfg(feature = "bevy_reflect")]
            // let app_registry = unsafe {
            //     world
            //         .get_resource::<crate::reflect::AppTypeRegistry>()
            //         .cloned()
            // };
            // #[cfg(not(feature = "bevy_reflect"))]
            let app_registry = Option::<()>::None;

            bundle_scratch = BundleScratchSpace::with_capacity(source_archetype.component_count());

            let target_archetype = LazyCell::new(|| {
                world
                    .get_entity(target)
                    .expect("Target entity must be valid and spawned.")
                    .archetype()
            });

            if state.move_components {
                moved_components.reserve(source_archetype.component_count());
                // Replace default handler with special handler which would track if component was moved instead of cloned.
                // This is later used to determine whether we need to run component's drop function when removing it from the source entity or not.
                state.default_clone_fn = |_, ctx| ctx.move_component();
            }

            filter.clone_components(source_archetype, target_archetype, |component| {
                let handler = match state.clone_behavior_overrides.get(&component).or_else(|| {
                    world
                        .components()
                        .get_info(component)
                        .map(ComponentInfo::clone_behavior)
                }) {
                    Some(behavior) => match behavior {
                        ComponentCloneBehavior::Default => state.default_clone_fn,
                        ComponentCloneBehavior::Ignore => return,
                        ComponentCloneBehavior::Custom(custom) => *custom,
                    },
                    None => state.default_clone_fn,
                };

                // SAFETY: This component exists because it is present on the archetype.
                let info = unsafe { world.components().get_info_unchecked(component) };

                // SAFETY:
                // - There are no other mutable references to source entity.
                // - `component` is from `source_entity`'s archetype
                let source_component_ptr =
                    unsafe { source_entity.get_by_id(component).debug_checked_unwrap() };

                let source_component = SourceComponent {
                    info,
                    ptr: source_component_ptr,
                };

                // SAFETY:
                // - `components` and `component` are from the same world
                // - `source_component_ptr` is valid and points to the same type as represented by `component`
                let mut ctx = unsafe {
                    ComponentCloneCtx::new(
                        component,
                        source,
                        target,
                        &bundle_scratch_allocator,
                        &mut bundle_scratch,
                        world.entity_allocator(),
                        info,
                        state,
                        mapper,
                        app_registry.as_ref(),
                    )
                };

                (handler)(&source_component, &mut ctx);

                if ctx.state.move_components {
                    if ctx.target_component_moved {
                        moved_components.push(component);
                    }
                    // Component wasn't written by the clone handler, so assume it's going to be
                    // cloned/processed using deferred_commands instead.
                    // This means that it's ComponentId won't be present in BundleScratch's component_ids,
                    // but it should still be removed when move_components is true.
                    else if !ctx.target_component_written() {
                        deferred_cloned_component_ids.push(component);
                    }
                }
            });
        }

        world.flush();

        for deferred in state.deferred_commands.drain(..) {
            (deferred)(world, mapper);
        }

        if !world.entities.contains(target) {
            panic!("Target entity does not exist");
        }

        if state.move_components {
            let mut source_entity = world.entity_mut(source);

            let cloned_components = if deferred_cloned_component_ids.is_empty() {
                &bundle_scratch.component_ids
            } else {
                // Remove all cloned components with drop by concatenating both vectors
                deferred_cloned_component_ids.extend(&bundle_scratch.component_ids);
                &deferred_cloned_component_ids
            };
            source_entity.remove_by_ids_with_caller(
                cloned_components,
                MaybeLocation::caller(),
                RelationshipHookMode::RunIfNotLinked,
                BundleRemover::empty_pre_remove,
            );

            let table_row = source_entity.location().table_row;

            source_entity.remove_by_ids_with_caller(
                &moved_components,
                MaybeLocation::caller(),
                RelationshipHookMode::RunIfNotLinked,
                |sparse_sets, mut table, components, bundle| {
                    for &component_id in bundle {
                        let Some(component_ptr) = sparse_sets
                            .get(component_id)
                            .and_then(|component| component.get(source))
                            .or_else(|| {
                                table.as_mut().and_then(|table| unsafe {
                                    table.get_component(component_id, table_row)
                                })
                            })
                        else {
                            // Component was removed by some other component's clone side effect before we got to it.
                            continue;
                        };

                        // SAFETY: component_id is valid because remove_by_ids_with_caller checked it before calling this closure
                        let info = unsafe { components.get_info_unchecked(component_id) };
                        let layout = info.layout();
                        let target_ptr = bundle_scratch_allocator.alloc_layout(layout);
                        // SAFETY:
                        // - component_ptr points to data with component layout
                        // - target_ptr was just allocated with component layout
                        // - component_ptr and target_ptr don't overlap
                        // - component_ptr matches component_id
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                component_ptr.as_ptr(),
                                target_ptr.as_ptr(),
                                layout.size(),
                            );
                            bundle_scratch.push_ptr(component_id, PtrMut::new(target_ptr));
                        }
                    }

                    (/* should drop? */ false, ())
                },
            );
        }

        // SAFETY:
        // - All `component_ids` are from the same world as `target` entity
        // - All `component_data_ptrs` are valid types represented by `component_ids`
        unsafe { bundle_scratch.write(world, target, relationship_hook_insert_mode) };
        target
    }
}

/// Part of the [`EntityCloner`], see there for more information.
struct EntityClonerState {
    clone_behavior_overrides: FixedHashMap<ComponentId, ComponentCloneBehavior>,
    move_components: bool,
    linked_cloning: bool,
    default_clone_fn: ComponentCloneFn,
    clone_queue: VecDeque<Entity>,
    deferred_commands: VecDeque<Box<dyn FnOnce(&mut World, &mut dyn EntityMapper)>>,
}

impl Default for EntityClonerState {
    fn default() -> Self {
        Self {
            move_components: false,
            linked_cloning: false,
            default_clone_fn: ComponentCloneBehavior::global_default_fn(),
            clone_behavior_overrides: Default::default(),
            clone_queue: Default::default(),
            deferred_commands: Default::default(),
        }
    }
}

/// A builder for configuring [`EntityCloner`]. See [`EntityCloner`] for more information.
pub struct EntityClonerBuilder<'w, Filter> {
    world: &'w mut World,
    filter: Filter,
    state: EntityClonerState,
}

/// Filters that can selectively clone components depending on its inner configuration are unified with this trait.
#[doc(hidden)]
pub trait CloneByFilter: Into<EntityClonerFilter> {
    /// The filter will call `clone_component` for every [`ComponentId`] that passes it.
    fn clone_components<'a>(
        &mut self,
        source_archetype: &Archetype,
        target_archetype: LazyCell<&'a Archetype, impl FnOnce() -> &'a Archetype>,
        clone_component: impl FnMut(ComponentId),
    );
}

/// Part of the [`EntityCloner`], see there for more information.
#[doc(hidden)]
#[derive(From)]
pub enum EntityClonerFilter {
    OptOut(OptOut),
    OptIn(OptIn),
}

impl Default for EntityClonerFilter {
    fn default() -> Self {
        Self::OptOut(Default::default())
    }
}

impl CloneByFilter for EntityClonerFilter {
    fn clone_components<'a>(
        &mut self,
        source_archetype: &Archetype,
        target_archetype: LazyCell<&'a Archetype, impl FnOnce() -> &'a Archetype>,
        clone_component: impl FnMut(ComponentId),
    ) {
        match self {
            EntityClonerFilter::OptOut(filter) => {
                filter.clone_components(source_archetype, target_archetype, clone_component);
            }
            EntityClonerFilter::OptIn(filter) => {
                filter.clone_components(source_archetype, target_archetype, clone_component);
            }
        }
    }
}

/// Generic for [`EntityClonerBuilder`] that makes the cloner try to clone every component from the source entity
/// except for components that were explicitly denied, for example by using the
/// [`deny`](EntityClonerBuilder::deny) method.
///
/// Required components are not considered by denied components and must be explicitly denied as well if desired.
pub struct OptOut {
    /// Contains the components that should not be cloned.
    deny: FixedHashSet<ComponentId>,

    /// Determines if a component is inserted when it is existing already.
    insert_mode: InsertMode,

    /// Is `true` unless during [`EntityClonerBuilder::without_required_by_components`] which will suppress
    /// components that require denied components to be denied as well, causing them to be created independent
    /// from the value at the source entity if needed.
    attach_required_by_components: bool,
}

impl Default for OptOut {
    fn default() -> Self {
        Self {
            deny: Default::default(),
            insert_mode: InsertMode::Replace,
            attach_required_by_components: true,
        }
    }
}

/// Generic for [`EntityClonerBuilder`] that makes the cloner try to clone every component that was explicitly
/// allowed from the source entity, for example by using the [`allow`](EntityClonerBuilder::allow) method.
///
/// Required components are also cloned when the target entity does not contain them.
pub struct OptIn {
    /// Contains the components explicitly allowed to be cloned.
    allow: FixedHashMap<ComponentId, Explicit>,

    /// Lists of required components, [`Explicit`] refers to a range in it.
    required_of_allow: Vec<ComponentId>,

    /// Contains the components required by those in [`Self::allow`].
    /// Also contains the number of components in [`Self::allow`] each is required by to track
    /// when to skip cloning a required component after skipping explicit components that require it.
    required: FixedHashMap<ComponentId, Required>,

    /// Is `true` unless during [`EntityClonerBuilder::without_required_components`] which will suppress
    /// evaluating required components to clone, causing them to be created independent from the value at
    /// the source entity if needed.
    attach_required_components: bool,
}

impl Default for OptIn {
    fn default() -> Self {
        Self {
            allow: Default::default(),
            required_of_allow: Default::default(),
            required: Default::default(),
            attach_required_components: true,
        }
    }
}

/// Contains the components explicitly allowed to be cloned.
struct Explicit {
    /// If component was added via [`allow`](EntityClonerBuilder::allow) etc, this is `Overwrite`.
    ///
    /// If component was added via [`allow_if_new`](EntityClonerBuilder::allow_if_new) etc, this is `Keep`.
    insert_mode: InsertMode,

    /// Contains the range in [`OptIn::required_of_allow`] for this component containing its
    /// required components.
    ///
    /// Is `None` if [`OptIn::attach_required_components`] was `false` when added.
    /// It may be set to `Some` later if the component is later added explicitly again with
    /// [`OptIn::attach_required_components`] being `true`.
    ///
    /// Range is empty if this component has no required components that are not also explicitly allowed.
    required_range: Option<Range<usize>>,
}

struct Required {
    /// Amount of explicit components this component is required by.
    required_by: u32,

    /// As [`Self::required_by`] but is reduced during cloning when an explicit component is not cloned,
    /// either because [`Explicit::insert_mode`] is `Keep` or the source entity does not contain it.
    ///
    /// If this is zero, the required component is not cloned.
    ///
    /// The counter is reset to `required_by` when the cloning is over in case another entity needs to be
    /// cloned by the same [`EntityCloner`].
    required_by_reduced: u32,
}
