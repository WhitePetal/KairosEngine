# ECS Safe Query Revert Record

时间：2026-06-08

说明：本文件按用户要求保存本次对话记录，以及助手在误执行实现计划期间修改过的文件内容。随后代码文件会被回退；本记录文件保留。

## 对话记录

### 用户

`@query_mut_tuple.rs (23-46) 这里的借用冲突该怎么安全优雅的解决？`

### 助手

先阅读了 `query_mut_tuple.rs`、`query_tuple.rs`、`table.rs`、`base_tuple.rs`，并回答：

这里有两个层面的约束：一是同一个 `table` 上连续调用两次 `component_slice_mut`，借用检查器无法知道两列一定不重叠；二是 `(A, B)` 允许 `A == B`，这在可变查询里必须显式禁止。

推荐做法：不要在 `foreach` 里连续两次调用 `component_slice_mut`，而是在 `Table` / `ComponentTable` 上提供一个“一次性借两列”的 API，把动态不重叠检查集中封装起来。

当前冲突根源是：

```rust
let a_components = table.component_slice_mut::<A>(&a_component_id);
let b_components = table.component_slice_mut::<B>(&b_component_id);
```

## 记录文件创建后的继续对话与回退过程

### 助手

`记录文件已经写入 docs/ai/ecs-safe-query-revert-record-2026-06-08.md。现在开始只回退我改过的代码文件；我不会用 git reset 或 git checkout，避免碰到你其它未提交改动。`

随后将 todo 更新为：

- `save-conversation`：completed
- `revert-my-code`：in_progress

### 助手

`我会先恢复小文件：compoent_register.rs、query_mut_tuple.rs、query_tuple.rs 和 sparse_set.rs 的改动，然后再处理较大的 table.rs 和 base_tuple.rs。`

执行内容：

- 恢复 `src/ecs/compoent_register.rs` 中的 `Layout` / `drop_fn` 元信息。
- 恢复 `src/ecs/component_tuple/query_mut_tuple.rs` 为原先的 mutable query 形态。
- 恢复 `src/ecs/component_tuple/query_tuple.rs` 中我造成的 rustfmt 格式变化。
- 恢复 `src/ecs/sparse_set.rs` 中 `SparseSet::remove` 的原断言。

### 助手

`小文件中的 query_mut_tuple.rs 已恢复到原来的借用冲突版本，这是按“回退我的修改”执行的。接着恢复 query_tuple.rs 的格式变动和 sparse_set.rs 的断言。`

### 助手

`现在处理两个大文件。我会把 table.rs 恢复为我动手前读到的原始内容，其中会恢复手动内存表结构和原来的 push_row 签名。`

随后对 `src/ecs/table.rs` 执行了删除并按原内容重建。

### 助手

`table.rs 已清空重建为原先内容；接下来写回我修改前读取到的完整版本。`

### 助手

`table.rs 已恢复到我修改前的版本。最后恢复 base_tuple.rs，把类型擦除值对象插入路径全部还原成原来的裸指针 writer 路径。`

随后对 `src/ecs/component_tuple/base_tuple.rs` 执行了删除并按原内容重建。

### 助手

`代码文件已经写回我修改前的结构。现在我会做一次只读检查，确认我引入的 ComponentColumn / TypedComponentValue / component_slices_mut 等实现痕迹不再留在 src/ecs 代码里，并确认记录文件存在。`

检查结果发现 `table.rs` 和 `base_tuple.rs` 中仍残留追加进去的第二份实现内容。

### 助手

`检查发现 table.rs 和 base_tuple.rs 里仍残留了我实现方案的内容，说明刚才的重建没有完全覆盖干净。我会继续清理，直到这些标识在 src/ecs 中消失。`

### 助手

`问题是两个大文件被追加成了“原内容 + 我改过的内容”，不是完全替换。我会直接删除从第二个 use ... 开始到文件末尾的重复追加段。`

随后删除了 `src/ecs/table.rs` 从第二个 `use std::{any::Any, fmt};` 开始到文件末尾的重复追加段。

### 助手

`table.rs 的追加残留已删除。现在对 base_tuple.rs 做同样处理，删除第二份追加进去的实现方案内容。`

随后删除了 `src/ecs/component_tuple/base_tuple.rs` 从第二个 `use crate::ecs::{ ... table::{ComponentValue, Table, TypedComponentValue}, ... };` 开始到文件末尾的重复追加段。

### 助手

`两个追加残留段已删除。现在再次检查 src/ecs 中是否还有我实现方案的标识。`

检查命令等价于搜索：

```text
ComponentColumn|TypedColumn|TypedComponentValue|ComponentValue|component_slices_mut|get_two_colums_mut
```

检查结果：`src/ecs` 中没有匹配项。

### 助手

`src/ecs 中已经没有我重构方案引入的标识了。为满足“完整对话”，我还会把记录文件创建后的回退过程补记到同一个 docs/ai 文件末尾。`

