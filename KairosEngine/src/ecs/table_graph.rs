use petgraph::graph::DiGraph;

use crate::ecs::table::Table;

#[derive(Debug)]
pub struct TableEdge {}

#[derive(Debug)]
pub struct TableGraph {
    pub graph: DiGraph<Table, TableEdge>,
}

impl TableGraph {
    pub fn new(table_capacity: usize) -> Self {
        let graph = DiGraph::with_capacity(table_capacity, table_capacity << 2);
        Self { graph }
    }
}
