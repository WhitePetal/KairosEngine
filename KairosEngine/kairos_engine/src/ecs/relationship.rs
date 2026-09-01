//! This module provides functionality to link entities to each other using specialized components called "relationships". See the [`Relationship`] trait for more info.

mod related_methods;
mod relationship_source_collection;

pub use related_methods::*;
pub use relationship_source_collection::*;

use std::{marker::PhantomData, sync::Arc};

use log::warn;

use crate::{
    debug::DebugName, ecs::{
        component::{Component, ComponentId, Components, Mutable}, entity::Entity, lifecycle::HookContext, system::EntityCommand, world::{DeferredWorld, EntityWorldMut},
    }, ptr::Ptr,
};

/// A [`Component`] on a "source" [`Entity`] that references another target [`Entity`], creating a "relationship" between them. Every [`Relationship`]
/// has a corresponding [`RelationshipTarget`] type (and vice-versa), which exists on the "target" entity of a relationship and contains the list of all
/// "source" entities that relate to the given "target".
///
/// A [`Relationship`] may only be one-to-many (or one-to-one): an [`Entity`] may point to at most one [`Entity`] through the [`Relationship`] component.
///
/// The [`Relationship`] component is the "source of truth" and the [`RelationshipTarget`] component reflects that source of truth. When a [`Relationship`]
/// component is inserted on an [`Entity`], the corresponding [`RelationshipTarget`] component is immediately inserted on the target component if it does
/// not already exist, and the "source" entity is automatically added to the [`RelationshipTarget`] collection (this is done via "component hooks").
///
/// A common example of a [`Relationship`] is the parent / child relationship. Bevy ECS includes a canonical form of this via the [`ChildOf`](crate::hierarchy::ChildOf)
/// [`Relationship`] and the [`Children`](crate::hierarchy::Children) [`RelationshipTarget`].
///
/// [`Relationship`] and [`RelationshipTarget`] should always be derived via the [`Component`] trait to ensure the hooks are set up properly.
///
/// ## Derive
///
/// [`Relationship`] and [`RelationshipTarget`] can only be derived for structs with a single unnamed field, single named field
/// or for named structs where one field is annotated with `#[relationship]`.
/// If there are additional fields, they must all implement [`Default`].
///
/// [`RelationshipTarget`] also requires that the relationship field is private to prevent direct mutation,
/// ensuring the correctness of relationships.
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::entity::Entity;
/// #[derive(Component)]
/// #[relationship(relationship_target = Children)]
/// pub struct ChildOf {
///     #[relationship]
///     pub parent: Entity,
///     internal: u8,
/// };
///
/// #[derive(Component)]
/// #[relationship_target(relationship = ChildOf)]
/// pub struct Children(Vec<Entity>);
/// ```
///
/// A one-to-one relationship can be created by putting a single [`Entity`] in the [`RelationshipTarget`]'s field.
/// In that case, if another entity is added to the relationship, the original entity is removed.
///
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::entity::Entity;
/// #[derive(Component)]
/// #[relationship(relationship_target = View)]
/// pub struct ViewOf(pub Entity);
///
/// #[derive(Component)]
/// #[relationship_target(relationship = ViewOf)]
/// pub struct View(Entity);
/// ```
///
/// When deriving [`RelationshipTarget`] you can specify the `#[relationship_target(linked_spawn)]` attribute to
/// automatically despawn entities stored in an entity's [`RelationshipTarget`] when that entity is despawned:
///
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::entity::Entity;
/// #[derive(Component)]
/// #[relationship(relationship_target = Children)]
/// pub struct ChildOf(pub Entity);
///
/// #[derive(Component)]
/// #[relationship_target(relationship = ChildOf, linked_spawn)]
/// pub struct Children(Vec<Entity>);
/// ```
///
/// By default, relationships cannot point to their own entity. If you want to allow self-referential
/// relationships, you can use the `allow_self_referential` attribute:
///
/// ```
/// # use bevy_ecs::component::Component;
/// # use bevy_ecs::entity::Entity;
/// #[derive(Component)]
/// #[relationship(relationship_target = PeopleILike, allow_self_referential)]
/// pub struct LikedBy(pub Entity);
///
/// #[derive(Component)]
/// #[relationship_target(relationship = LikedBy)]
/// pub struct PeopleILike(Vec<Entity>);
/// ```
pub trait Relationship: Component + Sized {
    /// The [`Component`] added to the "target" entities of this [`Relationship`], which contains the list of all "source"
    /// entities that relate to the "target".
    type RelationshipTarget: RelationshipTarget<Relationship = Self>;