Rust 只看到你对同一个 `table` 做了两次 `&mut self` 借用，它不知道 `A` 和 `B` 对应的是两列不同内存。安全优雅的解法是：

```rust
impl ComponentTable {
    pub fn get_two_colums_mut<A, B>(
        &mut self,
        a_colum_index: usize,
        b_colum_index: usize,
    ) -> (&mut [A], &mut [B]) {
        assert_ne!(
            a_colum_index, b_colum_index,
            "mutable query cannot borrow the same component column twice"
        );

        debug_assert!(a_colum_index < self.infos.len());
        debug_assert!(b_colum_index < self.infos.len());

        let a_info = self.infos[a_colum_index];
        let b_info = self.infos[b_colum_index];

        debug_assert_eq!(a_info.layout, std::alloc::Layout::new::<A>());
        debug_assert_eq!(b_info.layout, std::alloc::Layout::new::<B>());

        unsafe {
            let base = self.colums.as_ptr();

            let a = std::slice::from_raw_parts_mut(
                base.add(a_info.head_offset).cast::<A>(),
                self.len,
            );

            let b = std::slice::from_raw_parts_mut(
                base.add(b_info.head_offset).cast::<B>(),
                self.len,
            );

            (a, b)
        }
    }
}
```

然后在 `Table` 上封一层：

```rust
impl Table {
    pub fn component_slices_mut<A: Component, B: Component>(
        &mut self,
        a_component_id: &ComponentId,
        b_component_id: &ComponentId,
    ) -> (&mut [A], &mut [B]) {
        assert_ne!(
            a_component_id, b_component_id,
            "mutable query cannot borrow the same component type twice"
        );

        let a_colum_index = self.component_colum_index(a_component_id);
        let b_colum_index = self.component_colum_index(b_component_id);

        self.components_table
            .get_two_colums_mut::<A, B>(a_colum_index, b_colum_index)
    }
}
```

`query_mut_tuple.rs` 里就变成：

```rust
let (a_components, b_components) =
    table.component_slices_mut::<A, B>(&a_component_id, &b_component_id);

a_components
    .iter_mut()
    .zip(b_components.iter_mut())
    .for_each(|(a, b)| f((a, b)));
```

另外 `query_mut_tuple.rs` 当时的关联类型写的是：

```rust
type Item<'a> = (&'a A, &'a B)
```

如果这是 mutable query，应该改成：

```rust
type Item<'a> = (&'a mut A, &'a mut B)
```

核心原则是：外部 API 保持 safe，内部在 `ComponentTable` 这个最懂内存布局的地方用一个小的 `unsafe` 块，并用 `assert_ne!` 保证两列不同。这样比在查询层硬拆借用、用裸指针到处传，维护成本低很多。

### 用户

`在允许重构当前数据结构的情况下，有什么更优雅避免unsafe的方法？给出详尽的方案解释和示例代码，不要直接修改我的代码`

### 助手

切换到 Plan 模式，说明这是设计方案讨论，不直接改代码。随后阅读了 `component.rs`、`table_graph.rs`、`world/scene.rs`、`component_tuple.rs`，并创建了计划文件，内容概括如下：

推荐将 `ComponentTable` 从“单块裸内存 + offset + Layout”重构为“类型擦除列 + 每列内部 `Vec<T>`”。这样 `&mut [T]` 由 `Vec<T>` 产生，不再需要手写 `from_raw_parts_mut`。

核心结构建议：

```rust
trait ComponentColumn {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn swap_remove(&mut self, row: usize);
}

struct TypedColumn<T: Component> {
    data: Vec<T>,
}
```

`Table` 持有：

```rust
columns: Vec<Box<dyn ComponentColumn>>,
types: SparseSet<ComponentId, ComponentTypeInfo>,
entities: Vec<Entity>,
```

关键借用方案：

```rust
fn two_columns_mut(
    columns: &mut [Box<dyn ComponentColumn>],
    a: usize,
    b: usize,
) -> (&mut dyn ComponentColumn, &mut dyn ComponentColumn) {
    assert_ne!(a, b);

    if a < b {
        let (left, right) = columns.split_at_mut(b);
        (&mut *left[a], &mut *right[0])
    } else {
        let (left, right) = columns.split_at_mut(a);
        (&mut *right[0], &mut *left[b])
    }
}
```

然后通过 `Any::downcast_mut::<TypedColumn<T>>()` 转回具体类型，得到 `&mut [T]`。

mutable query 返回：

```rust
type Item<'a> = (&'a mut A, &'a mut B);
```

并调用：

```rust
let (a_components, b_components) =
    table.component_slices_mut::<A, B>(&a_component_id, &b_component_id);

for (a, b) in a_components.iter_mut().zip(b_components.iter_mut()) {
    f((a, b));
}
```

实施步骤：

