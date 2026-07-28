use std::{any::TypeId, collections::hash_map::Entry, fmt::Debug};

use crate::{ecs::{component::Component, entity::Entity}, types::TypeIdMap};


#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a `Resource`",
    label = "invalid `Resource`",
    note = "consider annotating `{Self}` with `#[derive(Resource)]`"
)]
pub trait Resource: Component + Debug {}


#[derive(Default, Debug)]
pub struct ResourceEntities(TypeIdMap<Entity>);

impl ResourceEntities {
    /// Returns an iterator over all registered resource components and their corresponding entity.
    ///
    /// This must scan the entire array of components to find non-empty values,
    /// which may be slow even if there are few resources.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (TypeId, Entity)> {
        self.deref().iter().map(|(id, entity)| (*id, *entity))
    }

    /// Returns the entity for the given resource component, or `None` if there is no entity.
    #[inline]
    pub fn get(&self, id: TypeId) -> Option<Entity> {
        self.deref().get(&id).copied()
    }

    #[inline]
    pub fn entry(&mut self, id: TypeId) -> Entry<'_, TypeId, Entity> {
        self.deref_mut().entry(id)
    }

    #[inline]
    pub fn insert(&mut self, id: TypeId, entity: Entity) -> Option<Entity> {
        self.deref_mut().insert(id, entity)
    }

    #[inline]
    pub fn remove(&mut self, id: TypeId) -> Option<Entity> {
        self.deref_mut().remove(&id)
    }

    #[inline]
    pub fn contains(&self, id: TypeId) -> bool {
        self.deref().contains_key(&id)
    }

    #[inline]
    fn deref(&self) -> &TypeIdMap<Entity> {
        &self.0
    }

    #[inline]
    fn deref_mut(&mut self) -> &mut TypeIdMap<Entity> {
        &mut self.0
    }
}


/// A marker component for entities that have a Resource component.
#[derive(Debug)]
pub struct IsResource(TypeId);
impl Component for IsResource {}

impl IsResource {
    /// Creates a new instance with the given `component_id`
    pub fn new(component_id: TypeId) -> Self {
        Self(component_id)
    }

    /// The [`ComponentId`] of the resource component (the _actual_ resource value component, not the [`IsResource`] component).
    pub fn resource_component_id(&self) -> TypeId {
        self.0
    }

    // pub(crate) fn on_insert(mut world: DeferredWorld, context: HookContext) {
    //     let resource_component_id = world
    //         .entity(context.entity)
    //         .get::<Self>()
    //         .unwrap()
    //         .resource_component_id();

    //     if let Some(original_entity) = world.resource_entities.get(resource_component_id) {
    //         if !world.entities().contains(original_entity) {
    //             let name = world
    //                 .components()
    //                 .get_name(resource_component_id)
    //                 .expect("resource is registered");
    //             panic!(
    //                 "Resource entity {} of {} has been despawned, when it's not supposed to be.",
    //                 original_entity, name
    //             );
    //         }

    //         if original_entity != context.entity {
    //             // the resource already exists and the new one should be removed
    //             world
    //                 .commands()
    //                 .entity(context.entity)
    //                 .remove_by_id(resource_component_id);
    //             world
    //                 .commands()
    //                 .entity(context.entity)
    //                 .remove_by_id(context.component_id);
    //             let name = world
    //                 .components()
    //                 .get_name(resource_component_id)
    //                 .expect("resource is registered");
    //             warn!("Tried inserting the resource {} while one already exists. \
    //             Resources are unique components stored on a single entity. \
    //             Inserting on a different entity, when one already exists, causes the new value to be removed.", name);
    //         }
    //     } else {
    //         // SAFETY: We have exclusive world access (as long as we don't make structural changes).
    //         let cache = unsafe { world.as_unsafe_world_cell().resource_entities() };
    //         // SAFETY: There are no shared references to the map.
    //         // We only expose `&ResourceCache` to code with access to a resource (such as `&World`),
    //         // and that would conflict with the `DeferredWorld` passed to the resource hook.
    //         unsafe { &mut *cache.0.get() }.insert(resource_component_id, context.entity);
    //     }
    // }

    // pub(crate) fn on_discard(mut world: DeferredWorld, context: HookContext) {
    //     let resource_component_id = world
    //         .entity(context.entity)
    //         .get::<Self>()
    //         .unwrap()
    //         .resource_component_id();

    //     if let Some(resource_entity) = world.resource_entities.get(resource_component_id)
    //         && resource_entity == context.entity
    //     {
    //         // SAFETY: We have exclusive world access (as long as we don't make structural changes).
    //         let cache = unsafe { world.as_unsafe_world_cell().resource_entities() };
    //         // SAFETY: There are no shared references to the map.
    //         // We only expose `&ResourceCache` to code with access to a resource (such as `&World`),
    //         // and that would conflict with the `DeferredWorld` passed to the resource hook.
    //         unsafe { &mut *cache.0.get() }.remove(resource_component_id);

    //         world
    //             .commands()
    //             .entity(context.entity)
    //             .remove_by_id(resource_component_id);
    //     }
    // }

    // pub(crate) fn on_despawn(_world: DeferredWorld, _context: HookContext) {
    //     warn!("Resource entities are not supposed to be despawned.");
    // }
}