    /// If `true`, a relationship is allowed to point to its own entity.
    ///
    /// Set this to `true` when self-relationships are semantically valid for your use case,
    /// such as `Likes(self)`, `EmployedBy(self)`, or a `ColliderOf` relationship where
    /// a collider can be attached to its own entity.
    ///
    /// # Warning
    ///
    /// When `ALLOW_SELF` is `true`, be careful when using recursive traversal methods
    /// like `iter_ancestors` or `root_ancestor`, as they will loop infinitely if an entity
    /// points to itself.
    const ALLOW_SELF_REFERENITAL: bool = false;

    /// Gets the [`Entity`] ID of the related entity.
    fn get(&self) -> Entity;

    /// Creates this [`Relationship`] from the given `entity`.
    fn from(entity: Entity) -> Self;

    /// Changes the current [`Entity`] ID of the entity containing the [`RelationshipTarget`] to another one.
    ///
    /// This is useful for updating the relationship without overwriting other fields stored in `Self`.
    ///
    /// # Warning
    ///
    /// This should generally not be called by user code, as modifying the related entity could invalidate the
    /// relationship. If this method is used, then the hooks [`on_discard`](Relationship::on_discard) have to
    /// run before and [`on_insert`](Relationship::on_insert) after it.
    /// This happens automatically when this method is called with [`EntityWorldMut::modify_component`].
    ///
    /// Prefer to use regular means of insertions when possible.
    fn set_risky(&mut self, entity: Entity);

    /// The `on_insert` component hook that maintains the [`Relationship`] / [`RelationshipTarget`] connection.
    fn on_insert(
        mut world: DeferredWorld,
        HookContext {
            entity,
            caller,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {}
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            }
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().get();
        if !Self::ALLOW_SELF_REFERENITAL && target_entity == entity {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} points to itself. The invalid {} relationship has been removed. \nIf this is intended behavior self-referential reltaions can be enabled with the allow_self_referential attribute: #[relationship(allow_self_referential)]",
                caller
                    .map(|location| format!("{location}"))
                    .unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
            return;
        }
        // For one-to-one relationships, remove existing relationship before adding new one
        let current_source_to_remove = world
            .get_entity(target_entity)
            .ok()
            .and_then(|target_entity_ref| target_entity_ref.get::<Self::RelationshipTarget>())
            .and_then(|relationship_target| {
                relationship_target
                    .collection()
                    .source_to_remove_before_add()
            });

        if let Some(current_source) = current_source_to_remove {
            world.commands().entity(current_source).try_remove::<Self>();
        }