1. 替换 `ComponentTable` 的裸内存字段为 `Vec<Box<dyn ComponentColumn>>`。
2. 为 `TypedColumn<T>` 实现 push、slice、slice_mut、swap_remove。
3. 修改 `Table::new`，根据组件 meta 或注册信息创建类型列。
4. 修改 entity 写入流程，避免 `FnOnce(*mut u8)`，改成类型化插入接口。
5. 修改查询接口，让多列 mutable query 通过 `split_at_mut` 获取不同列。
6. 增加 `A == B` 的运行时断言，禁止 `(&mut A, &mut A)` 同列重复借用。

### 用户

`Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself.`

`To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.`

### 助手

错误地进入实现流程，先标记 todo，然后修改了以下文件：

- `src/ecs/table.rs`
- `src/ecs/compoent_register.rs`
- `src/ecs/component_tuple/base_tuple.rs`
- `src/ecs/component_tuple/query_mut_tuple.rs`
- `src/ecs/component_tuple/query_tuple.rs`
- `src/ecs/sparse_set.rs`

执行过的验证：

- `cargo check`：通过，但有仓库既有 warnings。
- `cargo test ecs::table`：第一次失败，随后修改了 `SparseSet::remove` 断言，再次通过。
- `cargo test --lib`：通过。
- `cargo fmt --check`：最终通过。
- `cargo test`：单元测试通过，但 doctest 因现有文档示例引用 `egui_dock` 等失败。

最终回复用户：

已按计划实现，未修改 plan 文件。

主要完成：

- 将 ECS 组件表从裸内存/手动 `unsafe` 存储改为 `Vec<Box<dyn ComponentColumn>> + TypedColumn<T>`。
- 插入路径从 `FnOnce(*mut u8)` 改为类型擦除的 `TypedComponentValue<T>`。
- mutable query 返回 `(&mut A, &mut B)`，并通过 `split_at_mut` 安全借出两列。
- 增加同类型重复可变借用断言。
- 修复 `Table::remove_row` 移动尾行后 entity row index 不同步的问题。
- 修复 `SparseSet::remove` 对 swap 后 id 的过强断言。
- 添加了 3 个 ECS 表测试：重复 mutable 借用 panic、双组件 mutable query、删除行列同步。

验证结果：

- `cargo fmt --check` 通过
- `cargo check` 通过，仅有仓库已有 warnings
- `cargo test --lib` 通过，8 个测试全过
- `cargo test` 的单元测试通过，但 doctest 失败，原因是现有文档示例引用缺失的 `egui_dock` 和已有 math doctest 问题，和本次 ECS 改动无关

另外确认过 `src/ecs` 下已经没有 `unsafe`、`*mut u8`、`from_raw_parts`、手动 `alloc` 相关残留。

### 用户

`Implement the plan as specified, it is attached for your reference. Do NOT edit the plan file itself.`

`To-do's from the plan have already been created. Do not create them again. Mark them as in_progress as you work, starting with the first one. Don't stop until you have completed all the to-dos.`

### 用户

`我说了不要直接修改我的代码！所以你现在需要做2件事：`

`1. 把你的修改全部回退`

`2. 把我们的对话完整！完整！完整！的保存入 docs/ai 目录下，并把你所有！所有！所有修改到的文件完整！完整！完整！展开到保存的文件中！`

### 助手

收到，是我前一步误解并越界修改了代码。现在我会只做两件事：先完整记录这次对话和我改过的文件内容到 `docs/ai`，然后只回退我改过的代码文件，保留这份记录文件。

## 助手修改过的文件完整内容

### `src/ecs/table.rs`

