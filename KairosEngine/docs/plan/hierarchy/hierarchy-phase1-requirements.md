# Hierarchy 面板设计案 — Phase 1：需求分析

> **状态**：✅ 完成
> **创建日期**：2026-07-24

## 1.1 背景

Kairos Editor 当前有 `hierarchy_window.rs` 占位（显示 "TODO: Hierarchy"），需要实现完整的 Hierarchy 面板。

当前编辑器的关键已有系统：

- **ECS**：自定义实现，`World` / `Entity` / `Component` trait，支持 table-based 存储
- **Project Window**：含 `hierarchy_panel.rs`（项目目录树），支持节点选中、右键菜单、拖拽
- **Inspector System**：基于 `InspectorNodeInfo`（path + kind + guid），支持 Material、Texture、Shader 等多种 Inspector
- **Serialization**：`serialize_asset` 模块（audio、material、texture），使用 TOML 格式

## 1.2 核心概念

- **`.world` 文件**：TOML 格式的场景描述文件，包含实体层次结构和组件数据
- **Hierarchy 面板**：编辑器中可视化 `.world` 内容的树形面板
- **Inspector 面板**：选中 Hierarchy 节点后，展示该节点（Entity）及其组件的详细信息

## 1.3 已确认决策

### D-001：.world 文件格式

**决策**：使用 **TOML** 格式。

**理由**：与现有资产系统（`.mat`、`.texture`）保持一致，可读性好，已有 `toml` crate 依赖。

**备选方案**：
- JSON：同样可读，但 TOML 对嵌套结构和多行字符串更友好
- 自定义二进制：性能好但不可读，不适合编辑器资产
- RON（Rusty Object Notation）：Rust 原生，但生态不如 TOML 成熟

### D-002：.world 文件 schema 架构

**决策**：采用**混合方案**——以类型化 section（方案 B）为主，动态值（方案 A）为扩展兜底。

**设计**：

1. **Entity/Component 层用方案 B**：每个 Component 有确定的 `#[derive(Deserialize)]` Rust struct，编译期类型安全 + Inspector 精确 UI 控件。
2. **顶层新 section（System、Generator 等）先用方案 A 起步**：动态 `TomlValue` 足够，成熟后可"毕业"为方案 B。
3. **`#[serde(flatten)]` 兜底**：未知 section 自动吸收不丢数据（round-trip safe）。

**`.world` 顶层结构**：

```toml
[meta]
version = 1
name = "MainScene"

[entity.player]
parent = ""

[entity.player.component.Transform]
position = [0.0, 0.0, 0.0]
rotation = [0.0, 0.0, 0.0, 1.0]
scale = [1.0, 1.0, 1.0]

[entity.player.component.MeshRenderer]
mesh_path = "res/models/player.mesh"

# 未来扩展
# [system.physics]
# [generator.terrain]
# [whatever.future_thing]
```

**理由**：`.world` 定位为"超级容器"，需同时满足编译期安全（核心 Entity/Component）和运行时扩展（未来未知类型）。

---

### D-003：节点嵌套 = Transform 层级

**决策**：Hierarchy 面板中的节点嵌套表示 **Transform 父子关系**，与 UE、Unity、Godot 一致。

**规则**：
1. Hierarchy 中的每一个节点对应的 Entity 都必须携带 `Transform` Component
2. 子节点的世界空间 Transform = 父节点世界 Transform × 子节点局部 Transform
3. 拖拽节点改变父子关系
4. 纯逻辑 Entity（TimerManager、GameState 等）不显示在 Hierarchy 中
5. 未来可叠加 Folder 节点（仅编辑时的组织工具，运行时不存在）

**前置依赖**：当前 `Transform` 不支持嵌套结构，也没有 System 处理层级 Transform 更新。需要在实现 Hierarchy 面板之前或同步实现 Transform 层级系统（`Parent`/`Children` 组件 + `TransformPropagateSystem`）。

### D-004：Transform 层级更新方案