        if let Ok(mut entity_commands) = world.commands().get_entity(target_entity) {
            entity_commands
                .entry::<Self::RelationshipTarget>()
                .and_modify(move |mut relationship_target| {
                    relationship_target.collection_mut_risky().add(entity);
                })
                .or_insert_with(move || {
                    let mut target = Self::RelationshipTarget::with_capacity(1);
                    target.collection_mut_risky().add(entity);
                    target
                });
        } else {
            warn!(
                "{}The {}({target_entity:?}) relationship on entity {entity:?} relates to an entity that does not exist. The invalid {} relationship has been removed.",
                caller.map(|localtion| format!("{localtion}")).unwrap_or_default(),
                DebugName::type_name::<Self>(),
                DebugName::type_name::<Self>()
            );
            world.commands().entity(entity).remove::<Self>();
        }
    }

    /// The `on_discard` component hook that maintains the [`Relationship`] / [`RelationshipTarget`] connection.
    // note: think of this as "on_drop"
    fn on_discard(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {},
            RelationshipHookMode::Skip => return,
            RelationshipHookMode::RunIfNotLinked => {
                if <Self::RelationshipTarget as RelationshipTarget>::LINKED_SPAWN {
                    return;
                }
            },
        }
        let target_entity = world.entity(entity).get::<Self>().unwrap().get();
        if let Ok(mut target_entity_mut) = world.get_entity_mut(target_entity)
            && let Some(mut relationship_target) =
                target_entity_mut.get_mut::<Self::RelationshipTarget>()
        {
            relationship_target.collection_mut_risky().remove(entity);
            if relationship_target.len() == 0 {
                let command = |mut entity: EntityWorldMut| {
                    // this "remove" operation must check emptiness because in the event that an identical
                    // relationship is inserted on top, this despawn would result in the removal of that identical
                    // relationship ... not what we want!
                    if entity
                        .get::<Self::RelationshipTarget>()
                        .is_some_and(RelationshipTarget::is_empty)
                    {
                        entity.remove::<Self::RelationshipTarget>();
                    }
                };

                world
                    .commands()
                    .queue_silenced(command.with_entity(target_entity));
            }
        }
    }
}

/// The iterator type for the source entities in a [`RelationshipTarget`] collection,
/// as defined in the [`RelationshipSourceCollection`] trait.
pub type SourceIter<'w, R> =
    <<R as RelationshipTarget>::Collection as RelationshipSourceCollection>::SourceIter<'w>;

/// A [`Component`] containing the collection of entities that relate to this [`Entity`] via the associated `Relationship` type.
/// See the [`Relationship`] documentation for more information.
pub trait RelationshipTarget: Component<Mutability = Mutable> + Sized {
    /// If this is true, when despawning or cloning (when [linked cloning is enabled](crate::entity::EntityClonerBuilder::linked_cloning)), the related entities targeting this entity will also be despawned or cloned.
    ///
    /// For example, this is set to `true` for Bevy's built-in parent-child relation, defined by [`ChildOf`](crate::prelude::ChildOf) and [`Children`](crate::prelude::Children).
    /// This means that when a parent is despawned, any children targeting that parent are also despawned (and the same applies to cloning).
    ///
    /// To get around this behavior, you can first break the relationship between entities, and *then* despawn or clone.
    /// This defaults to false when derived.
    const LINKED_SPAWN: bool;
    /// The [`Relationship`] that populates this [`RelationshipTarget`] collection.
    type Relationship: Relationship<RelationshipTarget = Self>;
    /// The collection type that stores the "source" entities for this [`RelationshipTarget`] component.
    ///
    /// Check the list of types which implement [`RelationshipSourceCollection`] for the data structures that can be used inside of your component.
    /// If you need a new collection type, you can implement the [`RelationshipSourceCollection`] trait
    /// for a type you own which wraps the collection you want to use (to avoid the orphan rule),
    /// or open an issue on the Bevy repository to request first-party support for your collection type.
    type Collection: RelationshipSourceCollection;

    /// Returns a reference to the stored [`RelationshipTarget::Collection`].
    fn collection(&self) -> &Self::Collection;

    /// Returns a mutable reference to the stored [`RelationshipTarget::Collection`].
    ///
    /// # Warning
    /// This should generally not be called by user code, as modifying the internal collection could invalidate the relationship.
    /// The collection should not contain duplicates.
    fn collection_mut_risky(&mut self) -> &mut Self::Collection;

    /// Creates a new [`RelationshipTarget`] from the given [`RelationshipTarget::Collection`].
    ///
    /// # Warning
    /// This should generally not be called by user code, as constructing the internal collection could invalidate the relationship.
    /// The collection should not contain duplicates.
    fn from_collection_risky(collection: Self::Collection) -> Self;

