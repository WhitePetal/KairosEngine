use std::{
    any::type_name,
    error::Error,
    fmt::{self, Display},
};

use crate::ecs::sparse_set::NoSuchId;

pub trait Component: 'static {}

/// 用于在Error中表示实体没有需要的组件
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MissingComponent(&'static str);

impl MissingComponent {
    pub fn new<T: Component>() -> Self {
        Self(type_name::<T>())
    }
}

impl fmt::Display for MissingComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "missing {} component", self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum ComponentError {
    NoSuchEntity,
    MissingComponent(MissingComponent),
}

impl Error for ComponentError {}

impl Display for ComponentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentError::NoSuchEntity => f.write_str("no such entity"),
            ComponentError::MissingComponent(missing_component) => missing_component.fmt(f),
        }
    }
}

impl From<NoSuchId> for ComponentError {
    fn from(value: NoSuchId) -> Self {
        Self::NoSuchEntity
    }
}

impl From<MissingComponent> for ComponentError {
    fn from(value: MissingComponent) -> Self {
        Self::MissingComponent(value)
    }
}
