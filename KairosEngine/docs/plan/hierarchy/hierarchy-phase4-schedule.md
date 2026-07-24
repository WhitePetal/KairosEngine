# Hierarchy 面板设计案 — Phase 4：功能排期

> **状态**：进行中
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./hierarchy-phase1-requirements.md) | [Phase 2 功能分析](./hierarchy-phase2-features.md) | [Phase 3 技术选型](./hierarchy-phase3-tech.md)

## 4.1 排期总览

```
Week 1-2: F0 ECS Change Detection
Week 2-3: F1 Transform 层级系统
Week 3-4: F2 .world 序列化  ←→  并行：D-007 inventory 注册 + EntityComponentInspector trait
Week 4-5: F3 Hierarchy Window UI（核心交互）
Week 5-6: F4 Entity Inspector
Week 6:   F5 EditorMode 状态机
Week 7:   F6 Project Window 集成
```

---

## 4.2 详细任务拆分

### Phase 4a：基础建设（Week 1-3，串行）

#### F0：ECS Change Detection

> 依赖：无
> 被依赖：F1

| ID | 任务 | 说明 |
|----|------|------|
| F0.1 | Component 列增加 tick 存储 | 每个 Component 列（table column）增加 `Vec<u32>` ticks，与 data 同 index |
| F0.2 | 写入路径增加 tick 标记 | `World::insert`、`World::remove`、`World::exchange`、`QueryMut` deref 等写路径：`ticks[row] += 1` |
| F0.3 | 新增 `Changed<T>` 查询过滤器 | query 时比较当前 tick 与记录的上次 tick，返回变化的行 |
| F0.4 | 新增 `Added<T>` 查询过滤器 | 本帧 `World::insert` 的组件，tick 从 0→1 视为 Added |
| F0.5 | 查询迭代器记录 last_seen_ticks | 每个 System/查询调用方维护自己的 tick 快照，消费后更新 |
| F0.6 | 编写单元测试 | 覆盖：insert→Added 检测、修改→Changed 检测、未修改→不变 |

**产出**：`Changed<T>` / `Added<T>` 查询过滤器可用。

---

#### F1：Transform 层级系统

> 依赖：F0
> 被依赖：F2、F3、F5

| ID | 任务 | 说明 |
|----|------|------|
| F1.1 | `Transform` → `LocalTransform` 重命名 | 改名 + `get_local_to_world()` → `compute_local_matrix()`。全项目 grep 替换所有引用 |
| F1.2 | 新增 `Parent` 组件 | `Parent(Entity)`，根节点无此组件 |
| F1.3 | 新增 `Children` 组件 | `Children(SmallVec<[Entity; 8]>)`，双向维护 |
| F1.4 | 新增 `GlobalTransform` 组件 | `GlobalTransform(float4x4)`，缓存世界矩阵 |
| F1.5 | `World::set_parent(entity, parent)` | 含循环引用检测（沿 Parent 链向上查）；自动维护 Children 双向关系 |
| F1.6 | 实现 `TransformPropagateSystem` | 查询 `Changed<LocalTransform>` + `Changed<Parent>` 的 Entity，自顶向下递归计算 GlobalTransform，利用 F0 跳过无变化子树 |
| F1.7 | `World::despawn` 级联 | 递归 despawn 所有 Children（先递归再标记 Dead + 移除组件） |
| F1.8 | 编写单元测试 | 覆盖：set_parent / get_children / 循环检测拒绝 / 传播计算 / 级联 despawn |

**产出**：Parent/Children/LocalTransform/GlobalTransform 组件 + 传播系统可用。

---

### Phase 4b：数据层（Week 3-4，F1 完成后可并行）

#### F2：.world 序列化

> 依赖：F1（需知道可序列化的 Component 类型集合）
> 被依赖：F3、F5

| ID | 任务 | 说明 |
|----|------|------|
| F2.1 | 定义 `SerializedWorld` struct | meta（version, next_id）+ entities（HashMap<String, SerializedEntity>）+ extensions（`#[serde(flatten)]` 兜底） |
| F2.2 | 定义 `SerializedEntity` struct | name + parent（String，路径 key）+ components（`HashMap<String, TomlValue>`，section 名→值） |
| F2.3 | 实现 `World → SerializedWorld` 序列化 | 遍历 Entity：读取 Name→名称、Parent→路径、Children→构建树；每个 Component 调用 `EntityComponentInspector::try_serialize` |
| F2.4 | 实现 `SerializedWorld → World` 反序列化 | 两步加载：Pass1 创建所有 Entity + insert 所有 Component；Pass2 统一 set_parent |
| F2.5 | Cmd+S 保存处理 | `ui.rs` 中新增 `Message::SaveWorld`；快捷键 Ctrl+S 触发；SceneWorld → TOML → 写文件 |
| F2.6 | 编辑器启动加载 | `KairosEditorRuntime::new()` 中从 `res/worlds/World.world` 加载到 SceneWorld |
| F2.7 | 创建默认 `res/worlds/World.world` | 含最小 meta 的合法空 .world 文件，随项目提交 |
| F2.8 | Round-trip 测试 | SceneWorld → 序列化 → 文件 → 反序列化 → 新 World → 验证 Entity/Component/Parent 一致性 |

