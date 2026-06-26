# 空间 Cell ID 分区：ECS Table 的第二维分区键

> 日期：2026-06-14  
> 状态：设计方案讨论，非最终实现规范  
> 范围：在 KairosEngine 自研 ECS 中引入空间 Cell ID 作为 Table 分区键，使空间局部性与内存局部性对齐，并分析此方案与八叉树 + ECS 组合查询的优劣。

## 背景

KairosEngine 当前 ECS 使用 archetype/table graph 架构（类似 Flecs），`TableKey = ComponentTupleKey`（即按组件类型组合分区）。`World::spawn((A, B, C))` 创建一个带 (A,B,C) 组件的实体，被放入对应的 Table 中。

空间模块当前有：
- `TransformComponent`：实体位置/旋转/缩放
- `AABB`：轴对齐包围盒
- `Octree<Entity>`：泛型八叉树，存储 `(Entity, AABB)` 条目，支持空间范围查询

需要解决的核心矛盾：**八叉树返回空间有序的 Entity 列表，但 ECS Table 按 archetype（组件组合）组织数据，导致空间相邻的实体在内存中分散在不同 Table 的不同行**——缓存不友好。

## 方案核心思想

**把空间 Cell ID 提升为 ECS Table 的第二维分区键**，在实体 spawn 时就确定其空间归属：

```
当前:  TableKey = f(ComponentTypeSet)
       e.g., Table#42 = {Transform, TerrainPatch, TerrainLODState}

提议:  TableKey = f(CellId, ComponentTypeSet)
       e.g., Table#42 = (CellId=7, {Transform, TerrainPatch, TerrainLODState})
       e.g., Table#0  = (CellId=0, {AudioConfig, GlobalSettings})  ← 无空间归属
```

CellId(0) 预留给全局组件（没有 Transform 的实体，如 `AudioSettings`、`RenderSettings`），表示"无空间划分"。

## 工业界对应：UE5 World Partition + Mass Entity

UE5 的 World Partition 本质上就是把空间哈希网格的 Cell 作为 ECS（Mass Entity）实体的分区键：

```
World Partition Grid (空间哈希)
├── Cell (0,0,0)
│   ├── ArchetypeTable {Transform, StaticMesh}     ← 12 个实体
│   ├── ArchetypeTable {Transform, LightProbe}     ← 3 个实体
│   └── ArchetypeTable {Transform, LandscapeTile}  ← 4 个实体
├── Cell (1,0,0)
│   ├── ArchetypeTable {Transform, StaticMesh}     ← 8 个实体
│   └── ArchetypeTable {Transform, NPC}            ← 2 个实体
└── CellId(0) — Global
    ├── ArchetypeTable {WorldSettings}
    ├── ArchetypeTable {AudioListener}
    └── ArchetypeTable {RenderSettings}
```

每个 Cell 内部独立跑 System，天然并行。Mass Entity 正是用 "spatial hash + archetype" 作为实体的分桶键。

## 优势分析

### 1. 空间局部性 = 内存局部性（核心价值）

```
无空间分区:
  Table{Transform, TerrainPatch}: [Patch@(100,0), Patch@(5,0), Patch@(900,0), ...]
                                   ↑ 空间分散 → 遍历时随机访问 position 字段 → cache miss

有 Cell 分区:
  Cell#7 Table{Transform, TerrainPatch}:  [Patch@(100,0), Patch@(110,0), Patch@(105,0)]
                                            ↑ 空间聚集 → 遍历时顺序访问 → cache 友好
```

不需要八叉树 + ECS 两步查询。Cell 内的 Table 本身就是空间聚集的，直接迭代 Table 列就是空间顺序访问。这和 `query_component_in_radius_cache_friendly` 中「按 Table 分组 + 按 row 排序」达到的效果一致，但不需要运行时的分组和排序开销——**在插入时就保证了**。

### 2. 流式加载天然对齐

```
加载 Cell(7,3,1):
  → 反序列化 Table {Transform, TerrainPatch}     → 直接 insert_batch 到 TableGraph
  → 反序列化 Table {Transform, StaticMesh}       → 直接 insert_batch 到 TableGraph
  → GPU 上传 mesh 数据

卸载 Cell(7,3,1):
  → 遍历 Cell(7,3,1) 的所有 Table
  → despawn 所有实体 / drop 整个 Table
  → 释放 GPU 资源
```

一个 Cell = 一组 Table = 一个加载/卸载单元。不需要额外的空间索引来追踪哪些实体属于哪个 Cell——这个信息已经编码在 Table 的分区键里。

### 3. 并行无锁

