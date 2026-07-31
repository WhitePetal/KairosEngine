use std::{marker::PhantomData, sync::Arc};

use crate::{
    ecs::{
        component::{ComponentId, Components},
        entity::Entity,
    },
    ptr::Ptr,
};

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
