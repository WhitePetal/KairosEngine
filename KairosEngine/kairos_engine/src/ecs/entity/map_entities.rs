use crate::ecs::entity::{Entity, EntityHashMap};

/// An implementor of this trait knows how to map an [`Entity`] into another [`Entity`].
///
/// Usually this is done by using an [`EntityHashMap<Entity>`] to map source entities
/// (mapper inputs) to the current world's entities (mapper outputs).
///
/// More generally, this can be used to map [`Entity`] references between any two [`Worlds`](World).
///
/// This is used by [`MapEntities`] implementers.
///
/// ## Example
///
/// ```
/// # use bevy_ecs::entity::{Entity, EntityMapper};
/// # use bevy_ecs::entity::EntityHashMap;
/// #
/// pub struct SimpleEntityMapper {
///   map: EntityHashMap<Entity>,
/// }
///
/// // Example implementation of EntityMapper where we map an entity to another entity if it exists
/// // in the underlying `EntityHashMap`, otherwise we just return the original entity.
/// impl EntityMapper for SimpleEntityMapper {
///     fn get_mapped(&mut self, entity: Entity) -> Entity {
///         self.map.get(&entity).copied().unwrap_or(entity)
///     }
///
///     fn set_mapped(&mut self, source: Entity, target: Entity) {
///         self.map.insert(source, target);
///     }
/// }
/// ```
pub trait EntityMapper {
    /// Returns the "target" entity that maps to the given `source`.
    fn get_mapped(&mut self, source: Entity) -> Entity;

    /// Maps the `target` entity to the given `source`. For some implementations this might not actually determine the result
    /// of [`EntityMapper::get_mapped`].
    fn set_mapped(&mut self, source: Entity, target: Entity);
}

impl EntityMapper for &mut dyn EntityMapper {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        (*self).get_mapped(source)
    }

    fn set_mapped(&mut self, source: Entity, target: Entity) {
        (*self).set_mapped(source, target);
    }
}

impl EntityMapper for EntityHashMap<Entity> {
    /// Returns the corresponding mapped entity or returns `entity` if there is no mapped entity
    fn get_mapped(&mut self, source: Entity) -> Entity {
        self.get(&source).cloned().unwrap_or(source)
    }

    fn set_mapped(&mut self, source: Entity, target: Entity) {
        self.insert(source, target);
    }
}

// TODO!
