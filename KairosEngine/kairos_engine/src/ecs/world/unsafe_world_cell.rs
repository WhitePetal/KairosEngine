//! Contains types that allow disjoint mutable access to a [`World`].

use std::{any::TypeId, cell::UnsafeCell, fmt::Debug, marker::PhantomData, ptr};

use crate::ecs::{change_detection::Tick, component::{Component, ComponentId, Components}, entity::{Entities, Entity, EntityAllocator, EntityLocation}, resource::ResourceEntities, storage::Storages, world::{DeferredWorld, World, WorldId}};

/// Variant of the [`World`] where resource and component accesses take `&self`, and the responsibility to avoid
/// aliasing violations are given to the caller instead of being checked at compile-time by rust's unique XOR shared rule.
///
/// ### Rationale
/// In rust, having a `&mut World` means that there are absolutely no other references to the safe world alive at the same time,
/// without exceptions. Not even unsafe code can change this.
///
/// But there are situations where careful shared mutable access through a type is possible and safe. For this, rust provides the [`UnsafeCell`]
/// escape hatch, which allows you to get a `*mut T` from a `&UnsafeCell<T>` and around which safe abstractions can be built.
///
/// Access to resources and components can be done uniquely using [`World::resource_mut`] and [`World::entity_mut`], and shared using [`World::resource`] and [`World::entity`].
/// These methods use lifetimes to check at compile time that no aliasing rules are being broken.
///
/// This alone is not enough to implement bevy systems where multiple systems can access *disjoint* parts of the world concurrently. For this, bevy stores all values of
/// resources and components (and [`ComponentTicks`]) in [`UnsafeCell`]s, and carefully validates disjoint access patterns using
/// APIs like [`System::initialize`](crate::system::System::initialize).
///
/// A system then can be executed using [`System::run_unsafe`](crate::system::System::run_unsafe) with a `&World` and use methods with interior mutability to access resource values.
///
/// ### Example Usage
///
/// [`UnsafeWorldCell`] can be used as a building block for writing APIs that safely allow disjoint access into the world.
/// In the following example, the world is split into a resource access half and a component access half, where each one can
/// safely hand out mutable references.
///
/// ```
/// use bevy_ecs::world::World;
/// use bevy_ecs::change_detection::Mut;
/// use bevy_ecs::resource::Resource;
/// use bevy_ecs::component::Mutable;
/// use bevy_ecs::world::unsafe_world_cell::UnsafeWorldCell;
///
/// // INVARIANT: existence of this struct means that users of it are the only ones being able to access resources in the world
/// struct OnlyResourceAccessWorld<'w>(UnsafeWorldCell<'w>);
/// // INVARIANT: existence of this struct means that users of it are the only ones being able to access components in the world
/// struct OnlyComponentAccessWorld<'w>(UnsafeWorldCell<'w>);
///
/// impl<'w> OnlyResourceAccessWorld<'w> {
///     fn get_resource_mut<T: Resource<Mutability = Mutable>>(&mut self) -> Option<Mut<'_, T>> {
///         // SAFETY: resource access is allowed through this UnsafeWorldCell
///         unsafe { self.0.get_resource_mut::<T>() }
///     }
/// }
/// // impl<'w> OnlyComponentAccessWorld<'w> {
/// //     ...
/// // }
///
/// // the two `UnsafeWorldCell`s borrow from the `&mut World`, so it cannot be accessed while they are live
/// fn split_world_access(world: &mut World) -> (OnlyResourceAccessWorld<'_>, OnlyComponentAccessWorld<'_>) {
///     let unsafe_world_cell = world.as_unsafe_world_cell();
///     let resource_access = OnlyResourceAccessWorld(unsafe_world_cell);
///     let component_access = OnlyComponentAccessWorld(unsafe_world_cell);
///     (resource_access, component_access)
/// }
/// ```
#[derive(Copy, Clone)]
pub struct UnsafeWorldCell<'w> {
    ptr: *mut World,
    #[cfg(debug_assertions)]
    allows_mutable_access: bool,
    _marker: PhantomData<(&'w World, &'w UnsafeCell<World>)>,
}

// SAFETY: `&World` and `&mut World` are both `Send`
unsafe impl Send for UnsafeWorldCell<'_> {}
// SAFETY: `&World` and `&mut World` are both `Sync`
unsafe impl Sync for UnsafeWorldCell<'_> {}

impl<'w> From<&'w mut World> for UnsafeWorldCell<'w> {
    fn from(value: &'w mut World) -> Self {
        value.as_unsafe_world_cell()
    }
}

impl<'w> From<&'w World> for UnsafeWorldCell<'w> {
    fn from(value: &'w World) -> Self {
        value.as_unsafe_world_cell_readonly()
    }
}

impl<'w> UnsafeWorldCell<'w> {
    /// Creates a [`UnsafeWorldCell`] that can be used to access everything immutably
    #[inline]
    pub(crate) fn new_readonly(world: &'w World) -> Self {
        Self {
            ptr: ptr::from_ref(world).cast_mut(),
            #[cfg(debug_assertions)]
            allows_mutable_access: false,
            _marker: PhantomData,
        }
    }

