# ECS Scene 泛型 Components 查询设计对话

日期：2026-06-07

本文件记录了关于当前项目 ECS 架构中，如何通过 `Scene` 查询任意泛型数量组合 `Components` 的讨论。

## 对话 1

### 用户

在当前项目的ecs架构中，该怎么通过scene获取任意泛型数量组合的Components？给出详尽的方案分析和示例代码，不要直接修改我的代码

### 助手

可以。基于你现在的 ECS，推荐把“通过 `Scene` 获取任意泛型数量组合 Components”做成一个 **QueryTuple trait + tuple macro 展开** 的查询系统。

核心思路是：

1. `Scene` 持有所有 archetype table。
2. 查询 `(A, B, C)` 时，先通过 `ComponentRegister` 得到 `A/B/C` 的 `ComponentId`。
3. 遍历 `Scene` 中所有 `Table`，筛选出“包含这些 ComponentId”的表。
4. 在每张匹配表里，根据 `ComponentId -> column_index` 拿到对应列。
5. 按 row 组合成 `(&A, &B, &C)` 或 `(&mut A, &mut B, &mut C)` 返回给系统逻辑。

### 现有结构判断

你现在的存储已经很接近 archetype ECS：

```text
Scene
  table_graph: TableGraph
    graph: DiGraph<Table, TableEdge>

Table
  types: SparseSet<ComponentId, ComponentTypeInfo>
  entities: Vec<Entity>
  components_table: ComponentTable

ComponentTable
  每个 Component 类型是一列
  每个 Entity 是一行
```

也就是说，一个 `Table` 代表一种组件组合，比如：

```text
Table 1: [Transform, Velocity]
Table 2: [Transform, Mesh, Material]
Table 3: [Transform, Velocity, Health]
```

如果用户查询 `(Transform, Velocity)`，应该命中 `Table 1` 和 `Table 3`，而不是只找组件集合完全等于 `[Transform, Velocity]` 的表。

这里很重要：查询要找的是 **superset table**，也就是表拥有查询所需组件即可。

### 需要补的底层 API

你的 `ComponentTable` 里其实已经写过切片接口，但被注释掉了：

```rust
// pub fn get_components<T>(&self, colum_index: usize) -> &[T]
// pub fn get_components_mut<T>(&mut self, colum_index: usize) -> &mut [T]
```

建议恢复并完善成这样：

```rust
impl ComponentTable {
    pub fn len(&self) -> usize {
        self.len
    }

    pub unsafe fn get_components<T: Component>(&self, column_index: usize) -> &[T] {
        debug_assert!(column_index < self.infos.len());

        let info = &self.infos[column_index];
        debug_assert_eq!(info.layout, std::alloc::Layout::new::<T>());

        let ptr = self.colums.as_ptr().add(info.head_offset).cast::<T>();
        std::slice::from_raw_parts(ptr, self.len)
    }

    pub unsafe fn get_components_mut<T: Component>(&mut self, column_index: usize) -> &mut [T] {
        debug_assert!(column_index < self.infos.len());

        let info = &self.infos[column_index];
        debug_assert_eq!(info.layout, std::alloc::Layout::new::<T>());

        let ptr = self.colums.as_ptr().add(info.head_offset).cast::<T>();
        std::slice::from_raw_parts_mut(ptr, self.len)
    }
}
```

注意你现在 `push_value` 里的这个断言大概率是错的：

```rust
debug_assert!(colum_index < self.len);
```

应该是检查列数量，而不是行数量：

```rust
debug_assert!(colum_index < self.infos.len());
```

然后 `Table` 需要暴露根据 `ComponentId` 查列索引的能力：

```rust
impl Table {
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn has_component(&self, id: &ComponentId) -> bool {
        self.types.has(id)
    }

    pub fn column_index(&self, id: &ComponentId) -> Option<usize> {
        if !self.types.has(id) {
            return None;
        }

        Some(self.types.get_value(id).colum_index)
    }

    pub fn contains_all(&self, ids: &[ComponentId]) -> bool {
        ids.iter().all(|id| self.has_component(id))
    }

    pub unsafe fn component_slice<T: Component>(&self, column_index: usize) -> &[T] {
        self.components_table.get_components::<T>(column_index)
    }

    pub unsafe fn component_slice_mut<T: Component>(&mut self, column_index: usize) -> &mut [T] {
        self.components_table.get_components_mut::<T>(column_index)
    }
}
```

### 查询 Trait 设计