**决策**：采用 Bevy 模式——组件化 + 系统轮询。

**设计**：

1. **新增组件**：
   - `Parent(Entity)`：当前 Entity 的父节点
   - `Children(Vec<Entity>)`：当前 Entity 的子节点列表
   - `GlobalTransform(float4x4)`：缓存的世界空间变换矩阵

2. **现有 `Transform` 重命名为 `LocalTransform`**：语义明确为**局部空间**（相对父节点）。`get_local_to_world()` 改为 `compute_local_matrix()`。所有引用 `Transform` 的地方（音频、相机等）同步更新。

3. **传播系统 `TransformPropagateSystem`**：每帧运行，自顶向下计算 `GlobalTransform = parent.GlobalTransform × self.Transform.local_matrix()`，然后递归处理子节点。

4. **变更检测**：实现完整方案——为 ECS 添加 `Changed<T>` 查询过滤器，传播系统只处理 dirty 子树。

5. **编辑器反馈**：接受 1 帧延迟，传播系统在帧循环中自然执行即可，无需立即传播。

**前置依赖链**：
```
ECS Change Detection → Transform 层级系统 → Hierarchy 面板
```

**理由**：
- 方案 A 最符合 ECS 哲学，`GlobalTransform` 缓存让下游系统（渲染、物理、音频）直接读取无需遍历树
- 方案 B（按需计算）破坏批量查询性能
- 方案 C（事件通知）将副作用引入数据层

### D-005：Hierarchy 面板的根节点

**决策**：采用 Unity 方案——树根显示 "World"，所有顶层 Entity 是其子节点。

**未来方向**：World 内可包含多个 Scene 根节点，每个 Scene 内含各自的 Entity 子树。当前先保持简单。

### D-006：编辑时 World vs 运行时 World

**决策**：双 World + 面板切换模型（类似 Unity）。

**三态流**：

1. **编辑器启动**：从 `res/worlds/World.world`（默认路径）加载创建 SceneWorld。
2. **编辑态（Edit）**：Hierarchy、Scene、Inspector 读写 SceneWorld 中的 Entity/Component，修改立即反映在 UI 上。
3. **保存（Cmd+S）**：将 SceneWorld 序列化写回 `.world` 文件。
4. **Play**：从 `.world` 文件重新加载，创建 GameWorld（全新实例）。所有面板（Hierarchy、Scene、Inspector）切换到 GameWorld。
5. **运行时（Play）**：Inspector 允许临时修改 GameWorld 中的 Component（调试用途，Stop 后丢弃）。Scene 窗口渲染 GameWorld，可点击选中 Entity。
6. **Stop**：丢弃 GameWorld，所有面板切回 SceneWorld。

**核心设计**：

- 所有面板不感知具体 World 实例，只感知"当前活跃 World"——通过 `EditorMode` 切换数据源
- `EditorMode::Edit` → 活跃 World = SceneWorld，面板读写 SceneWorld
- `EditorMode::Play` → 活跃 World = GameWorld，面板读写 GameWorld
- Stop 后切回 Edit 模式，面板重新指向 SceneWorld

**对 `KairosEditorRuntime` 的影响**：当前只持有 `engine.world`，需要扩展为 SceneWorld + `Option<GameWorld>` + EditorMode 状态机。

**Play 时 Inspector 权限**：允许临时读写（调试用），Stop 后修改随 GameWorld 丢弃。

### D-007：Component Inspector 实现方式与架构分离

**决策**：分布式注册方案——`#[derive(Component)]` 宏 + `inventory` crate + `EntityComponentInspector` trait。添加新 Component 不会遗漏 Inspector 实现。

**核心机制**：

1. `#[derive(Component)]` 展开时自动生成三种代码：
   - (a) `impl Component for T`（原有 marker trait）
   - (b) 编译期约束——强制 `T` 必须实现 `EntityComponentInspector<T>`，否则编译失败
   - (c) `inventory::submit!` 向全局注册表注入 `ComponentMeta`（含 type_id、name、draw_fn、deserialize_fn、serialize_fn）

