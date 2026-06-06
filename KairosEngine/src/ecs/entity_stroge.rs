use crate::ecs::{entity::Entity, sparse_set::SparseSet};

type EntityStorage = SparseSet<Entity, Entity>;