Rust 目前没有真正稳定可用的 variadic generics，所以“任意泛型数量”通常不是字面意义上的无限数量，而是通过宏生成：

```rust
(A)
(A, B)
(A, B, C)
(A, B, C, D)
...
```

你的 `ComponentsTuple` 现在已经是手写到 6 个类型。查询也可以用同样模式，但建议用 macro 生成。

不可变查询 trait 可以设计成：

```rust
pub trait QueryTuple {
    type Item<'a>
    where
        Self: 'a;

    fn ids(register: &mut ComponentRegister) -> Vec<ComponentId>;

    unsafe fn fetch<'a>(
        table: &'a Table,
        columns: &[usize],
        row: usize,
    ) -> Self::Item<'a>;
}
```

单组件示例：

```rust
impl<A: Component> QueryTuple for A {
    type Item<'a> = &'a A where A: 'a;

    fn ids(register: &mut ComponentRegister) -> Vec<ComponentId> {
        vec![register.get::<A>().0]
    }

    unsafe fn fetch<'a>(
        table: &'a Table,
        columns: &[usize],
        row: usize,
    ) -> Self::Item<'a> {
        &table.component_slice::<A>(columns[0])[row]
    }
}
```

两个组件示例：

```rust
impl<A: Component, B: Component> QueryTuple for (A, B) {
    type Item<'a> = (&'a A, &'a B)
    where
        A: 'a,
        B: 'a;

    fn ids(register: &mut ComponentRegister) -> Vec<ComponentId> {
        vec![
            register.get::<A>().0,
            register.get::<B>().0,
        ]
    }

    unsafe fn fetch<'a>(
        table: &'a Table,
        columns: &[usize],
        row: usize,
    ) -> Self::Item<'a> {
        (
            &table.component_slice::<A>(columns[0])[row],
            &table.component_slice::<B>(columns[1])[row],
        )
    }
}
```

然后 `Scene` 可以提供：

```rust
impl Scene {
    pub fn for_each_components<Q, F>(
        &self,
        component_register: &mut ComponentRegister,
        mut f: F,
    )
    where
        Q: QueryTuple,
        for<'a> F: FnMut(Q::Item<'a>),
    {
        let ids = Q::ids(component_register);

        for table in self.table_graph.graph.node_weights() {
            if !table.contains_all(&ids) {
                continue;
            }

            let columns = ids
                .iter()
                .map(|id| table.column_index(id).unwrap())
                .collect::<Vec<_>>();

            for row in 0..table.len() {
                unsafe {
                    f(Q::fetch(table, &columns, row));
                }
            }
        }
    }
}
```

使用方式：

```rust
scene.for_each_components::<(Transform, Velocity), _>(
    &mut component_register,
    |(transform, velocity)| {
        println!("{:?} {:?}", transform, velocity);
    },
);
```

单组件：

```rust
scene.for_each_components::<Transform, _>(
    &mut component_register,
    |transform| {
        println!("{:?}", transform);
    },
);
```

三个组件：

```rust
scene.for_each_components::<(Transform, Velocity, Health), _>(
    &mut component_register,
    |(transform, velocity, health)| {
        // system logic
    },
);
```

### 可变查询

可变查询要更谨慎，因为 `(&mut A, &mut B)` 必须保证 `A` 和 `B` 不是同一个组件类型，否则会制造别名可变引用。

可以单独做一个 trait：

```rust
pub trait QueryTupleMut {
    type Item<'a>
    where
        Self: 'a;

    fn ids(register: &mut ComponentRegister) -> Vec<ComponentId>;

    unsafe fn fetch_mut<'a>(
        table: *mut Table,
        columns: &[usize],
        row: usize,
    ) -> Self::Item<'a>;
}
```

两个组件示意：

```rust
impl<A: Component, B: Component> QueryTupleMut for (A, B) {
    type Item<'a> = (&'a mut A, &'a mut B)
    where
        A: 'a,
        B: 'a;

    fn ids(register: &mut ComponentRegister) -> Vec<ComponentId> {
        vec![
            register.get::<A>().0,
            register.get::<B>().0,
        ]
    }

    unsafe fn fetch_mut<'a>(
        table: *mut Table,
        columns: &[usize],
        row: usize,
    ) -> Self::Item<'a> {
        let a = &mut (*table).component_slice_mut::<A>(columns[0])[row] as *mut A;
        let b = &mut (*table).component_slice_mut::<B>(columns[1])[row] as *mut B;

        (&mut *a, &mut *b)
    }
}
```