2. 遍历：`inventory::iter::<ComponentMeta>()` 获取所有已注册组件，无需手动枚举。

3. `EntityComponentInspector<T>` trait 定义（`ui/inspector/entity/` 下）：
   - `draw(ui, component)` — Inspector UI 绘制
   - `component_name()` — 人类可读名称
   - `toml_section_name()` — .world 文件中的 section 名
   - `try_deserialize(toml)` — 从 TOML 反序列化
   - `try_serialize(component)` — 序列化为 TOML

**目录结构**：

```
kairos_engine/src/
├── spatial/transform.rs                    ← Transform struct + #[derive(Component)]（运行时）
└── kairos_editor/ui/inspector/entity/
    ├── registry.rs                         ← ComponentMeta + inventory::collect!
    ├── trait.rs                            ← EntityComponentInspector trait
    ├── transform_inspector.rs              ← impl EntityComponentInspector<Transform>（编辑器）
    ├── mesh_renderer_inspector.rs
    └── camera_inspector.rs
```

**添加新组件的完整流程**：
```
1. #[derive(Component)] pub struct RigidBody { ... }
2. impl EntityComponentInspector<RigidBody> for RigidBodyInspector { ... }
3. 完成——无需改任何注册表、枚举、match 语句
```

**SceneWorld/GameWorld 同样遵循分离原则**：核心 ECS `World` 不感知编辑模式，编辑器侧维护 `EditorMode` 状态机。

**理由**：
- `inventory` 是 Rust 生态分布式注册的标准方案（Bevy App、tracing subscriber 等均采用）
- 编译器强制检查防止遗漏 Inspector 实现
- 编辑器代码与运行时 Component 完全解耦

### D-008：与 Project Window hierarchy_panel 的关系

**决策**：

1. **命名**：新的 Hierarchy 面板命名为 `hierarchy_window`（沿用现有占位文件名），Project Window 中的 `hierarchy_panel` 后续可重命名为 `directory_tree` 以消除歧义。
2. **实现**：完全独立实现，不复用 `hierarchy_panel.rs` 的树形 UI 代码——Entity 树的绘制表现和布局差异较大。底层的通用数据结构（如树遍历、有序子节点迭代等）可酌情抽象复用。

### D-009：.world 文件存放位置与可见性

**决策**：

1. **默认路径**：`res/worlds/World.world`，编辑器启动时自动加载。
2. **支持多文件**：`res/worlds/` 目录下可存放多个 `.world` 文件（如 `Level1.world`、`MainMenu.world`）。
3. **Project Window 中可见且可操作**：
   - 双击 `.world` 文件 → 替换加载该场景到 SceneWorld（未保存修改时弹出确认对话框）
   - 从 Project Window 拖拽 `.world` 到 Hierarchy → 打开拖拽的场景（当前实现为加载替换）
   - 右键菜单 → 新建 `.world` 文件

---

## 待确认事项

| # | 问题 | 状态 |
|---|------|------|
| ~~Q1~~ | ~~.world 文件格式~~ | ✅ D-001 |
| ~~Q2~~ | ~~SerializedEntity/SerializedComponent 的 schema~~ | ✅ D-002 |
| ~~Q3~~ | ~~节点嵌套含义~~ | ✅ D-003 |
| ~~Q4~~ | ~~Transform 层级更新方案~~ | ✅ D-004 |
| ~~Q5~~ | ~~Hierarchy 面板的根节点~~ | ✅ D-005 |
| ~~Q6~~ | ~~编辑时 World vs 运行时 World 的关系~~ | ✅ D-006 |
| ~~Q7~~ | ~~Hierarchy 与 Inspector 的协作方式~~ | ✅ D-007 |
| ~~Q8~~ | ~~与 Project Window hierarchy_panel 的关系~~ | ✅ D-008 |
| ~~Q9~~ | ~~.world 文件存放位置~~ | ✅ D-009 |
