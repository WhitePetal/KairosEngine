use std::{any::TypeId, collections::HashMap};

use petgraph::{adj::NodeIndex, stable_graph::StableDiGraph};

use crate::ecs::table::Table;

#[derive(Debug)]
pub struct TableEdge {}

#[derive(Debug)]
pub struct TableGraph {
    graph: StableDiGraph<Table, TableEdge>,
    index: HashMap<Box<TypeId>, NodeIndex>,
}

impl TableGraph {
    pub fn new(table_capacity: usize) -> Self {
        let graph = StableDiGraph::with_capacity(table_capacity, table_capacity << 2);
        let index = HashMap::with_capacity(table_capacity);
        Self { graph, index }
    }
}