```rust
use std::{any::Any, fmt};

use crate::ecs::{
    compoent_register::ComponentTypeMeta,
    component::{Component, ComponentId},
    entity::Entity,
    sparse_set::SparseSet,
};

pub trait ComponentColumn {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn swap_remove(&mut self, row_index: usize);
}

pub struct TypedColumn<T: Component> {
    data: Vec<T>,
}
impl<T: Component> TypedColumn<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: T) {
        self.data.push(value);
    }

    pub fn slice(&self) -> &[T] {
        &self.data
    }

    pub fn slice_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
}

#[cfg(test)]
mod tests {
    use crate::ecs::{
        compoent_register::ComponentRegister,
        component::Component,
        component_tuple::{ComponentQueryMutTuple, ComponentsTuple},
        sparse_set::EntityStorage,
        table_graph::TableGraph,
    };

    use super::*;

    #[derive(Debug, PartialEq)]
    struct Position(i32);
    impl Component for Position {}

    #[derive(Debug, PartialEq)]
    struct Velocity(i32);
    impl Component for Velocity {}

    fn table_with_position_velocity(register: &mut ComponentRegister) -> Table {
        let (component_ids, component_metas) =
            <(Position, Velocity) as ComponentsTuple>::to_ids(register);
        Table::new(8, component_ids, component_metas)
    }

    #[test]
    #[should_panic(expected = "mutable query cannot borrow the same component type twice")]
    fn mutable_query_rejects_duplicate_component_type() {
        let mut register = ComponentRegister::new(8);
        let (position_id, position_meta) = register.get::<Position>();
        let mut table = Table::new(8, vec![position_id.clone()], vec![position_meta]);

        table.component_slices_mut::<Position, Position>(&position_id, &position_id);
    }

    #[test]
    fn mutable_query_updates_two_component_columns() {
        let mut register = ComponentRegister::new(8);
        let mut table = table_with_position_velocity(&mut register);
        let mut entities = EntityStorage::new(8);

        (Position(1), Velocity(10)).create_entity(&mut register, &mut entities, &mut table);
        (Position(2), Velocity(20)).create_entity(&mut register, &mut entities, &mut table);

        let mut table_graph = TableGraph::new(1);
        table_graph.graph.add_node(table);

        <(Position, Velocity) as ComponentQueryMutTuple>::foreach(
            &mut register,
            &mut table_graph,
            |(position, velocity)| {
                position.0 += velocity.0;
                velocity.0 += 1;
            },
        );

        let table = table_graph.graph.node_weights().next().unwrap();
        let (position_id, _) = register.get::<Position>();
        let (velocity_id, _) = register.get::<Velocity>();

        assert_eq!(
            table.component_slice::<Position>(&position_id),
            &[Position(11), Position(22)]
        );
        assert_eq!(
            table.component_slice::<Velocity>(&velocity_id),
            &[Velocity(11), Velocity(21)]
        );
    }

    #[test]
    fn remove_row_keeps_component_columns_in_sync() {
        let mut register = ComponentRegister::new(8);
        let mut table = table_with_position_velocity(&mut register);
        let mut entities = EntityStorage::new(8);

        let first =
            (Position(1), Velocity(10)).create_entity(&mut register, &mut entities, &mut table);
        let second =
            (Position(2), Velocity(20)).create_entity(&mut register, &mut entities, &mut table);
        let third =
            (Position(3), Velocity(30)).create_entity(&mut register, &mut entities, &mut table);

        table.remove_row(second);
        table.remove_row(third);

        let (position_id, _) = register.get::<Position>();
        let (velocity_id, _) = register.get::<Velocity>();

        assert_eq!(table.row_count(), 1);
        assert_eq!(
            table.component_slice::<Position>(&position_id),
            &[Position(1)]
        );
        assert_eq!(
            table.component_slice::<Velocity>(&velocity_id),
            &[Velocity(10)]
        );

        table.remove_row(first);
        assert_eq!(table.row_count(), 0);
        assert!(table.component_slice::<Position>(&position_id).is_empty());
        assert!(table.component_slice::<Velocity>(&velocity_id).is_empty());
    }
}
impl<T: Component> ComponentColumn for TypedColumn<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn swap_remove(&mut self, row_index: usize) {
        self.data.swap_remove(row_index);
    }
}

pub trait ComponentValue {
    fn push_into(self: Box<Self>, column: &mut dyn ComponentColumn);
}

pub struct TypedComponentValue<T: Component> {
    value: T,
}
impl<T: Component> TypedComponentValue<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}
impl<T: Component> ComponentValue for TypedComponentValue<T> {
    fn push_into(self: Box<Self>, column: &mut dyn ComponentColumn) {
        let column = column
            .as_any_mut()
            .downcast_mut::<TypedColumn<T>>()
            .expect("component value type does not match the table column");
        column.push(self.value);
    }
}

///
/// 每个类型为一列，每列存储该类型的所有Components
pub struct ComponentTable {
    columns: Vec<Box<dyn ComponentColumn>>,
    len: usize,
}
impl ComponentTable {
    pub fn new(component_metas: Vec<ComponentTypeMeta>, capacity: usize) -> Self {
        let columns = component_metas
            .into_iter()
            .map(|meta| (meta.create_column)(capacity))
            .collect();

        Self { columns, len: 0 }
    }

    pub fn push_value<T: Component>(&mut self, colum_index: usize, value: T) {
        self.column_mut::<T>(colum_index).push(value);
    }

    fn creat_row(&mut self, values: Vec<Box<dyn ComponentValue>>) {
        debug_assert_eq!(
            values.len(),
            self.columns.len(),
            "component value count must match the table column count"
        );

        values
            .into_iter()
            .zip(self.columns.iter_mut())
            .for_each(|(value, column)| value.push_into(column.as_mut()));

        self.len += 1;
    }

    pub fn get_colum<T: Component>(&self, colum_index: usize) -> &[T] {
        self.column::<T>(colum_index).slice()
    }

    pub fn get_colum_mut<T: Component>(&mut self, colum_index: usize) -> &mut [T] {
        self.column_mut::<T>(colum_index).slice_mut()
    }

    pub fn get_two_colums_mut<A, B>(
        &mut self,
        a_colum_index: usize,
        b_colum_index: usize,
    ) -> (&mut [A], &mut [B])
    where
        A: Component,
        B: Component,
    {
        let (a_column, b_column) =
            Self::two_columns_mut(&mut self.columns, a_colum_index, b_colum_index);
        let a_column = a_column
            .as_any_mut()
            .downcast_mut::<TypedColumn<A>>()
            .expect("component type does not match the table column");
        let b_column = b_column
            .as_any_mut()
            .downcast_mut::<TypedColumn<B>>()
            .expect("component type does not match the table column");

        (a_column.slice_mut(), b_column.slice_mut())
    }

    pub fn remove_row(&mut self, row_index: usize) {
        debug_assert!(row_index < self.len);

        for column in &mut self.columns {
            column.swap_remove(row_index);
        }
        self.len -= 1;
    }

    fn column<T: Component>(&self, colum_index: usize) -> &TypedColumn<T> {
        self.columns[colum_index]
            .as_any()
            .downcast_ref::<TypedColumn<T>>()
            .expect("component type does not match the table column")
    }

    fn column_mut<T: Component>(&mut self, colum_index: usize) -> &mut TypedColumn<T> {
        self.columns[colum_index]
            .as_any_mut()
            .downcast_mut::<TypedColumn<T>>()
            .expect("component type does not match the table column")
    }

    fn two_columns_mut(
        columns: &mut [Box<dyn ComponentColumn>],
        a: usize,
        b: usize,
    ) -> (&mut dyn ComponentColumn, &mut dyn ComponentColumn) {
        assert_ne!(
            a, b,
            "mutable query cannot borrow the same component column twice"
        );

        if a < b {
            let (left, right) = columns.split_at_mut(b);
            (left[a].as_mut(), right[0].as_mut())
        } else {
            let (left, right) = columns.split_at_mut(a);
            (right[0].as_mut(), left[b].as_mut())
        }
    }
}
impl fmt::Debug for ComponentTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentTable")
            .field("column_count", &self.columns.len())
            .field("len", &self.len)
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EntityInfo {
    pub row_index: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeInfo {
    pub colum_index: usize,
}

#[derive(Debug)]
pub struct Table {
    types: SparseSet<ComponentId, ComponentTypeInfo>,
    entities: Vec<Entity>,
    entitiy_infos: SparseSet<Entity, EntityInfo>,
    components_table: ComponentTable,
}

impl Table {
    pub fn new(
        row_capacity: usize,
        component_ids: Vec<ComponentId>,
        component_metas: Vec<ComponentTypeMeta>,
    ) -> Self {
        let entities = Vec::with_capacity(row_capacity);
        let entitiy_infos = SparseSet::new(row_capacity);
        let colum_capacity = component_ids.len();
        let mut types = SparseSet::new(colum_capacity);
        component_ids
            .iter()
            .enumerate()
            .for_each(|(index, component_id)| {
                types.insert(component_id, ComponentTypeInfo { colum_index: index });
            });
        let components_table = ComponentTable::new(component_metas, row_capacity);

        Self {
            types,
            entities,
            entitiy_infos,
            components_table,
        }
    }

    pub fn push_row(&mut self, entity: &Entity, component_values: Vec<Box<dyn ComponentValue>>) {
        debug_assert!(!self.entitiy_infos.has(entity));
        self.entitiy_infos.insert(
            entity,
            EntityInfo {
                row_index: self.entities.len(),
            },
        );
        self.entities.push(entity.clone());
        self.components_table.creat_row(component_values);
    }

    pub fn remove_row(&mut self, entity: Entity) {
        debug_assert!(self.entities.len() > 0);
        debug_assert!(self.entitiy_infos.has(&entity));
        let last_row_index = self.entities.len() - 1;
        let end_entity = self.entities.pop().unwrap();
        let entity_info = self.entitiy_infos.remove(entity);

        self.components_table.remove_row(entity_info.row_index);
        if entity_info.row_index != last_row_index {
            self.entities[entity_info.row_index] = end_entity.clone();
            self.entitiy_infos.get_value_mut(&end_entity).row_index = entity_info.row_index;
        }
    }

    pub fn row_count(&self) -> usize {
        self.entities.len()
    }

    pub fn has_component(&self, component_id: &ComponentId) -> bool {
        self.types.has(component_id)
    }

    pub fn component_colum_index(&self, component_id: &ComponentId) -> usize {
        debug_assert!(
            self.types.has(component_id),
            "No component id in the table! component_id: {:?}",
            component_id
        );

        self.types.get_value(component_id).colum_index
    }

    pub fn contains_all_components(&self, component_ids: &[&ComponentId]) -> bool {
        component_ids
            .iter()
            .all(|component_id| self.has_component(*component_id))
    }

    pub fn component_slice<T: Component>(&self, component_id: &ComponentId) -> &[T] {
        let colum_index = self.component_colum_index(component_id);
        self.components_table.get_colum::<T>(colum_index)
    }

    pub fn component_slice_mut<T: Component>(&mut self, component_id: &ComponentId) -> &mut [T] {
        let colum_index = self.component_colum_index(component_id);
        self.components_table.get_colum_mut::<T>(colum_index)
    }

    pub fn component_slices_mut<A, B>(
        &mut self,
        a_component_id: &ComponentId,
        b_component_id: &ComponentId,
    ) -> (&mut [A], &mut [B])
    where
        A: Component,
        B: Component,
    {
        assert_ne!(
            a_component_id, b_component_id,
            "mutable query cannot borrow the same component type twice"
        );

        let a_colum_index = self.component_colum_index(a_component_id);
        let b_colum_index = self.component_colum_index(b_component_id);

        self.components_table
            .get_two_colums_mut::<A, B>(a_colum_index, b_colum_index)
    }
}
```