    /// Creates [`UnsafeWorldCell`] that can be used to access everything mutably
    #[inline]
    pub(crate) fn new_mutable(world: &'w mut World) -> Self {
        Self {
            ptr: ptr::from_mut(world),
            #[cfg(debug_assertions)]
            allows_mutable_access: true,
            _marker: PhantomData,
        }
    }

    #[cfg_attr(debug_assertions, inline(never), track_caller)]
    #[cfg_attr(not(debug_assertions), inline(always))]
    pub(crate) fn assert_allows_mutable_access(self) {
        // This annotation is needed because the
        // allows_mutable_access field doesn't exist otherwise.
        // Kinda weird, since debug_assert would never be called,
        // but CI complained in https://github.com/bevyengine/bevy/pull/17393
        #[cfg(debug_assertions)]
        debug_assert!(
            self.allows_mutable_access,
            "mutating world data via 'World::as_unsafe_world_cell_readonly' is forbidden"
        );
    }

    /// Gets a mutable reference to the [`World`] this [`UnsafeWorldCell`] belongs to.
    /// This is an incredibly error-prone operation and is only valid in a small number of circumstances.
    ///
    /// Calling this method implies mutable access to the *whole* world (see first point on safety section
    /// below), which includes all entities, components, and resources. Notably, calling this on
    /// [`WorldQuery::init_fetch`](crate::query::WorldQuery::init_fetch) and
    /// [`SystemParam::get_param`](crate::system::SystemParam::get_param) are most likely *unsound* unless
    /// you can prove that the underlying [`World`] is exclusive, which in normal circumstances is not.
    ///
    /// # Safety
    /// - `self` must have been obtained from a call to [`World::as_unsafe_world_cell`]
    ///   (*not* `as_unsafe_world_cell_readonly` or any other method of construction that
    ///   does not provide mutable access to the entire world).
    ///   - This means that if you have an `UnsafeWorldCell` that you didn't create yourself,
    ///     it is likely *unsound* to call this method.
    /// - The returned `&mut World` *must* be unique: it must never be allowed to exist
    ///   at the same time as any other borrows of the world or any accesses to its data.
    ///   This includes safe ways of accessing world data, such as [`UnsafeWorldCell::archetypes`].
    ///   - Note that the `&mut World` *may* exist at the same time as instances of `UnsafeWorldCell`,
    ///     so long as none of those instances are used to access world data in any way
    ///     while the mutable borrow is active.
    ///
    /// [//]: # (This test fails miri.)
    /// ```no_run
    /// # use bevy_ecs::prelude::*;
    /// # #[derive(Component)] struct Player;
    /// # fn store_but_dont_use<T>(_: T) {}
    /// # let mut world = World::new();
    /// // Make an UnsafeWorldCell.
    /// let world_cell = world.as_unsafe_world_cell();
    ///
    /// // SAFETY: `world_cell` was originally created from `&mut World`.
    /// // We must be sure not to access any world data while `world_mut` is active.
    /// let world_mut = unsafe { world_cell.world_mut() };
    ///
    /// // We can still use `world_cell` so long as we don't access the world with it.
    /// store_but_dont_use(world_cell);
    ///
    /// // !!This is unsound!! Even though this method is safe, we cannot call it until
    /// // `world_mut` is no longer active.
    /// let tick = world_cell.change_tick();
    ///
    /// // Use mutable access to spawn an entity.
    /// world_mut.spawn(Player);
    ///
    /// // Since we never use `world_mut` after this, the borrow is released
    /// // and we are once again allowed to access the world using `world_cell`.
    /// let archetypes = world_cell.archetypes();
    /// ```
    #[inline]
    pub unsafe fn world_mut(self) -> &'w mut World {
        self.assert_allows_mutable_access();
        // SAFETY:
        // - caller ensures the created `&mut World` is the only borrow of world
        unsafe { &mut *self.ptr }
    }

    /// Gets a reference to the [`&World`](World) this [`UnsafeWorldCell`] belongs to.
    /// This can be used for arbitrary shared/readonly access.
    ///
    /// # Safety
    /// - must have permission to access the whole world immutably
    /// - there must be no live exclusive borrows of world data
    /// - there must be no live exclusive borrow of world
    #[inline]
    pub unsafe fn world(self) -> &'w World {
        // SAFETY:
        // - caller ensures there is no `&mut World` this makes it okay to make a `&World`
        // - caller ensures there are no mutable borrows of world data, this means the caller cannot
        //   misuse the returned `&World`
        unsafe { self.unsafe_world() }
    }

    /// Gets a reference to the [`World`] this [`UnsafeWorldCell`] belong to.
    /// This can be used for arbitrary read only access of world metadata
    ///
    /// You should attempt to use various safe methods on [`UnsafeWorldCell`] for
    /// metadata access before using this method.
    ///
    /// # Safety
    /// - must only be used to access world metadata
    #[inline]
    pub unsafe fn world_metadata(self) -> &'w World {
        // SAFETY: caller ensures that returned reference is not used to violate aliasing rules
        unsafe { self.unsafe_world() }
    }

    /// Variant on [`UnsafeWorldCell::world`] solely used for implementing this type's methods.
    /// It allows having an `&World` even with live mutable borrows of components and resources
    /// so the returned `&World` should not be handed out to safe code and care should be taken
    /// when working with it.
    ///
    /// Deliberately private as the correct way to access data in a [`World`] that may have existing
    /// mutable borrows of data inside it, is to use [`UnsafeWorldCell`].
    ///
    /// # Safety
    /// - must not be used in a way that would conflict with any
    ///   live exclusive borrows of world data
    #[inline]
    unsafe fn unsafe_world(self) -> &'w World {
        // SAFETY:
        // - caller ensures that the returned `&World` is not used in a way that would conflict
        //   with any existing mutable borrows of world data
        unsafe { &*self.ptr }
    }

    /// Retrieves this world's unique [ID](WorldId).
    #[inline]
    pub fn id(self) -> WorldId {
        unsafe { self.world_metadata() }.id
    }

    /// Retrieves this world's [`Entities`] collection.
    #[inline]
    pub fn entities(self) -> &'w Entities {
        // SAFETY:
        // - we only access world metadata
        &unsafe {
            self.world_metadata()
        }.entities
    }

    /// Retrieves this world's [`Entities`] collection.
    #[inline]
    pub fn entity_allocator(self) -> &'w EntityAllocator {
        // SAFETY:
        // - we only access world metadata
        &unsafe {
            self.world_metadata()
        }.entity_allocator
    }

    /// Retrieves this world's [`Components`] collection.
    #[inline]
    pub fn components(self) -> &'w Components {
        // SAFETY:
        // - we only access world metadata
        &unsafe {
            self.world_metadata()
        }.components
    }

    /// Provides unchecked access to the internal data stores of the [`World`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that this is only used to access world data
    /// that this [`UnsafeWorldCell`] is allowed to.
    /// As always, any mutable access to a component must not exist at the same
    /// time as any other accesses to that same component.
    #[inline]
    pub unsafe fn storages(self) -> &'w Storages {
        &unsafe {
            self.unsafe_world()
        }.storages
    }
}