    /// The `on_discard` component hook that maintains the [`Relationship`] / [`RelationshipTarget`] connection.
    // note: think of this as "on_drop"
    fn on_discard(
        mut world: DeferredWorld,
        HookContext {
            entity,
            relationship_hook_mode,
            ..
        }: HookContext,
    ) {
        match relationship_hook_mode {
            RelationshipHookMode::Run => {},
            // For RelationshipTarget we don't want to run this hook even if it isn't linked, but for Relationship we do.
            RelationshipHookMode::Skip | RelationshipHookMode::RunIfNotLinked  => return,
        }
        todo!()
    }

    /// Creates this [`RelationshipTarget`] with the given pre-allocated entity capacity.
    fn with_capacity(capacity: usize) -> Self {
        let collection =
            <Self::Collection as RelationshipSourceCollection>::with_capacity(capacity);
        Self::from_collection_risky(collection)
    }

    /// Iterates the entities stored in this collection.
    #[inline]
    fn iter(&self) -> SourceIter<'_, Self> {
        self.collection().iter()
    }

    /// Returns the number of entities in this collection.
    #[inline]
    fn len(&self) -> usize {
        self.collection().len()
    }

    /// Returns true if this entity collection is empty.
    #[inline]
    fn is_empty(&self) -> bool {
        self.collection().is_empty()
    }
}

/// Configures the conditions under which the Relationship insert/discard hooks will be run.
#[derive(Copy, Clone, Debug)]
pub enum RelationshipHookMode {
    /// Relationship insert/discard hooks will always run
    Run,
    /// Relationship insert/discard hooks will run if [`RelationshipTarget::LINKED_SPAWN`] is false
    RunIfNotLinked,
    /// Relationship insert/discard hooks will always be skipped
    Skip,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum MaybeRelationshipAccessor {
    /// Not a relationship
    #[default]
    NoAccessor,
    /// Uninitialized relationship, which will be initialized when the second component of the relationship is registered.
    /// Boxed to reduce size overhead.
    Initializer(Box<RelationshipAccessorInitializer>),
    /// Relationship
    Accessor(RelationshipAccessor),
}

impl MaybeRelationshipAccessor {
    /// Initializes the relationship accessor if it isn't initialized already and the counterpart is registered.
    pub fn initialize(&mut self, id: ComponentId, components: &mut Components) {
        todo!()
    }