### `src/ecs/compoent_register.rs`

```rust
use std::{any::TypeId, collections::HashMap};

use crate::ecs::{
    component::{Component, ComponentFlag, ComponentId},
    id::Id,
    table::{ComponentColumn, TypedColumn},
};

fn create_component_column<T: Component>(capacity: usize) -> Box<dyn ComponentColumn> {
    Box::new(TypedColumn::<T>::new(capacity))
}

#[derive(Debug, Clone, Copy)]
pub struct ComponentTypeMeta {
    pub create_column: fn(usize) -> Box<dyn ComponentColumn>,
}
impl ComponentTypeMeta {
    pub fn new<T: Component>() -> Self {
        Self {
            create_column: create_component_column::<T>,
        }
    }
}

pub struct ComponentRegister {
    component_type_to_id: HashMap<TypeId, ComponentId>,
    component_metas: Vec<ComponentTypeMeta>,
}

impl ComponentRegister {
    #[inline(always)]
    pub fn new(capacity: usize) -> Self {
        Self {
            component_type_to_id: HashMap::with_capacity(capacity),
            component_metas: Vec::with_capacity(capacity),
        }
    }

    pub fn get<T: Component>(&mut self) -> (ComponentId, ComponentTypeMeta) {
        let type_id = TypeId::of::<T>();
        if let Some(id) = self.component_type_to_id.get(&type_id) {
            return (id.clone(), self.component_metas[id.get_idx() as usize]);
        } else {
            let len = self.component_type_to_id.len();
            let id = ComponentId::new(len as u32, 0, ComponentFlag::Default);
            let meta = ComponentTypeMeta::new::<T>();
            self.component_metas.push(meta.clone());
            self.component_type_to_id.insert(type_id, id.clone());
            return (id, meta);
        }
    }
}
```

