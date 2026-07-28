//! This module contains all entity types and utilities for interacting with their ids.
//!
//! # What is an Entity?
//!
//! The ecs [docs](crate) give an overview of what entities are and generally how to use them.
//! These docs provide more detail into how they actually work.
//! In these docs [`Entity`] and "entity id" are synonymous and refer to the [`Entity`] type, which identifies an entity.
//! The term "entity" used on its own refers to the "thing"/"game object" that id references.
//!
//! # In this Module
//!
//! This module contains four main things:
//!
//!  - Core ECS types like [`Entity`], [`Entities`], and [`EntityAllocator`].
//!  - Utilities for [`Entity`] ids like [`MapEntities`], [`EntityHash`], and [`UniqueEntityVec`].
//!  - Helpers for entity tasks like [`EntityCloner`].
//!  - Entity-related error types like [`EntityNotSpawnedError`].
//!
//! # Entity Life Cycle
//!
//! Entities have life cycles.
//! They are created, used for a while, and eventually destroyed.
//! Let's start from the top:
//!
//! **Spawn:** An entity is created.
//! In bevy, this is called spawning.
//! Most commonly, this is done through [`World::spawn`](crate::world::World::spawn) or [`Commands::spawn`](crate::system::Commands::spawn).
//! This creates a fresh entity in the world and returns its [`Entity`] id, which can be used to interact with the entity it identifies.
//! These methods initialize the entity with a [`Bundle`], a group of [components](crate::component::Component) that it starts with.
//! It is also possible to use [`World::spawn_empty`](crate::world::World::spawn_empty) or [`Commands::spawn_empty`](crate::system::Commands::spawn_empty), which are similar but do not add any components to the entity.
//! In either case, the returned [`Entity`] id is used to further interact with the entity.
//!
//! **Update:** Once an entity is created, you will need its [`Entity`] id to perform further actions on it.
//! This can be done through [`World::entity_mut`](crate::world::World::entity_mut) and [`Commands::entity`](crate::system::Commands::entity).
//! Even if you don't store the id, you can still find the entity you spawned by searching for it in a [`Query`].
//! Queries are also the primary way of interacting with an entity's components.
//! You can use [`EntityWorldMut::remove`](crate::world::EntityWorldMut::remove) and [`EntityCommands::remove`](crate::system::EntityCommands::remove) to remove components,
//! and you can use [`EntityWorldMut::insert`](crate::world::EntityWorldMut::insert) and [`EntityCommands::insert`](crate::system::EntityCommands::insert) to insert more components.
//! Be aware that each entity can only have 0 or 1 values for each kind of component, so inserting a bundle may overwrite existing component values.
//! This can also be further configured based on the insert method.
//!
//! **Despawn:** Despawn an entity when it is no longer needed.
//! This destroys it and all its components.
//! The entity is no longer reachable through the [`World`], [`Commands`], or [`Query`]s.
//! Note that this means an [`Entity`] id may refer to an entity that has since been despawned!
//! Not all [`Entity`] ids refer to active entities.
//! If an [`Entity`] id is used when its entity has been despawned, an [`EntityNotSpawnedError`] is emitted.
//! Any [`System`](crate::system) could despawn any entity; even if you never share its id, it could still be despawned unexpectedly.
//! Your code should do its best to handle these errors gracefully.
//!
//! In short:
//!
//! - Entities are spawned through methods like [`World::spawn`](crate::world::World::spawn), which return an [`Entity`] id for the new entity.
//! - Once spawned, they can be accessed and modified through [`Query`]s and other apis.
//! - You can get the [`Entity`] id of an entity through [`Query`]s, so losing an [`Entity`] id is not a problem.
//! - Entities can have components inserted and removed via [`World::entity_mut`](crate::world::World::entity_mut) and [`Commands::entity`](crate::system::Commands::entity).
//! - Entities are eventually despawned, destroying the entity and causing its [`Entity`] id to no longer refer to an entity.
//! - Not all [`Entity`] ids point to actual entities, which makes many entity methods fallible.
//!
//! # [`Entity`] Allocation
//!
//! Entity spawning is actually done in two stages:
//! 1. Allocate: We generate a new valid / unique [`Entity`].
//! 2. Spawn: We make the entity "exist" in the [`World`]. It will show up in queries, it can have components, etc.
//!
//! The reason for this split is that we need to be able to _allocate_ entity ids concurrently,
//! whereas spawning requires unique (non-concurrent) access to the world.
//!
//! An [`Entity`] therefore goes through the following lifecycle:
//! 1. Unallocated (and "valid"): Only the allocator has any knowledge of this [`Entity`], but it _could_ be spawned, theoretically.
//! 2. Allocated (and "valid"): The allocator has handed out the [`Entity`], but it is not yet spawned.
//! 3. Spawned: The entity now "exists" in the [`World`]. It will show up in queries, it can have components, etc.
//! 4. Despawned: The entity no longer "exist" in the [`World`].
//! 5. Freed (and "invalid"): The [`Entity`] is returned to the allocator. The [`Entity::generation`] is bumped, which makes all existing [`Entity`] references with the previous generation "invalid".
//!
//! Note that by default, most spawn and despawn APIs handle the [`Entity`] allocation and freeing process for developers.
//!
//! [`World`]: crate::world::World
//! [`Query`]: crate::system::Query
//! [`Bundle`]: crate::bundle::Bundle
//! [`Component`]: crate::component::Component
//! [`Commands`]: crate::system::Commands
//!