```
Thread 0: process Cell(7,3,1) → 只访问 Table(Cell=7,3,1, ...)
Thread 1: process Cell(2,5,0) → 只访问 Table(Cell=2,5,0, ...)
Thread 2: process Cell(9,1,3) → 只访问 Table(Cell=9,1,3, ...)
```

不同 Cell 的 Table 完全独立，零竞争，不需要锁。现有的 `AtomicBorrow` 机制甚至不需要跨 Cell 协调。

### 4. Query 语法自然

```rust
// 查询某个 Cell 内的所有地形 Patch
world.query_in_cell::<(&TransformComponent, &TerrainPatchComponent)>(CellId(7));

// 内部实现：TableGraph 中 key=(CellId(7), {Transform, TerrainPatch}) 的 Table
// → 直接迭代，零过滤开销

// 查询全局组件（无空间归属）
world.query::<&AudioSettings>();  // 隐含 CellId(0)
```

### 5. 八叉树仍然有用

八叉树提供 "哪些 Cell 在范围内" 的粗筛选，Cell 内 Table 迭代提供细粒度的顺序访问。两者互补：

```
当前:  八叉树 → 实体列表 → 逐个 world.get（随机 Table 访问）
提议:  八叉树 → Cell 列表 → 每个 Cell 内 Table 迭代（顺序 Table 访问）
```

第二步从"逐实体随机访问"变成了"逐 Table 顺序迭代"，这才是真正的缓存友好。

## 劣势与风险

### 1. Table 数量爆炸（最大风险）

```
当前: N 种组件组合 → 最多 N 个 Table
提议: C 个 Cell × N 种组件组合 → 最多 C×N 个 Table
```

场景估算：
- 组件组合种类 N ≈ 20-50
- Cell 数量 C：64³ 网格 = 262144 个 Cell
- 理论最大：262144 × 50 = 1310 万个 Table（不可能全部分配）

缓解策略：

| 策略 | 说明 |
|------|------|
| **稀疏 Cell** | 只在有实体的位置创建 Cell。大多数 Cell 为空，不占 Table |
| **懒创建** | `HashMap<(CellId, TypeSet), Table>` 只在第一个实体 spawn 时创建 Table |
| **层级 Cell** | 远处用大 Cell（粗粒度 CellId），近处用小 Cell（细粒度 CellId） |
| **Cell 合并** | 实体数 < 阈值的相邻 Cell 共享 Table（回退到无空间分区） |

实际场景中，有实体的 Cell 通常只有几百到几千个。以 4km × 4km 世界 + 64m Cell 为例：62×62 ≈ 3800 Cell × 5 种组件组合 ≈ **19000 个 Table**，在现有 `TableGraph` 中完全可管理。

### 2. 跨 Cell 移动的代价

```
当前：实体移动 = 更新八叉树（O(log N)）+ Transform 位置更新（O(1)）
提议：实体移动 = 从旧 Table 移除 + 插入新 Table（O(component_count)，涉及列数据搬迁）
```

适合/不适合的实体类型：

| 实体类型 | 适合？ | 原因 |
|----------|--------|------|
| 静态地形 Patch | ✅ | 从不移动 |
| Light Probe Volume | ✅ | 从不移动 |
| 建筑物 / Static Mesh | ✅ | 偶尔移动 |
| 慢速 NPC | ⚠️ | 偶尔跨 Cell |
| 子弹/粒子 | ❌ | 每帧跨 Cell → 留在 CellId(0) 或用原始八叉树方案 |

### 3. 跨 Cell 查询需额外的粗筛选

查询半径 R 内的所有实体需要先用八叉树/空间哈希找出被覆盖的 Cell ID 列表。

## 方案对比总览

```
方案                         空间精度    内存局部性   流式加载    移动代价    实现复杂度
───────────────────────────────────────────────────────────────────────────────────────
A) 纯 Archetype (当前)       无          ❌           ❌          ✅          ✅ 已完成
B) 八叉树 + world.get        精确        ❌ (随机行)   ❌          ✅          ✅ 已完成
C) 八叉树 + TableColum分组   精确        ⚠️ (运行时排序) ❌       ✅          ⚠️ 已完成
D) CellId分区 (提议)         粗粒度      ✅ (天然顺序)  ✅          ❌          ⚠️ 需改ECS
E) 混合 D+B (Cell + 八叉树)  精确        ✅             ✅          ⚠️          ❌ 工程量大
```

方案 D 的核心权衡：用「实体移动代价 ↑」和「Table 数量 ↑」换取「空间访问的缓存友好性 ↑↑」和「流式加载的简洁性 ↑↑」。

## 适用判断