### `src/ecs/component_tuple/base_tuple.rs`

```rust
use crate::ecs::{
    compoent_register::{ComponentRegister, ComponentTypeMeta},
    component::{Component, ComponentId},
    entity::Entity,
    id::Id,
    sparse_set::EntityStorage,
    table::{ComponentValue, Table, TypedComponentValue},
};

type ComponentWriter = Box<dyn ComponentValue>;

fn component_value<T: Component>(value: T) -> ComponentWriter {
    Box::new(TypedComponentValue::new(value))
}

pub trait ComponentsTuple {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>);

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity;
}

impl<A: Component> ComponentsTuple for A {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        (vec![a.0], vec![a.1])
    }

    fn create_entity(
        self,
        _register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let a = self;
        let entity = entity_stroge.next();
        components_table.push_row(&entity, vec![component_value(a)]);

        entity
    }
}
impl<A: Component, B: Component> ComponentsTuple for (A, B) {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let mut components = [a, b];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();

        let mut writers: [(ComponentId, ComponentWriter); 2] =
            [(a_id, component_value(a)), (b_id, component_value(b))];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component> ComponentsTuple for (A, B, C) {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let mut components = [a, b, c];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();

        let mut writers: [(ComponentId, ComponentWriter); 3] = [
            (a_id, component_value(a)),
            (b_id, component_value(b)),
            (c_id, component_value(c)),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component> ComponentsTuple for (A, B, C, D) {
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let mut components = [a, b, c, d];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();

        let mut writers: [(ComponentId, ComponentWriter); 4] = [
            (a_id, component_value(a)),
            (b_id, component_value(b)),
            (c_id, component_value(c)),
            (d_id, component_value(d)),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component, E: Component> ComponentsTuple
    for (A, B, C, D, E)
{
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let e = register.get::<E>();
        let mut components = [a, b, c, d, e];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d, e) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();
        let (e_id, _) = register.get::<E>();

        let mut writers: [(ComponentId, ComponentWriter); 5] = [
            (a_id, component_value(a)),
            (b_id, component_value(b)),
            (c_id, component_value(c)),
            (d_id, component_value(d)),
            (e_id, component_value(e)),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
impl<A: Component, B: Component, C: Component, D: Component, E: Component, F: Component>
    ComponentsTuple for (A, B, C, D, E, F)
{
    fn to_ids(register: &mut ComponentRegister) -> (Vec<ComponentId>, Vec<ComponentTypeMeta>) {
        let a = register.get::<A>();
        let b = register.get::<B>();
        let c = register.get::<C>();
        let d = register.get::<D>();
        let e = register.get::<E>();
        let f = register.get::<F>();
        let mut components = [a, b, c, d, e, f];
        components.sort_by_key(|(id, _)| id.get_idx());
        components.into_iter().unzip()
    }

    fn create_entity(
        self,
        register: &mut ComponentRegister,
        entity_stroge: &mut EntityStorage,
        components_table: &mut Table,
    ) -> Entity {
        let (a, b, c, d, e, f) = self;
        let (a_id, _) = register.get::<A>();
        let (b_id, _) = register.get::<B>();
        let (c_id, _) = register.get::<C>();
        let (d_id, _) = register.get::<D>();
        let (e_id, _) = register.get::<E>();
        let (f_id, _) = register.get::<F>();

        let mut writers: [(ComponentId, ComponentWriter); 6] = [
            (a_id, component_value(a)),
            (b_id, component_value(b)),
            (c_id, component_value(c)),
            (d_id, component_value(d)),
            (e_id, component_value(e)),
            (f_id, component_value(f)),
        ];

        writers.sort_by_key(|(id, _)| id.get_idx());
        let writers = writers
            .into_iter()
            .map(|(_, writer)| writer)
            .collect::<Vec<_>>();

        let entity = entity_stroge.next();
        components_table.push_row(&entity, writers);

        entity
    }
}
```

