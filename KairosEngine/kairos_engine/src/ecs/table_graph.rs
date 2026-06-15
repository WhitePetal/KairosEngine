use std::{
    any::TypeId,
    borrow::Borrow,
    collections::{HashMap, hash_map::Entry},
    ops::{Index, IndexMut},
};

use petgraph::graph::{DiGraph, Node, NodeIndex};

use crate::ecs::{
    component_tuple::DynamicComponentTuple,
    table::{ComponentTypeInfo, Table},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TableGraphGeneration(u32);

#[derive(Debug)]
pub struct TableEdge {}

#[derive(Debug)]
pub struct InsertTarget {
    need_updates: Vec<ComponentTypeInfo>,
    need_moves: Vec<ComponentTypeInfo>,
    target: NodeIndex,
}
impl InsertTarget {
    pub fn get_need_updates(&self) -> &Vec<ComponentTypeInfo> {
        &self.need_updates
    }

    pub fn get_need_moves(&self) -> &Vec<ComponentTypeInfo> {
        &self.need_moves
    }

    pub fn get_node_index(&self) -> NodeIndex {
        self.target
    }
}

#[derive(Debug)]
pub struct TableGraph {
    // 我们让Table只create而不delete，因此其NodeIndex天然是稳定的
    // 不需要引入StableDiGraph的安全检查，直接使用DiGraph
    graph: DiGraph<Table, TableEdge>,
    index: HashMap<Box<[TypeId]>, NodeIndex>,

}

impl TableGraph {
    pub fn new(table_capacity: usize) -> Self {
        let edge_capacity = table_capacity << 1;
        let mut graph = DiGraph::with_capacity(table_capacity, edge_capacity);
        graph.add_node(Table::new(Box::new([])));
        let index = HashMap::with_capacity(table_capacity);
        Self { graph, index }
    }

    pub fn get_insert_target<T: DynamicComponentTuple>(
        &mut self,
        source_table: NodeIndex,
        components: &T,
    ) -> InsertTarget {
        let table = &mut self.graph[source_table];
        let mut infos = table.types().to_vec();
        let mut need_updates = Vec::new();
        let mut need_moves = Vec::new();

        let mut src_ty = 0;
        let tabe_colum_count = table.colum_count();
        for ty in components.type_infos() {
            // src_table 中存在，但 insert_components 中不存在的类型
            // 这些类型的数据需要移动到新表中
            while src_ty < tabe_colum_count && table.types()[src_ty] <= ty {
                if table.types()[src_ty] != ty {
                    need_moves.push(table.types()[src_ty]);
                }
                src_ty = src_ty + 1;
            }
            // src_table 和 insert_components 中都存在的类型
            // 这些类型的数据需要用insert_component的数据做更新
            if table.has_component_type_id(ty.id()) {
                need_updates.push(ty);
            }
            // src_table 中不存在，仅在 insert_components 中存在的类型
            // 这些类型数据只需要用 insert_component 做创建
            else {
                infos.push(ty);
            }
        }
        infos.sort_unstable();
        let infos = infos.into_boxed_slice();
        // 同样是 src_table 中存在，但 insert_components 中不存在的类型
        // 这些类型的数据需要移动到新表中
        need_moves.extend_from_slice(&table.types()[src_ty..]);

        let types = infos.iter().map(|info| info.id()).collect::<Box<_>>();
        let target = self.get_target_node_index(types, || infos);
        InsertTarget {
            need_updates,
            need_moves,
            target,
        }
    }

    fn get_target_node_index<
        T: Borrow<[TypeId]> + Into<Box<[TypeId]>>,
        F: FnOnce() -> Box<[ComponentTypeInfo]>,
    >(
        &mut self,
        types: T,
        get_type_infos: F,
    ) -> NodeIndex {
        self.index
            .get(types.borrow())
            .copied()
            .unwrap_or_else(|| self.insert_table(types.into(), get_type_infos()))
    }

    fn insert_table(&mut self, types: Box<[TypeId]>, infos: Box<[ComponentTypeInfo]>) -> NodeIndex {
        let index = self.graph.add_node(Table::new(infos));
        self.index.insert(types, index);
        index
    }

    pub fn index2(&mut self, index0: NodeIndex, index1: NodeIndex) -> (&mut Table, &mut Table) {
        debug_assert!(index0 != index1);
        debug_assert!(index0.index() < self.graph.node_count());
        debug_assert!(index1.index() < self.graph.node_count());

        unsafe {
            let a = &mut self.graph[index0] as *mut Table;
            let b = &mut self.graph[index1] as *mut Table;
            (&mut *a, &mut *b)
        }
    }

    /// 通过types获取原型表，如果不存在该类原型表，那么就通过 get_infos 创建该类型原型表
    pub fn get<
        T: Borrow<[TypeId]> + Into<Box<[TypeId]>>,
        F: FnOnce() -> Box<[ComponentTypeInfo]>,
    >(
        &mut self,
        types: T,
        get_infos: F,
    ) -> NodeIndex {
        self.index
            .get(types.borrow())
            .copied()
            .unwrap_or_else(|| self.insert_table(types.into(), (get_infos)()))
    }

    /// 插入整个Table
    ///
    /// - Return:
    ///     - table_node_index
    ///     - row_index: 插入的Table可能在Geaph已存在，此时会合并，并返回该Tabe entity在新 Table 中的起始row_index
    ///         否则为0
    pub fn insert_batch(&mut self, table: Table) -> (NodeIndex, usize) {
        let types = table
            .types()
            .iter()
            .map(|info| info.id())
            .collect::<Box<[_]>>();

        match self.index.entry(types) {
            Entry::Occupied(occupied_entry) => {
                // 已有该原型，合并
                let existing = &mut self.graph[*occupied_entry.get()];
                let base = existing.row_count();
                existing.merge(table);
                (*occupied_entry.get(), base)
            }
            Entry::Vacant(vacant_entry) => {
                // 没有该原型，创建并插入新表
                let id = self.graph.add_node(table);
                vacant_entry.insert(id);
                (id, 0)
            }
        }
    }

    pub fn get_tables_mut(&mut self) -> impl Iterator<Item = &mut Table> {
        self.graph.node_weights_mut()
    }

    pub fn get_tables(&self) -> &[Node<Table>] {
        self.graph.raw_nodes()
    }

    pub unsafe fn get_table_node_unchecked(&self, index: usize) -> &Node<Table> {
        unsafe { self.graph.raw_nodes().get_unchecked(index) }
    }

    pub fn gneeration(&self) -> TableGraphGeneration {
        TableGraphGeneration(self.graph.node_count() as u32)
    }
}

impl Index<NodeIndex> for TableGraph {
    type Output = Table;

    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.graph[index]
    }
}
impl IndexMut<NodeIndex> for TableGraph {
    fn index_mut(&mut self, index: NodeIndex) -> &mut Self::Output {
        &mut self.graph[index]
    }
}
