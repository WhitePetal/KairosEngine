use crate::ecs::{
    entity::{Entity},
    sparse_set::{SparseStroge},
};

pub type EntityStorage = SparseStroge<Entity>;