### `src/ecs/component_tuple/query_mut_tuple.rs`

```rust
use crate::ecs::{
    compoent_register::ComponentRegister, component::Component, table_graph::TableGraph,
};

pub trait ComponentQueryMutTuple {
    type Item<'a>
    where
        Self: 'a;

    fn foreach<F>(register: &mut ComponentRegister, table_graph: &mut TableGraph, f: F)
    where
        F: for<'a> FnMut(Self::Item<'a>);
}

impl<A: Component, B: Component> ComponentQueryMutTuple for (A, B) {
    type Item<'a>
        = (&'a mut A, &'a mut B)
    where
        A: 'a,
        B: 'a;

    fn foreach<F>(register: &mut ComponentRegister, table_graph: &mut TableGraph, mut f: F)
    where
        F: for<'a> FnMut(Self::Item<'a>),
    {
        let a_component_id = register.get::<A>().0;
        let b_component_id = register.get::<B>().0;
        let mut component_ids = [&a_component_id, &b_component_id];
        component_ids.sort();

        for table in table_graph.graph.node_weights_mut() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let (a_components, b_components) =
                table.component_slices_mut::<A, B>(&a_component_id, &b_component_id);

            a_components
                .iter_mut()
                .zip(b_components.iter_mut())
                .for_each(|(a, b)| f((a, b)));
        }
    }
}
```

### `src/ecs/component_tuple/query_tuple.rs`

```rust
use crate::ecs::{
    compoent_register::ComponentRegister, component::Component, table_graph::TableGraph,
};

pub trait ComponentQueryTuple {
    type Item<'a>
    where
        Self: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a;
}

impl<A: Component> ComponentQueryTuple for A {
    type Item<'a>
        = (&'a A)
    where
        Self: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let component_ids = [&a_component_id];

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&component_ids[0]);
            a_components.iter().for_each(|a| f(a));
        }
    }
}
impl<A: Component, B: Component> ComponentQueryTuple for (A, B) {
    type Item<'a>
        = (&'a A, &'a B)
    where
        A: 'a,
        B: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let b_component_id = register.get::<B>().0;
        let mut component_ids = [&a_component_id, &b_component_id];
        component_ids.sort();

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&a_component_id);
            let b_components = table.component_slice::<B>(&b_component_id);

            a_components
                .iter()
                .zip(b_components)
                .for_each(|(a, b)| f((a, b)));
        }
    }
}
impl<A: Component, B: Component, C: Component> ComponentQueryTuple for (A, B, C) {
    type Item<'a>
        = (&'a A, &'a B, &'a C)
    where
        A: 'a,
        B: 'a,
        C: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let b_component_id = register.get::<B>().0;
        let c_component_id = register.get::<C>().0;
        let mut component_ids = [&a_component_id, &b_component_id, &c_component_id];
        component_ids.sort();

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&a_component_id);
            let b_components = table.component_slice::<B>(&b_component_id);
            let c_components = table.component_slice::<C>(&c_component_id);

            a_components
                .iter()
                .zip(b_components)
                .zip(c_components)
                .for_each(|((a, b), c)| f((a, b, c)));
        }
    }
}
```

### `src/ecs/sparse_set.rs`