use std::{fmt::Display, mem, num::NonZero};

use nonmax::NonMaxU32;

use crate::ecs::remote_allocator;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Display)]
#[repr(transparent)]
pub struct EntityIndex(NonMaxU32);

impl EntityIndex {
    const PLACEHOLDER: Self = Self(NonMaxU32::MAX);

    /// Constructs a new [`EntityIndex`] from its index.
    pub const fn new(index: NonMaxU32) -> Self {
        Self(index)
    }

    /// Equivalent to [`new`](Self::new) except that it takes a `u32` instead of a `NonMaxU32`.
    ///
    /// Returns `None` if the index is `u32::MAX`.
    pub const fn from_raw_u32(index: u32) -> Option<Self> {
        match NonMaxU32::new(index) {
            Some(index) => Some(Self(index)),
            None => None,
        }
    }

    /// Gets the index of the entity.
    #[inline(always)]
    pub const fn index(self) -> u32 {
        self.0.get()
    }

    /// Gets some bits that represent this value.
    /// The bits are opaque and should not be regarded as meaningful.
    const fn to_bits(self) -> u32 {
        // SAFETY: NonMax is repr transparent.
        unsafe { mem::transmute::<NonMaxU32, u32>(self.0) }
    }

    /// Reconstruct an [`EntityIndex`] previously destructured with [`EntityIndex::to_bits`].
    ///
    /// Only useful when applied to results from `to_bits` in the same instance of an application.
    ///
    /// # Panics
    ///
    /// This method will likely panic if given `u32` values that did not come from [`EntityIndex::to_bits`].
    const fn from_bits(bits: u32) -> Self {
        Self::try_from_bits(bits).expect("Attempted to initialize invalid bits as an entity index")
    }

    /// Reconstruct an [`EntityIndex`] previously destructured with [`EntityIndex::to_bits`].
    ///
    /// Only useful when applied to results from `to_bits` in the same instance of an application.
    ///
    /// This method is the fallible counterpart to [`EntityIndex::from_bits`].
    #[inline(always)]
    const fn try_from_bits(bits: u32) -> Option<Self> {
        match NonZero::<u32>::new(bits) {
            Some(underlying) => Some(Self(unsafe {
                // SAFETY: NonMax and NonZero are repr transparent.
                mem::transmute::<NonZero<u32>, NonMaxU32>(underlying)
            })),
            None => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Display)]
#[repr(transparent)]
pub struct EntityGeneration(u32);

impl EntityGeneration {
    /// Represents the first generation of an [`EntityIndex`].
    pub const FIRST: Self = Self(0);

    /// Non-wrapping difference between two generations after which a signed interpretation becomes negative.
    const DIFF_MAX: u32 = 1u32 << 31;

    /// Gets some bits that represent this value.
    /// The bits are opaque and should not be regarded as meaningful.
    #[inline(always)]
    pub const fn to_bits(self) -> u32 {
        self.0
    }

    /// Reconstruct an [`EntityGeneration`] previously destructured with [`EntityGeneration::to_bits`].
    ///
    /// Only useful when applied to results from `to_bits` in the same instance of an application.
    #[inline]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the [`EntityGeneration`] that would result from this many more `versions` of the corresponding [`EntityIndex`] from passing.
    #[inline]
    pub const fn after_version(self, version: u32) -> Self {
        Self(self.0.wrapping_add(version))
    }