    /// Returns [`RelationshipAccessor`] if this component is a part of relationship and the accessor is initialized.
    pub fn accessor(&self) -> Option<&RelationshipAccessor> {
        match self {
            MaybeRelationshipAccessor::Accessor(relationship_accessor) => {
                Some(relationship_accessor)
            }
            _ => None,
        }
    }
}

impl From<Option<RelationshipAccessorInitializer>> for MaybeRelationshipAccessor {
    fn from(value: Option<RelationshipAccessorInitializer>) -> Self {
        value
            .map(|v| MaybeRelationshipAccessor::Initializer(Box::new(v)))
            .unwrap_or(MaybeRelationshipAccessor::NoAccessor)
    }
}

/// Initializer enum for [`RelationshipAccessor`] that allows to configure relationship for dynamic components.
#[derive(Clone)]
pub enum RelationshipAccessorInitializer {
    /// Describes a [`Relationship`] component.
    Relationship {
        /// Offset of the field containing [`Entity`] from the base of the component.
        ///
        /// Dynamic equivalent of [`Relationship::get`].
        entity_field_offset: usize,
        /// Value of [`RelationshipTarget::LINKED_SPAWN`] for the [`Relationship::RelationshipTarget`] of this [`Relationship`].
        linked_spawn: bool,
        /// Value of [`Relationship::ALLOW_SELF_REFERENTIAL`] of this [`Relationship`].
        allow_self_referential: bool,
        /// Getter for [`ComponentId`] of the [`RelationshipTarget`] counterpart.
        /// Should return `None` if [`RelationshipTarget`] isn't registered yet.
        relationship_target_getter: Arc<dyn Fn(&Components) -> Option<ComponentId>>,
    },
    /// Describes a [`RelationshipTarget`] component.
    RelationshipTarget {
        /// Function that returns an iterator over all [`Entity`]s of this [`RelationshipTarget`]'s collection.
        ///
        /// Dynamic equivalent of [`RelationshipTarget::iter`].
        /// # Safety
        /// Passed pointer must point to the value of the same component as the one that this accessor was registered to.
        iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        /// Value of [`RelationshipTarget::LINKED_SPAWN`] of this [`RelationshipTarget`].
        linked_spawn: bool,
        /// Value of [`Relationship::ALLOW_SELF_REFERENTIAL`] for the [`Relationship`] of this [`RelationshipTarget`].
        allow_self_referential: bool,
        /// Getter for [`ComponentId`] of the [`Relationship`] counterpart.
        /// Should return `None` if [`Relationship`] isn't registered yet.
        relationship_getter: Arc<dyn Fn(&Components) -> Option<ComponentId>>,
    },
}

impl std::fmt::Debug for RelationshipAccessorInitializer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelationshipAccessorInitializer::Relationship {
                entity_field_offset,
                linked_spawn,
                allow_self_referential,
                relationship_target_getter,
            } => f
                .debug_struct("Relationship")
                .field("entity_field_offset", entity_field_offset)
                .field("linked_spawn", linked_spawn)
                .field("allow_self_referential", allow_self_referential)
                .finish(),
            RelationshipAccessorInitializer::RelationshipTarget {
                iter,
                linked_spawn,
                allow_self_referential,
                relationship_getter,
            } => f
                .debug_struct("RelationshipTarget")
                .field("iter", iter)
                .field("linked_spawn", linked_spawn)
                .field("allow_self_referential", allow_self_referential)
                .finish(),
        }
    }
}

/// This enum describes a way to access the entities of [`Relationship`] and [`RelationshipTarget`] components
/// in a type-erased context.
#[derive(Debug, Clone, Copy)]
pub enum RelationshipAccessor {
    /// This component is a [`Relationship`].
    Relationship {
        /// Offset of the field containing [`Entity`] from the base of the component.
        ///
        /// Dynamic equivalent of [`Relationship::get`].
        entity_field_offset: usize,
        /// Value of [`RelationshipTarget::LINKED_SPAWN`] for the [`Relationship::RelationshipTarget`] of this [`Relationship`].
        linked_spawn: bool,
        /// Value of [`Relationship::ALLOW_SELF_REFERENTIAL`] of this [`Relationship`].
        allow_self_referential: bool,
        /// [`ComponentId`] of the [`RelationshipTarget`] counterpart.
        relationship_target: ComponentId,
    },
    /// This component is a [`RelationshipTarget`].
    RelationshipTarget {
        /// Function that returns an iterator over all [`Entity`]s of this [`RelationshipTarget`]'s collection.
        ///
        /// Dynamic equivalent of [`RelationshipTarget::iter`].
        /// # Safety
        /// Passed pointer must point to the value of the same component as the one that this accessor was registered to.
        iter: for<'a> unsafe fn(Ptr<'a>) -> Box<dyn Iterator<Item = Entity> + 'a>,
        /// Value of [`RelationshipTarget::LINKED_SPAWN`] of this [`RelationshipTarget`].
        linked_spawn: bool,
        /// Value of [`Relationship::ALLOW_SELF_REFERENTIAL`] for the [`Relationship`] of this [`RelationshipTarget`].
        allow_self_referential: bool,
        /// [`ComponentId`] of the [`Relationship`] counterpart.
        relationship: ComponentId,
    },
}

/// A type-safe convenience wrapper over [`RelationshipAccessor`].
pub struct ComponentRelationshipAccessor<C: ?Sized> {
    pub(crate) initializer: RelationshipAccessorInitializer,
    phantom: PhantomData<C>,
}

//TODO!