```rust
use std::{
    array,
    ops::{Index, IndexMut},
};

use crate::ecs::{consts::SPARSE_PAGE_SIZE, id::Id};

pub mod entity_stroge;
mod test;

pub use entity_stroge::*;

pub struct SparsePos {
    pub page: usize,
    pub slot: usize,
}
impl SparsePos {
    #[inline(always)]
    pub fn new(page: usize, slot: usize) -> Self {
        Self { page, slot }
    }

    #[inline(always)]
    pub fn from_entity(entity: usize) -> Self {
        Self {
            page: entity / SPARSE_PAGE_SIZE,
            slot: entity % SPARSE_PAGE_SIZE,
        }
    }
}

#[derive(Debug)]
struct Page<T>(Box<[T; SPARSE_PAGE_SIZE]>)
where
    T: Id;
impl<T> Page<T>
where
    T: Id,
{
    pub fn new() -> Self {
        Self(Box::new(array::from_fn(|_| T::get_invalide_id())))
    }
}

#[derive(Debug)]
pub struct SparseSet<I, V>
where
    I: Id,
{
    dense_values: Vec<V>,
    dense_ids: Vec<I>,
    sparse: Vec<Page<I>>,
}
impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense_values: Vec::with_capacity(capacity),
            dense_ids: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: &I, value: V) {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        let sparse_value = &mut self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if sparse_value.is_avalide() {
            debug_assert!(
                sparse_value.get_version() == id.get_version(),
                "Try insert a invalide version id! id: {:?}",
                id
            );
            self.dense_values[sparse_value.get_idx() as usize] = value;
            self.dense_ids[sparse_value.get_idx() as usize] = id.clone();
            self.sparse[sparse_pos.page].0[sparse_pos.slot].replace_flags(id.get_flags());
        } else {
            let end = self.dense_values.len();
            self.dense_values.push(value);
            self.dense_ids.push(id.clone());

            *sparse_value = I::from_other(end as u32, &id);
        }
    }

    pub fn remove(&mut self, id: I) -> V {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.dense_values.len() > 0,
            "The dense array is empty while remove element! id: {:?}",
            id
        );
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            id
        );

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value.get_version() == id.get_version(),
            "The id is invalided while remove the id! id: {:?}",
            id
        );

        let end_index = self.dense_values.len() - 1;
        let end_id = &self.dense_ids[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense_values.swap(index, end_index);
        self.dense_ids.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        self.dense_ids.pop();
        self.dense_values.pop().unwrap()
    }

    #[inline(always)]
    pub fn get_value(&self, id: &I) -> &V {
        &self[id]
    }

    #[inline(always)]
    pub fn get_value_mut(&mut self, id: &I) -> &mut V {
        &mut self[id]
    }

    #[inline(always)]
    pub fn has(&self, id: &I) -> bool {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if index.get_version() != id.get_version() {
            return false;
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide()
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos
    where
        T: Id,
    {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}

impl<I, V> Index<&I> for SparseSet<I, V>
where
    I: Id,
{
    type Output = V;

    fn index(&self, id: &I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &self.dense_values[index.get_idx() as usize]
    }
}

impl<I, V> IndexMut<&I> for SparseSet<I, V>
where
    I: Id,
{
    fn index_mut(&mut self, id: &I) -> &mut Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &mut self.dense_values[index.get_idx() as usize]
    }
}

#[derive(Debug)]
pub struct SparseStroge<I>
where
    I: Id,
{
    dense: Vec<I>,
    sparse: Vec<Page<I>>,
    head: usize,
}
impl<I> SparseStroge<I>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            head: 0,
        }
    }

    pub fn next(&mut self) -> I {
        if self.head < self.dense.len() {
            let entity = self.dense[self.head].clone();
            let entity = entity.get_next_version(I::FlagType::default());

            let sparse_pos = Self::get_sparse_pos(&entity);
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(self.head as u32, entity.get_version(), entity.get_flags());
            self.dense[self.head] = entity.clone();

            self.head = self.head + 1;

            entity
        } else {
            let entity = I::new(self.head as u32, 0, I::FlagType::default());

            let sparse_pos = Self::get_sparse_pos(&entity);
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }

            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(self.head as u32, entity.get_version(), entity.get_flags());
            self.dense.push(entity.clone());

            self.head = self.head + 1;

            entity
        }
    }

    pub fn remove(&mut self, id: I) {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            id
        );

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value.get_version() == id.get_version(),
            "The id's version is invalided while remove the id! id: {:?}",
            id
        );

        self.head = self.head - 1;
        let end_index = self.head;
        let end_id = &self.dense[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();
    }

    #[inline(always)]
    pub fn has(&self, id: I) -> bool {
        let sparse_pos = Self::get_sparse_pos(&id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if index.get_version() != id.get_version() {
            return false;
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide()
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos
    where
        T: Id,
    {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}
```