    /// Identical to [`after_versions`](Self::after_versions) but also returns a `bool` indicating if,
    /// after these `versions`, one such version could conflict with a previous one.
    ///
    /// If this happens, this will no longer uniquely identify a version of an [`EntityIndex`].
    /// This is called entity aliasing.
    #[inline]
    pub const fn after_versions_and_could_alias(self, version: u32) -> (Self, bool) {
        let raw = self.0.overflowing_add(version);
        (Self(raw.0), raw.1)
    }

    /// Compares two generations.
    ///
    /// Generations that are later will be [`Greater`](core::cmp::Ordering::Greater) than earlier ones.
    ///
    /// ```
    /// # use bevy_ecs::entity::EntityGeneration;
    /// # use core::cmp::Ordering;
    /// let later_generation = EntityGeneration::FIRST.after_versions(400);
    /// assert_eq!(EntityGeneration::FIRST.cmp_approx(&later_generation), Ordering::Less);
    ///
    /// let (aliased, did_alias) = EntityGeneration::FIRST.after_versions(400).after_versions_and_could_alias(u32::MAX);
    /// assert!(did_alias);
    /// assert_eq!(EntityGeneration::FIRST.cmp_approx(&aliased), Ordering::Less);
    /// ```
    ///
    /// Ordering will be incorrect and [non-transitive](https://en.wikipedia.org/wiki/Transitive_relation)
    /// for distant generations:
    ///
    /// ```should_panic
    /// # use bevy_ecs::entity::EntityGeneration;
    /// # use core::cmp::Ordering;
    /// let later_generation = EntityGeneration::FIRST.after_versions(3u32 << 31);
    /// let much_later_generation = later_generation.after_versions(3u32 << 31);
    ///
    /// // while these orderings are correct and pass assertions...
    /// assert_eq!(EntityGeneration::FIRST.cmp_approx(&later_generation), Ordering::Less);
    /// assert_eq!(later_generation.cmp_approx(&much_later_generation), Ordering::Less);
    ///
    /// // ... this ordering is not and the assertion fails!
    /// assert_eq!(EntityGeneration::FIRST.cmp_approx(&much_later_generation), Ordering::Less);
    /// ```
    ///
    /// Because of this, `EntityGeneration` does not implement `Ord`/`PartialOrd`.
    #[inline]
    pub const fn cmp_approx(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match self.0.wrapping_sub(other.0) {
            0 => Ordering::Equal,
            1..Self::DIFF_MAX => Ordering::Greater,
            _ => Ordering::Less,
        }
    }
}

/// Unique identifier for an entity in a [`World`].
/// Note that this is just an id, not the entity itself.
/// Further, the entity this id refers to may no longer exist in the [`World`].
/// For more information about entities, their ids, and how to use them, see the module [docs](crate::entity).
///
/// # Aliasing
///
/// Once an entity is despawned, it ceases to exist.
/// However, its [`Entity`] id is still present, and may still be contained in some data.
/// This becomes problematic because it is possible for a later entity to be spawned at the exact same id!
/// If this happens, which is rare but very possible, it will be logged.
///
/// Aliasing can happen without warning.
/// Holding onto a [`Entity`] id corresponding to an entity well after that entity was despawned can cause un-intuitive behavior for both ordering, and comparing in general.
/// To prevent these bugs, it is generally best practice to stop holding an [`Entity`] or [`EntityGeneration`] value as soon as you know it has been despawned.
/// If you must do otherwise, do not assume the [`Entity`] id corresponds to the same entity it originally did.
/// See [`EntityGeneration`]'s docs for more information about aliasing and why it occurs.
///
/// # Stability warning
/// For all intents and purposes, `Entity` should be treated as an opaque identifier. The internal bit
/// representation is liable to change from release to release as are the behaviors or performance
/// characteristics of any of its trait implementations (i.e. `Ord`, `Hash`, etc.). This means that changes in
/// `Entity`'s representation, though made readable through various functions on the type, are not considered
/// breaking changes under [SemVer].
///
/// In particular, directly serializing with `Serialize` and `Deserialize` make zero guarantee of long
/// term wire format compatibility. Changes in behavior will cause serialized `Entity` values persisted
/// to long term storage (i.e. disk, databases, etc.) will fail to deserialize upon being updated.
///
/// # Usage
///
/// This data type is returned by iterating a `Query` that has `Entity` as part of its query fetch type parameter ([learn more]).
/// It can also be obtained by calling [`EntityCommands::id`] or [`EntityWorldMut::id`].
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # #[derive(Component)]
/// # struct SomeComponent;
/// fn setup(mut commands: Commands) {
///     // Calling `spawn` returns `EntityCommands`.
///     let entity = commands.spawn(SomeComponent).id();
/// }
///
/// fn exclusive_system(world: &mut World) {
///     // Calling `spawn` returns `EntityWorldMut`.
///     let entity = world.spawn(SomeComponent).id();
/// }
/// #
/// # bevy_ecs::system::assert_is_system(setup);
/// # bevy_ecs::system::assert_is_system(exclusive_system);
/// ```
///
/// It can be used to refer to a specific entity to apply [`EntityCommands`], or to call [`Query::get`] (or similar methods) to access its components.
///
/// ```
/// # use bevy_ecs::prelude::*;
/// #
/// # #[derive(Component)]
/// # struct Expired;
/// #
/// fn dispose_expired_food(mut commands: Commands, query: Query<Entity, With<Expired>>) {
///     for food_entity in &query {
///         commands.entity(food_entity).despawn();
///     }
/// }
/// #
/// # bevy_ecs::system::assert_is_system(dispose_expired_food);
/// ```
///
/// [learn more]: crate::system::Query#entity-id-access
/// [`EntityCommands::id`]: crate::system::EntityCommands::id
/// [`EntityWorldMut::id`]: crate::world::EntityWorldMut::id
/// [`EntityCommands`]: crate::system::EntityCommands
/// [`Query::get`]: crate::system::Query::get
/// [`World`]: crate::world::World
/// [SemVer]: https://semver.org/
#[derive(Clone, Copy)]
// Alignment repr necessary to allow LLVM to better output
// optimized codegen for `to_bits`, `PartialEq` and `Ord`.
#[repr(C, align(8))]
pub struct Entity {
    #[cfg(target_endian = "little")]
    index: EntityIndex,
    generation: EntityGeneration,
    #[cfg(target_endian = "big")]
    index: EntityIndex,
}

impl Entity {
    /// An entity ID with a placeholder value. This may or may not correspond to an actual entity,
    /// and should be overwritten by a new value before being used.
    ///
    /// ## Examples
    ///
    /// Initializing a collection (e.g. `array` or `Vec`) with a known size:
    ///
    /// ```no_run
    /// # use bevy_ecs::prelude::*;
    /// // Create a new array of size 10 filled with invalid entity ids.
    /// let mut entities: [Entity; 10] = [Entity::PLACEHOLDER; 10];
    ///
    /// // ... replace the entities with valid ones.
    /// ```
    ///
    /// Deriving [`Reflect`] for a component that has an `Entity` field:
    ///
    /// ```no_run
    /// # use bevy_ecs::{prelude::*, component::*};
    /// # use bevy_reflect::Reflect;
    /// #[derive(Reflect, Component)]
    /// #[reflect(Component)]
    /// pub struct MyStruct {
    ///     pub entity: Entity,
    /// }
    ///
    /// impl FromWorld for MyStruct {
    ///     fn from_world(_world: &mut World) -> Self {
    ///         Self {
    ///             entity: Entity::PLACEHOLDER,
    ///         }
    ///     }
    /// }
    /// ```
    pub const PLACEHOLDER: Self = Self::from_index(EntityIndex::PLACEHOLDER);