#### 并行：D-007 `EntityComponentInspector` trait + `inventory` 注册

> 依赖：无（纯 trait 定义 + proc-macro 修改）
> 被依赖：F2.3/F2.4、F4

| ID | 任务 | 说明 |
|----|------|------|
| D7.1 | 定义 `EntityComponentInspector<T>` trait | draw / component_name / toml_section_name / try_serialize / try_deserialize |
| D7.2 | 定义 `ComponentMeta` + `inventory::collect!` | registry.rs：全局注册表 |
| D7.3 | 修改 `#[derive(Component)]` proc-macro | 展开时生成：编译期约束（impl EntityComponentInspector 必须存在）+ `inventory::submit!` 注册代码 |

**产出**：trait + 注册机制就绪。

---

### Phase 4c：UI 层（Week 4-6）

#### F3：Hierarchy Window UI

> 依赖：F1（Parent/Children/Name 组件）、F2（加载 .world）
> 被依赖：F4

| ID | 任务 | 说明 |
|----|------|------|
| F3.1 | 获取活跃 World 中的根 Entity | 查询所有没有 `Parent` 组件（或 Parent 指向无效 Entity）的 Entity，构成顶层节点列表 |
| F3.2 | 递归树形渲染 | 渲染 Entity 名称（Name 组件，fallback 到 ID）；通过 Children 组件递归渲染子节点 |
| F3.3 | 折叠/展开状态 + 阶段 2 性能 | 记录每个节点的展开状态；折叠的子树不执行递归遍历（跳过 Children 渲染） |
| F3.4 | 单击选中 | 点击节点 → 更新内部选中状态 → `messager.send(Message::SelectHierarchyEntity(Some(entity)))` |
| F3.5 | 多选 | Ctrl+Click 追加/取消、Shift+Click 范围选 |
| F3.6 | 右键菜单 | 弹出 ContextMenu：Create Empty Entity / Delete / Duplicate / Rename / Add Component（子菜单） |
| F3.7 | 拖拽重定父 | 拖拽 Entity A 到 Entity B → `World::set_parent(A, B)`；视觉反馈（高亮潜在父节点） |
| F3.8 | 重命名 | F2 或慢双击进入编辑模式，修改 Name 组件；编辑器同步更新 .world key（路径式命名） |
| F3.9 | Delete 删除 | Delete 键 → `World::despawn(entity)` → 级联删除子节点 |
| F3.10 | 复制/粘贴/复制 | Ctrl+C 深拷贝（Entity + 所有 Component）→ Ctrl+V 粘贴为当前选中节点的兄弟 |
| F3.11 | 搜索/过滤 | 输入框，按名称过滤；匹配节点高亮，不匹配的 dim 但保留树结构 |
| F3.12 | 快捷键 | Delete 删除、F2 重命名、Ctrl+D 复制、Ctrl+C/V 复制粘贴 |

**产出**：可交互的 Hierarchy 面板。

---

#### F4：Entity Inspector

> 依赖：F3（选中联动）、D7（EntityComponentInspector trait + registry）
> 被依赖：无

| ID | 任务 | 说明 |
|----|------|------|
| F4.1 | 扩展 `InspectorWindow` 上下文路由 | `InspectorContext` enum 增加 `Entity(Entity)` variant；`handle()` 中响应 `Message::SelectHierarchyEntity` |
| F4.2 | Component 列表渲染 | 遍历 `inventory::iter::<ComponentMeta>()`，对活跃 World 中 selected Entity 上存在的 Component 逐一渲染 |
| F4.3 | 每个 Component 用 CollapsingHeader 包裹 | 折叠/展开，标题显示 component_name() |
| F4.4 | "Add Component" 按钮 | 下拉列出所有 ComponentMeta 中当前 Entity 不存在的类型 → 点击 insert |
| F4.5 | "Remove Component" 按钮 | 每个 Component section 右上角 ⋮ 菜单 → Remove → `World::remove::<T>(entity)` |
| F4.6 | Entity 名称编辑 | Inspector 顶部显示 Name，可直接编辑（修改 Name.0） |
| F4.7 | 实现 `LocalTransformInspector` | 第一个 EntityComponentInspector 实现：position/rotation/scale 的 DragValue 编辑 |
| F4.8 | 实现 `CameraInspector` | fov（滑条 1-179）、near、far 编辑 |
| F4.9 | Play 模式支持 | 根据 EditorMode 获取活跃 World（GameWorld），允许临时修改 |