`Scene` 侧：

```rust
impl Scene {
    pub fn for_each_components_mut<Q, F>(
        &mut self,
        component_register: &mut ComponentRegister,
        mut f: F,
    )
    where
        Q: QueryTupleMut,
        for<'a> F: FnMut(Q::Item<'a>),
    {
        let ids = Q::ids(component_register);

        // 必须检查重复组件类型，避免 (&mut A, &mut A)
        debug_assert!({
            let mut sorted = ids.clone();
            sorted.sort_by_key(|id| id.get_idx());
            sorted.dedup();
            sorted.len() == ids.len()
        });

        for table in self.table_graph.graph.node_weights_mut() {
            if !table.contains_all(&ids) {
                continue;
            }

            let columns = ids
                .iter()
                .map(|id| table.column_index(id).unwrap())
                .collect::<Vec<_>>();

            let table_ptr = table as *mut Table;

            for row in 0..table.len() {
                unsafe {
                    f(Q::fetch_mut(table_ptr, &columns, row));
                }
            }
        }
    }
}
```

使用：

```rust
scene.for_each_components_mut::<(Transform, Velocity), _>(
    &mut component_register,
    |(transform, velocity)| {
        transform.position += velocity.value;
    },
);
```

### 宏生成任意数量组合

可以用宏减少重复：

```rust
macro_rules! impl_query_tuple {
    ($($name:ident),+) => {
        impl<$($name: Component),+> QueryTuple for ($($name,)+) {
            type Item<'a> = ($(&'a $name,)+)
            where
                $($name: 'a,)+;

            fn ids(register: &mut ComponentRegister) -> Vec<ComponentId> {
                vec![
                    $(register.get::<$name>().0,)+
                ]
            }

            unsafe fn fetch<'a>(
                table: &'a Table,
                columns: &[usize],
                row: usize,
            ) -> Self::Item<'a> {
                let mut index = 0;

                (
                    $(
                        {
                            let column = columns[index];
                            index += 1;
                            &table.component_slice::<$name>(column)[row]
                        },
                    )+
                )
            }
        }
    };
}

impl_query_tuple!(A, B);
impl_query_tuple!(A, B, C);
impl_query_tuple!(A, B, C, D);
impl_query_tuple!(A, B, C, D, E);
impl_query_tuple!(A, B, C, D, E, F);
```

这样就和你当前 `ComponentsTuple` 的思路一致，只是不用手写每个 impl。

严格来说这不是数学意义上的“无限泛型数量”，但在 Rust ECS 里这是最常见做法。Bevy、hecs 这类 ECS 也基本都是给 tuple query 展开到一个实际最大数量。

### 还需要注意的现有问题

你的 `Scene::create_entity` 里新建 table 后没有把它插入 `components_id_to_table`：

```rust
self.table_graph.graph.add_node(Table::new(...))
```

这里应该保存返回的 `NodeIndex`：

```rust
let node = self.table_graph.graph.add_node(Table::new(...));
self.components_id_to_table.insert(component_id_metas.0.clone(), node);
node
```

否则同一种组件组合每次创建实体都会新建一张 table，查询虽然还能遍历到，但 archetype 会碎掉，性能和语义都会不对。

另外 `Table::new` 里这里也值得检查：

```rust
let components_table = ComponentTable::new(component_metas, colum_capacity);
```

`ComponentTable::new` 的第二个参数实际被当作 row capacity 使用，但这里传的是组件列数量。更合理应该是：

```rust
let components_table = ComponentTable::new(component_metas, row_capacity);
```

### 推荐落地顺序

1. 先补 `ComponentTable` 的 `get_components/get_components_mut/len`。
2. 再补 `Table` 的 `contains_all/column_index/component_slice`。
3. 然后实现不可变 `QueryTuple` 和 `Scene::for_each_components`。
4. 稳定后再加可变 `QueryTupleMut`。
5. 最后用宏统一生成 `(A, B, C...)` 的查询 impl。

这样你的 `Scene` 就可以自然支持：

```rust
scene.for_each_components::<Transform, _>(...);
scene.for_each_components::<(Transform, Velocity), _>(...);
scene.for_each_components::<(Transform, Mesh, Material), _>(...);
```

并且不会破坏你当前 archetype/table 的整体架构。

## 对话 2

### 用户

有办法在查询时根据 <A, B, C> 泛型的 ComponentId 排序顺序访问吗？

### 助手

可以，但要区分两件事：

