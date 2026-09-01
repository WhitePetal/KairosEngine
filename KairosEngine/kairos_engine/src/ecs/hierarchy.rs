use crate::ecs::{
    component::{Component, Immutable, Mutable, StorageType},
    entity::Entity,
    relationship::{Relationship, RelationshipTarget},
    world::FromWorld,
};

/// Stores the parent entity of this child entity with this component.
///
/// This is a [`Relationship`] component, and creates the canonical
/// "parent / child" hierarchy. This is the "source of truth" component, and it pairs with
/// the [`Children`] [`RelationshipTarget`](crate::relationship::RelationshipTarget).
///
/// This relationship should be used for things like:
///
/// 1. Organizing entities in a scene
/// 2. Propagating configuration or data inherited from a parent, such as "visibility" or "world-space global transforms".
/// 3. Ensuring a hierarchy is despawned when an entity is despawned.
///
/// [`ChildOf`] contains a single "target" [`Entity`]. When [`ChildOf`] is inserted on a "source" entity,
/// the "target" entity will automatically (and immediately, via a component hook) have a [`Children`]
/// component inserted, and the "source" entity will be added to that [`Children`] instance.
///
/// If the [`ChildOf`] component is replaced with a different "target" entity, the old target's [`Children`]
/// will be automatically (and immediately, via a component hook) be updated to reflect that change.
///
/// Likewise, when the [`ChildOf`] component is removed, the "source" entity will be removed from the old
/// target's [`Children`]. If this results in [`Children`] being empty, [`Children`] will be automatically removed.
///
/// When a parent is despawned, all children (and their descendants) will _also_ be despawned.
///
/// You can create parent-child relationships in a variety of ways. The most direct way is to insert a [`ChildOf`] component:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::new();
/// let root = world.spawn_empty().id();
/// let child1 = world.spawn(ChildOf(root)).id();
/// let child2 = world.spawn(ChildOf(root)).id();
/// let grandchild = world.spawn(ChildOf(child1)).id();
///
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1, child2]);
/// assert_eq!(&**world.entity(child1).get::<Children>().unwrap(), &[grandchild]);
///
/// world.entity_mut(child2).remove::<ChildOf>();
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1]);
///
/// world.entity_mut(root).despawn();
/// assert!(world.get_entity(root).is_err());
/// assert!(world.get_entity(child1).is_err());
/// assert!(world.get_entity(grandchild).is_err());
/// ```
///
/// However if you are spawning many children, you might want to use the [`EntityWorldMut::with_children`] helper instead:
///
/// ```
/// # use bevy_ecs::prelude::*;
/// # let mut world = World::new();
/// let mut child1 = Entity::PLACEHOLDER;
/// let mut child2 = Entity::PLACEHOLDER;
/// let mut grandchild = Entity::PLACEHOLDER;
/// let root = world.spawn_empty().with_children(|p| {
///     child1 = p.spawn_empty().with_children(|p| {
///         grandchild = p.spawn_empty().id();
///     }).id();
///     child2 = p.spawn_empty().id();
/// }).id();
///
/// assert_eq!(&**world.entity(root).get::<Children>().unwrap(), &[child1, child2]);
/// assert_eq!(&**world.entity(child1).get::<Children>().unwrap(), &[grandchild]);
/// ```
///
/// [`Relationship`]: crate::relationship::Relationship
// #[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug)]
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "kairos_reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(
    feature = "kairos_reflect",
    reflect(Component, PartialEq, Debug, FromWorld, Clone)
)]
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(all(feature = "kairos_reflect"), reflect(Serialize, Deserialize))]
// #[relationship(relationship_target = Children)]
#[doc(alias = "IsChild", alias = "Parent")]
pub struct ChildOf(pub Entity);

// TODO!: use derive
impl Component for ChildOf {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    type Mutability = Immutable;
}

// TODO!: use derive
impl Relationship for ChildOf {
    type RelationshipTarget = Children;

    fn get(&self) -> Entity {
        todo!()
    }

    fn from(entity: Entity) -> Self {
        todo!()
    }

    fn set_risky(&mut self, entity: Entity) {
        todo!()
    }
}

impl ChildOf {
    /// The parent entity of this child entity.
    #[inline]
    pub fn parent(&self) -> Entity {
        self.0
    }
}

// TODO: We need to impl either FromWorld or Default so ChildOf can be registered as Reflect.
// This is because Reflect deserialize by creating an instance and apply a patch on top.
// However ChildOf should only ever be set with a real user-defined entity.  Its worth looking into
// better ways to handle cases like this.
impl FromWorld for ChildOf {
    fn from_world(world: &mut super::world::World) -> Self {
        ChildOf(Entity::PLACEHOLDER)
    }
}

/// Tracks which entities are children of this parent entity.
///
/// A [`RelationshipTarget`] collection component that is populated
/// with entities that "target" this entity with the [`ChildOf`] [`Relationship`] component.
///
/// Together, these components form the "canonical parent-child hierarchy". See the [`ChildOf`] component for the full
/// description of this relationship and instructions on how to use it.
///
/// # Usage
///
/// Like all [`RelationshipTarget`] components, this data should not be directly manipulated to avoid desynchronization.
/// Instead, modify the [`ChildOf`] components on the "source" entities.
///
/// To access the children of an entity, you can iterate over the [`Children`] component,
/// using the [`IntoIterator`] trait.
/// For more complex access patterns, see the [`RelationshipTarget`] trait.
///
/// [`Relationship`]: crate::relationship::Relationship
/// [`RelationshipTarget`]: crate::relationship::RelationshipTarget
// #[derive(Component, Default, Debug, PartialEq, Eq)]
#[derive(Default, Debug, PartialEq, Eq)]
// #[relationship_target(relationship = ChildOf, linked_spawn)]
#[cfg_attr(feature = "kairos_reflect", derive(kairos_reflect::Reflect))]
#[cfg_attr(feature = "kairos_reflect", reflect(Component, FromWorld, Default))]
#[doc(alias = "IsParent")]
pub struct Children(Vec<Entity>);

// TODO!: use derive
impl Component for Children {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    type Mutability = Mutable;
}

impl RelationshipTarget for Children {
    const LINKED_SPAWN: bool = true;

    type Relationship = ChildOf;

    type Collection = Vec<Entity>;

    fn collection(&self) -> &Self::Collection {
        todo!()
    }

    fn collection_mut_risky(&mut self) -> &mut Self::Collection {
        todo!()
    }

    fn from_collection_risky(collection: Self::Collection) -> Self {
        todo!()
    }
}

// TODO!