**产出**：选中 Entity 后在 Inspector 中展示所有 Component 并可编辑。

---

### Phase 4d：集成层（Week 6-7）

#### F5：EditorMode 状态机

> 依赖：F2（Play 时从文件加载 GameWorld）
> 被依赖：F3/F4（面板切换数据源）

| ID | 任务 | 说明 |
|----|------|------|
| F5.1 | 新增 `EditorMode` 枚举 | `enum EditorMode { Edit, Play }` |
| F5.2 | 新增 `WorldManager` struct | `{ scene_world: World, game_world: Option<World>, mode: EditorMode }` + `active_world()` / `active_world_mut()` |
| F5.3 | 改造 `KairosEditorRuntime` | 替换 `engine.world` → `WorldManager` |
| F5.4 | Play 流程 | 1) 检查 SceneWorld 未保存修改 2) 从 .world 文件加载 GameWorld 3) 切换 mode→Play 4) 面板自动指向 GameWorld |
| F5.5 | Stop 流程 | 1) 停止游戏 Systems 2) 销毁 GameWorld 3) 切换 mode→Edit 4) 面板切回 SceneWorld |
| F5.6 | Toolbar Play/Stop 按钮 | 连接现有 ToolBar UI，Play（▶）和 Stop（■）按钮 |
| F5.7 | 面板集成 | Hierarchy/Scene/Inspector drawer 通过 `WorldManager::active_world()` 获取当前数据源 |

---

#### F6：Project Window 集成

> 依赖：F2（AssetKind::World + 序列化）
> 被依赖：无

| ID | 任务 | 说明 |
|----|------|------|
| F6.1 | 扩展 `AssetKind` 枚举 | 新增 `World` variant |
| F6.2 | `.world` 文件映射 | 文件后缀 `.world` → `AssetKind::World` |
| F6.3 | 双击打开 | Project Window 双击 `.world` → 检查未保存 → 确认 → 加载到 SceneWorld |
| F6.4 | 右键新建 | Project Window 菜单 → "Create World" → 在 `res/worlds/` 下创建空 .world |
| F6.5 | 拖拽到 Hierarchy | 拖拽 .world → Hierarchy → 加载该场景（同双击） |
| F6.6 | World 文件图标 | `global_styles` 中注册 .world 图标 |

---

## 4.3 依赖关系图

```
Week │  1  │  2  │  3  │  4  │  5  │  6  │  7  │
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F0   │█████│█████│     │     │     │     │     │
F1   │     │█████│█████│     │     │     │     │
F2   │     │     │█████│█████│     │     │     │
D7   │     │     │█████│█████│     │     │     │  ← 并行
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F3   │     │     │     │█████│█████│     │     │
F4   │     │     │     │     │█████│█████│     │
─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
F5   │     │     │     │     │     │█████│     │
F6   │     │     │     │     │     │     │█████│
```

- **F2 和 D7 可并行**：序列化格式设计与 Inspector trait 定义互不依赖
- **F5 和 F6 可并行**：状态机与 Project Window 集成互不依赖

---

## 4.4 检查点

| 检查点 | 完成标记 | 验证方式 |
|--------|----------|----------|
| CP1: F0+F1 完成 | Change Detection + Transform 层级可用 | `cargo test` — 覆盖 Changed/Added 查询 + set_parent/传播/despawn |
| CP2: F2+D7 完成 | .world 可加载/保存 | `cargo test` — round-trip 测试通过；编辑器启动加载默认 World.world |
| CP3: F3 完成 | Hierarchy 面板可交互 | 手动验证：打开编辑器 → Hierarchy 显示 Entity 树 → 选中/右键/拖拽正常 |
| CP4: F4 完成 | Inspector 显示组件 | 手动验证：选中 Entity → Inspector 显示 LocalTransform/Camera 等组件字段 |
| CP5: F5 完成 | Play/Stop 工作 | 手动验证：Play → 面板切换到 GameWorld → Stop → 恢复 SceneWorld |
| CP6: F6 完成 | .world 文件管理 | 手动验证：Project Window 中 .world 文件可双击/右键新建/拖拽 |
