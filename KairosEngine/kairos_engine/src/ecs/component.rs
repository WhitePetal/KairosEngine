mod info;
mod clone;

pub use info::*;
pub use clone::*;


/// The storage used for a specific component type.
///
/// # Examples
/// The [`StorageType`] for a component is configured via the derive attribute
///
/// ```
/// # use bevy_ecs::{prelude::*, component::*};
/// #[derive(Component)]
/// #[component(storage = "SparseSet")]
/// struct A;
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StorageType {
    #[default]
    Table,
    SparseSet,
}

// TODO!
