//! System parameter for computing up-to-date [`GlobalTransform`]s.

use kairos_ecs::{entity::{Entity, EntityNotSpawnedError}, hierarchy::ChildOf, query::QueryEntityError, system::{Query, SystemParam}};
use thiserror::Error;

use crate::{GlobalTransform, LocalTransform, helper::ComputeGlobalTransformError::{MalformedHierarchy, MissingTransform, NoSuchEntity}};

/// System parameter for computing up-to-date [`GlobalTransform`]s.
///
/// Computing an entity's [`GlobalTransform`] can be expensive so it is recommended
/// you use the [`GlobalTransform`] component stored on the entity, unless you need
/// a [`GlobalTransform`] that reflects the changes made to any [`Transform`]s since
/// the last time the transform propagation systems ran.
#[derive(SystemParam)]
pub struct TransformHelper<'w, 's> {
    parent_query: Query<'w, 's, &'static ChildOf>,
    transform_query: Query<'w, 's, &'static LocalTransform>,
}

impl<'w, 's> TransformHelper<'w, 's> {
    /// Computes the [`GlobalTransform`] of the given entity from the [`LocalTransform`] component on it and its ancestors.
    pub fn compute_global_transform(
        &self,
        entity: Entity,
    ) -> Result<GlobalTransform, ComputeGlobalTransformError> {
        let transform = self
            .transform_query
            .get(entity)
            .map_err(|err| map_error(err, false))?;

        let mut global_transform = GlobalTransform::from(*transform);

        for entity in self.parent_query.iter_ancestors(entity) {
            let transform = self
                .transform_query
                .get(entity)
                .map_err(|err| map_error(err, true))?;

            global_transform = *transform * global_transform;
        }

        Ok(global_transform)
    }
}

fn map_error(err: QueryEntityError, ancestor: bool) -> ComputeGlobalTransformError {
    match err {
        QueryEntityError::QueryDoesNotMatch(entity, _) => MissingTransform(entity),
        QueryEntityError::NotSpawned(error) => {
            if ancestor {
                MalformedHierarchy(error)
            } else {
                NoSuchEntity(error)
            }
        },
        QueryEntityError::AliasedMutability(_) => unreachable!(),
    }
}

/// Error returned by [`TransformHelper::compute_global_transform`].
#[derive(Debug, Error)]
pub enum ComputeGlobalTransformError {
    /// The entity or one of its ancestors is missing the [`Transform`] component.
    #[error("The entity {0:?} or one of its ancestors is missing the `Transform` component")]
    MissingTransform(Entity),
    /// The entity does not exist.
    #[error("The entity does not exist: {0}")]
    NoSuchEntity(EntityNotSpawnedError),
    /// An ancestor is missing.
    /// This probably means that your hierarchy has been improperly maintained.
    #[error("The ancestor is missing: {0}")]
    MalformedHierarchy(EntityNotSpawnedError),
}