impl Debug for UnsafeWorldCell<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: World's Debug implementation only accesses metadata.
        Debug::fmt(unsafe {
            self.world_metadata()
        }, f)
    }
}

/// An interior-mutable reference to a particular [`Entity`] and all of its components
#[derive(Copy, Clone)]
pub struct UnsafeEntityCell<'w> {
    world: UnsafeWorldCell<'w>,
    entity: Entity,
    location: EntityLocation,
    last_run: Tick,
    this_run: Tick,
}

impl<'w> UnsafeEntityCell<'w> {
    pub(crate) fn new(
        world: UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
        last_run: Tick,
        this_run: Tick
    ) -> Self {
        UnsafeEntityCell {
            world,
            entity,
            location,
            last_run,
            this_run
        }
    }

    /// Returns the [ID](Entity) of the current entity.
    #[inline]
    #[must_use = "Omit the .id() call if you do not need to store the `Entity` identifier."]
    pub fn id(self) -> Entity {
        self.entity
    }

    /// Gets metadata indicating the location where the current entity is stored.
    #[inline]
    pub fn location(self) -> EntityLocation {
        self.location
    }

    /// Gets the world that the current entity belongs to.
    #[inline]
    pub fn world(self) -> UnsafeWorldCell<'w> {
        self.world
    }

    /// Returns `true` if the current entity has a component identified by `component_id`.
    /// Otherwise, this returns false.
    ///
    /// ## Notes
    ///
    /// - If you know the concrete type of the component, you should prefer [`Self::contains`].
    /// - If you know the component's [`TypeId`] but not its [`ComponentId`], consider using
    ///   [`Self::contains_type_id`].
    #[inline]
    pub fn contains_id(self, component_id: ComponentId) -> bool {
        todo!()
    }

    /// Returns `true` if the current entity has a component with the type identified by `type_id`.
    /// Otherwise, this returns false.
    ///
    /// ## Notes
    ///
    /// - If you know the concrete type of the component, you should prefer [`Self::contains`].
    /// - If you have a [`ComponentId`] instead of a [`TypeId`], consider using [`Self::contains_id`].
    #[inline]
    pub fn contains_type_id(self, type_id: TypeId) -> bool {
        let Some(id) = self.world.components().get_id(type_id) else {
            return false;
        };
        self.contains_id(id)
    }

    /// Returns `true` if the current entity has a component of type `T`.
    /// Otherwise, this returns `false`.
    ///
    /// ## Notes
    ///
    /// If you do not know the concrete type of a component, consider using
    /// [`Self::contains_id`] or [`Self::contains_type_id`].
    #[inline]
    pub fn contains<T: Component>(self) -> bool {
        self.contains_type_id(TypeId::of::<T>())
    }

    pub unsafe fn get<T: Component>(self) -> Option<&'w T> {
        let component_id = self.world.components().get_valid_id(TypeId::of::<T>())?;

        unsafe {
            todo!()
        }
    }
}

// TODO!