1. **内部按 `ComponentId` 排序访问列**：可以，而且推荐。
2. **返回值类型也按排序后变成 `(&B, &A, &C)`**：基本不现实，因为 Rust 的 tuple 类型顺序是编译期固定的，`(A, B, C)` 不能根据运行时 `ComponentId` 排序动态变成 `(B, A, C)`。

最推荐的方案是：**查询内部按排序后的 `ComponentId` 找列、访问列；对外仍然返回泛型声明顺序 `(A, B, C)`。**

例如：

```rust
pub trait QueryTuple {
    type Item<'a>
    where
        Self: 'a;

    fn sorted_ids(register: &mut ComponentRegister) -> Vec<ComponentId>;

    unsafe fn fetch<'a>(
        table: &'a Table,
        row: usize,
        component_ids: &[ComponentId],
    ) -> Self::Item<'a>;
}
```

对于 `(A, B, C)`：

```rust
impl<A: Component, B: Component, C: Component> QueryTuple for (A, B, C) {
    type Item<'a> = (&'a A, &'a B, &'a C)
    where
        A: 'a,
        B: 'a,
        C: 'a;

    fn sorted_ids(register: &mut ComponentRegister) -> Vec<ComponentId> {
        let a = register.get::<A>().0;
        let b = register.get::<B>().0;
        let c = register.get::<C>().0;

        let mut ids = vec![a, b, c];
        ids.sort_by_key(|id| id.get_idx());
        ids
    }

    unsafe fn fetch<'a>(
        table: &'a Table,
        row: usize,
        _component_ids: &[ComponentId],
    ) -> Self::Item<'a> {
        let a_id = table.component_id::<A>();
        let b_id = table.component_id::<B>();
        let c_id = table.component_id::<C>();

        let a_col = table.column_index(&a_id).unwrap();
        let b_col = table.column_index(&b_id).unwrap();
        let c_col = table.column_index(&c_id).unwrap();

        (
            &table.component_slice::<A>(a_col)[row],
            &table.component_slice::<B>(b_col)[row],
            &table.component_slice::<C>(c_col)[row],
        )
    }
}
```

不过上面这个版本里 `table.component_id::<A>()` 不一定适合你的结构，因为 `ComponentId` 来自 `ComponentRegister`。所以更实际的是在 `fetch` 之前缓存每个类型对应的列索引：

```rust
struct QueryColumns {
    sorted_ids: Vec<ComponentId>,
    columns: Vec<usize>,
}
```

查询时：

```rust
let mut ids = Q::sorted_ids(component_register);

for table in self.table_graph.graph.node_weights() {
    if !table.contains_all(&ids) {
        continue;
    }

    let columns = ids
        .iter()
        .map(|id| table.column_index(id).unwrap())
        .collect::<Vec<_>>();

    for row in 0..table.len() {
        unsafe {
            // 这里 columns 的顺序就是 ComponentId 排序后的顺序
        }
    }
}
```

如果你想让 `fetch` 也按照排序顺序拿列，可以让每个 impl 先计算“某个类型在 sorted ids 中的位置”：

```rust
let a_pos = ids.iter().position(|id| id == &a_id).unwrap();
let b_pos = ids.iter().position(|id| id == &b_id).unwrap();
let c_pos = ids.iter().position(|id| id == &c_id).unwrap();

let a_col = columns[a_pos];
let b_col = columns[b_pos];
let c_col = columns[c_pos];
```

这样内部列访问顺序是：

```text
sorted ComponentId -> column index -> component slice
```

但最终返回仍然是：

```rust
(&A, &B, &C)
```

如果你真的希望回调参数也按排序后顺序变化，比如用户写 `<A, B, C>`，但实际返回 `(&B, &A, &C)`，那在 Rust 类型系统里不适合作为静态 tuple API。更合适的形式是动态访问：

```rust
for component in query_row.components_sorted_by_id() {
    // component 是 dyn Any / raw pointer / ComponentRef
}
```

但这样会损失静态类型优势，不太适合你的 ECS 当前设计。

所以结论是：**可以在查询内部完全按 `ComponentId` 排序访问，这和你的 Table 列排序模型非常契合；但对外 API 最好保持泛型声明顺序返回，避免把运行时排序塞进静态 tuple 类型系统。**

## 对话 3

### 用户

把我们的对话完整的保存入 docs/ai 目录下

### 助手

已执行：将本次关于 ECS Scene 泛型组件查询的对话保存为本文档。