    /// Creates a new instance with the given index and generation.
    #[inline(always)]
    pub const fn from_index_and_generation(
        index: EntityIndex,
        generation: EntityGeneration,
    ) -> Entity {
        Self { index, generation }
    }

    /// Creates a new entity ID with the specified `index` and an unspecified generation.
    ///
    /// # Note
    ///
    /// Spawning a specific `entity` value is __rarely the right choice__. Most apps should favor
    /// [`Commands::spawn`](crate::system::Commands::spawn). This method should generally
    /// only be used for sharing entities across apps, and only when they have a scheme
    /// worked out to share an index space (which doesn't happen by default).
    ///
    /// In general, one should not try to synchronize the ECS by attempting to ensure that
    /// `Entity` lines up between instances, but instead insert a secondary identifier as
    /// a component.
    #[inline(always)]
    pub const fn from_index(index: EntityIndex) -> Entity {
        Self::from_index_and_generation(index, EntityGeneration::FIRST)
    }

    /// This is equivalent to [`from_index`](Self::from_index) except that it takes a `u32` instead of an [`EntityIndex`].
    ///
    /// Returns `None` if the index is `u32::MAX`.
    #[inline(always)]
    pub const fn from_raw_u32(index: u32) -> Option<Entity> {
        match NonMaxU32::new(index) {
            Some(index) => Some(Self::from_index(EntityIndex::new(index))),
            None => None,
        }
    }