**对于 KairosEngine 的地形 LOD + Light Probe 流式加载用例：非常值得。** 理由：

1. 地形 Patch 和 Light Probe 从不移动——移动代价这个最大劣势直接归零
2. 流式加载是核心需求——Cell = 加载单元映射极其自然
3. Table 数量可控——实际场景中远低于理论上限
4. CellId(0) 为全局组件保留——不受空间分区影响
5. 八叉树不消失——提供 Cell 级别的粗筛选

**不建议的场景**：
- 有大量快速移动实体的游戏（弹幕射击等）——Cell 切换代价太高
- 实体类型极度多样化的场景——Table 数量可能失控

## 落地路线（草案）

不改代码，仅设计思路：

```
Step 1: 定义 CellId
  0 = Global (无空间归属)
  1..N = 空间 Cell（映射到八叉树节点或空间哈希网格）

Step 2: 修改 TableKey
  当前: ComponentTupleKey (TypeId 的哈希)
  改为: (CellId, ComponentTupleKey)
  
  tuple_to_table: HashMap<ComponentTupleKey, NodeIndex>
  → tuple_to_table: HashMap<(CellId, ComponentTupleKey), NodeIndex>

Step 3: spawn 时传入 CellId
  world.spawn_in_cell(CellId(7), (transform, terrain_patch))
  → 内部查 key=(CellId(7), TypeId::of::<(Transform, TerrainPatch)>())
  → 找到或创建 Table，插入该行

Step 4: Query 支持 Cell 过滤
  // 方式 A: 显式 Cell ID
  world.query_in_cell::<&TerrainPatch>(CellId(7));

  // 方式 B: 多个 Cell
  world.query_in_cells::<&TerrainPatch>(&[CellId(7), CellId(8), CellId(9)]);

  // 方式 C: 无过滤 = CellId(0) 或全部 Cell（取决于语义）

Step 5: 与八叉树整合
  // System 代码示例:
  let nearby_cells = octree.query_cells_in_radius(&camera_pos, 500.0);
  for cell_id in nearby_cells {
      for (transform, patch_state) in world.query_in_cell::<(&Transform, &mut TerrainLODState)>(cell_id) {
          let dist = (transform.position - camera_pos).len();
          patch_state.target_lod = compute_lod(dist);
      }
  }
```

## 八叉树 + ECS 组合查询的三种模式（当前已实现）

作为本方案的配套基础设施，当前项目中已有以下空间查询模式：

### 方式一：手写两步法（最灵活）

```rust
// 第一步：八叉树粗筛选
let candidates = spatial_index.query_entities_in_aabb(&query_aabb);

// 第二步：逐个从 ECS World 取组件
for entity in candidates {
    if let Ok(patch) = world.get::<&TerrainPatchComponent>(entity) {
        // 处理
    }
}
```

### 方式二：封装好的组合函数

```rust
// 八叉树筛选 + ECS 取值一步完成
let results = query_component_in_radius::<TerrainPatchComponent>(
    &spatial_index, &world, &camera_pos, 500.0,
);
for (entity, patch) in &results {
    // Ref deref 到 &TerrainPatchComponent
}
```

### 方式三：缓存友好的批量查询（`query_component_in_radius_cache_friendly`）

按 Table 分组 → row_index 排序 → `TableColum` 顺序访问列切片。相比逐实体 `world.get`：
- SparseSet 查找：N 次 → 1 次/Table
- TypeId 二分：N×2 次 → 1 次/Table
- AtomicBorrow：N×2 次原子操作 → 1 次/Table
- 列数据访问：随机行 → 排序后顺序行（预取友好）

### `query_disjoint_mut` 为什么不适用

`world.query_disjoint_mut` 限制：
1. `const N: usize` 编译期常量——八叉树返回动态数量
2. `&mut World` 独占借用——阻止其他 System 并发
3. `assert_distinct` O(N²) 去重校验不必要
4. 内部同样逐实体随机访问——零缓存优化

## 相关文件

- `kairos_engine/src/spatial/aabb.rs` — AABB 包围盒
- `kairos_engine/src/spatial/octree.rs` — 泛型八叉树 `Octree<Entity>`
- `kairos_engine/src/spatial/spatial_index.rs` — 八叉树与 ECS World 的桥接层（`SpatialIndex`、组合查询函数）
- `kairos_engine/src/ecs/world.rs` — ECS World 实现（`query_disjoint_mut` 等）
- `kairos_engine/src/ecs/table.rs` — Table SoA 列存储实现（`TableColum`）
- `kairos_engine/src/ecs/table_graph.rs` — TableGraph（`insert_edges`、`remove_edges`）
