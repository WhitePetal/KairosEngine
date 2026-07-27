use std::any::TypeId;

use crate::{ecs::{component::Component, entity::Entity, sparse_set::SparseSet}, types::TypeIdMap};



#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a `Resource`",
    label = "invalid `Resource`",
    note = "consider annotating `{Self}` with `#[derive(Resource)]`"
)]
pub trait Resource: Component {}


pub struct ResourceEntities(TypeIdMap<Entity>);