    /// Convert to a form convenient for passing outside of rust.
    ///
    /// Only useful for identifying entities within the same instance of an application. Do not use
    /// for serialization between runs.
    ///
    /// No particular structure is guaranteed for the returned bits.
    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        self.index.to_bits() as u64 | ((self.generation.to_bits() as u64) << 32)
    }

    /// Reconstruct an `Entity` previously destructured with [`Entity::to_bits`].
    ///
    /// Only useful when applied to results from `to_bits` in the same instance of an application.
    ///
    /// # Panics
    ///
    /// This method will likely panic if given `u64` values that did not come from [`Entity::to_bits`].
    #[inline]
    pub const fn from_bits(bits: u64) -> Self {
        if let Some(id) = Self::try_from_bits(bits) {
            id
        } else {
            panic!("Attempted to initialize invalid bits as an entity")
        }
    }

    /// Reconstruct an `Entity` previously destructured with [`Entity::to_bits`].
    ///
    /// Only useful when applied to results from `to_bits` in the same instance of an application.
    ///
    /// This method is the fallible counterpart to [`Entity::from_bits`].
    #[inline(always)]
    pub const fn try_from_bits(bits: u64) -> Option<Self> {
        let raw_index = bits as u32;
        let raw_gen = (bits >> 32) as u32;

        if let Some(index) = EntityIndex::try_from_bits(raw_index) {
            Some(Self {
                index,
                generation: EntityGeneration::from_bits(raw_gen),
            })
        } else {
            None
        }
    }

    /// Return a transiently unique identifier.
    /// See also [`EntityIndex`].
    ///
    /// No two simultaneously-live entities share the same index, but dead entities' indices may collide
    /// with both live and dead entities. Useful for compactly representing entities within a
    /// specific snapshot of the world, such as when serializing.
    #[inline]
    pub const fn index(self) -> EntityIndex {
        self.index
    }

    /// Equivalent to `self.index().index()`. See [`Self::index`] for details.
    #[inline]
    pub const fn index_u32(self) -> u32 {
        self.index.index()
    }

    /// Returns the generation of this Entity's index. The generation is incremented each time an
    /// entity with a given index is despawned. This serves as a "count" of the number of times a
    /// given index has been reused (index, generation) pairs uniquely identify a given Entity.
    #[inline]
    pub const fn generation(self) -> EntityGeneration {
        self.generation
    }
}

impl Serialize for Entity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u64(self.to_bits())
    }
}

impl<'de> Deserialize<'de> for Entity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let id: u64 = Deserialize::deserialize(deserializer)?;
        Entity::try_from_bits(id)
            .ok_or_else(|| D::Error::custom("Attempting to deserialize an invalid entity."))
    }
}

/// Outputs the short entity identifier, including the index and generation.
///
/// This takes the format: `{index}v{generation}`.
///
/// For [`Entity::PLACEHOLDER`], this outputs `PLACEHOLDER`.
///
/// For a unique [`u64`] representation, use [`Entity::to_bits`].
impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Outputs the short entity identifier, including the index and generation.
///
/// This takes the format: `{index}v{generation}`.
///
/// For [`Entity::PLACEHOLDER`], this outputs `PLACEHOLDER`.
impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self == &Self::PLACEHOLDER {
            f.pad("PLACEHOLDER")
        } else {
            f.pad(&alloc::fmt::format(format_args!(
                "{}v{}",
                self.index(),
                self.generation()
            )))
        }
    }
}

/// Allocates [`Entity`] ids uniquely.
/// This is used in [`World::spawn_at`](crate::world::World::spawn_at) and [`World::despawn_no_free`](crate::world::World::despawn_no_free) to track entity ids no longer in use.
/// Allocating is fully concurrent and can be done from multiple threads.
///
/// Conceptually, this is a collection of [`Entity`] ids who's [`EntityIndex`] is despawned and who's [`EntityGeneration`] is the most recent.
/// See the module docs for how these ids and this allocator participate in the life cycle of an entity.
#[derive(Default, Debug)]
pub struct EntityAllocator {
    pub(crate) inner: remote_allocator::A,
}

impl EntityAllocator {}
