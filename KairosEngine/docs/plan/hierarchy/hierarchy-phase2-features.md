# Hierarchy 面板设计案 — Phase 2：功能分析

> **状态**：进行中
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./hierarchy-phase1-requirements.md)
> **研究参考**：
> - [Unity Hierarchy 调研](../research/hierarchy/unity-hierarchy.md)
> - [UE World Outliner 调研](../research/hierarchy/ue-outliner.md)
> - [Bevy & Godot Hierarchy 调研](../research/hierarchy/bevy-godot-hierarchy.md)

## 2.1 功能全景

```
┌─────────────────────────────────────────────────────────────────┐
│                     Hierarchy 面板系统                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ F0: ECS      │  │ F1: Transform│  │ F2: .world 文件      │  │
│  │ Change       │─▶│ 层级系统      │  │ 序列化/反序列化      │  │
│  │ Detection    │  │              │  │                      │  │
│  └──────────────┘  └──────┬───────┘  └──────────┬───────────┘  │
│                           │                      │               │
│                           ▼                      ▼               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              F3: Hierarchy Window UI                      │   │
│  │  ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────┐  │   │
│  │  │树形渲染  │ │选中/多选 │ │右键菜单   │ │拖拽重定父  │  │   │
│  │  └─────────┘ └────┬─────┘ └───────────┘ └────────────┘  │   │
│  │                   │                                       │   │
│  │  ┌─────────┐ ┌────┴─────┐ ┌───────────┐ ┌────────────┐  │   │
│  │  │搜索过滤  │ │复制/粘贴 │ │重命名     │ │展开/折叠   │  │   │
│  │  └─────────┘ └──────────┘ └───────────┘ └────────────┘  │   │
│  └──────────────────────┬───────────────────────────────────┘   │
│                         │                                       │
│                         ▼                                       │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              F4: Entity Inspector                         │   │
│  │  ┌──────────────────┐ ┌─────────────────┐ ┌───────────┐  │   │
│  │  │组件列表+字段编辑  │ │添加/移除组件    │ │分布式注册  │  │   │
│  │  └──────────────────┘ └─────────────────┘ └───────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              F5: EditorMode 状态机                        │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────────────────┐  │   │
│  │  │Edit/Play │ │World切换  │ │面板数据源切换           │  │   │
│  │  │/Stop     │ │          │ │                          │  │   │
│  │  └──────────┘ └──────────┘ └──────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              F6: Project Window 集成                      │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐  │   │
│  │  │.world    │ │双击打开  │ │右键新建  │ │拖拽到      │  │   │
│  │  │AssetKind │ │场景      │ │.world    │ │Hierarchy   │  │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2.2 功能详细拆解

### F0：ECS Change Detection（前置依赖）

> 依赖：无
> 被依赖：F1 Transform 层级系统

**目标**：为 ECS 添加组件变更检测能力，使得系统可以查询"哪些 Entity 的特定 Component 在本帧发生了变化"。

**功能清单**：

| ID | 功能 | 说明 |
|----|------|------|
| F0.1 | `Changed<T>` 查询过滤器 | 扩展查询系统，支持 `world.query::<Changed<Transform>>()` 等用法 |
| F0.2 | 变更标记机制 | Component 被 `&mut` 访问写入时自动标记 dirty；系统消费后清除 |
| F0.3 | 批量变更跟踪 | 支持 `Added<T>`（本帧新插入）和 `Changed<T>`（本帧被修改）两种粒度 |
| F0.4 | 性能考量 | 使用 generation counter 或 bitflag 实现，避免每次查询遍历全量 Entity |

**关键设计决策**（待 Phase 3 技术选型）：

- 标记粒度：per-Entity-per-Component？per-Component-table-column？
- 清除时机：每帧开始？每个 System 执行后？查询消费后？
- 与现有 table-based 存储的集成方式

---

### F1：Transform 层级系统（前置依赖）

> 依赖：F0 ECS Change Detection
> 被依赖：F3 Hierarchy Window UI

**目标**：为 ECS World 中的 Entity 建立 Transform 父子层级，支持局部-世界变换的传播计算。

**功能清单**：

| ID | 功能 | 说明 |
|----|------|------|
| F1.1 | `Parent` 组件 | `Parent(Entity)`，标识父节点。注意：当 `parent.0` 为无效 Entity 或 `EntityFlag::Dead` 时视为根节点 |
| F1.2 | `Children` 组件 | `Children(SmallVec<Entity>)`，维护子节点列表。操作 Parent 时需同步维护双向关系 |
| F1.3 | `Transform` 语义调整 | `position`/`rotation`/`scale` 含义改为**局部空间**（相对父节点），`get_local_to_world()` → `get_local_matrix()` |
| F1.4 | `GlobalTransform` 组件 | `GlobalTransform(float4x4)`，缓存的世界空间变换矩阵，由传播系统计算，下游系统直接读取 |
| F1.5 | 传播系统 | 自顶向下遍历，`GlobalTransform = parent.GlobalTransform × self.Transform.local_matrix()`。利用 F0 Changed<T> 只处理 dirty 子树 |
| F1.6 | 层级操作安全 | `add_child` / `remove_child` / `set_parent` 需防止循环引用、孤儿检测 |
| F1.7 | 根节点识别 | `parent == ""` 或 Parent 指向无效 Entity 的节点为根节点 |

**注意**：D-006 确定 GameWorld 从 `.world` 文件加载，因此运行时 Entity 也会携带 Parent/Children/Transform/GlobalTransform——这些组件不是编辑器专用，而是 ECS 核心组件。

**关键设计决策**（待 Phase 3）：

- Parent/Children 在 `spawn`/`despawn` 时的生命周期管理（despawn 父节点时子节点如何处理？Cascade despawn？Orphan？）
- `Children` 的存储结构——`Vec<Entity>` vs `SmallVec` vs `EntityHashSet`？
- 传播系统的执行时机——在渲染/物理/音频系统之前，确保下游读到正确的 GlobalTransform

---

### F2：.world 文件序列化/反序列化

> 依赖：F1 Transform 层级系统（部分：需要知道 SerializedComponent 的类型集合）
> 被依赖：F3 Hierarchy Window UI、F5 EditorMode 状态机

**目标**：实现 SceneWorld ↔ `.world` 文件的双向序列化。

**功能清单**：

| ID | 功能 | 说明 |
|----|------|------|
| F2.1 | `SerializedWorld` 数据结构 | 对应 `.world` TOML 文件的顶层结构（meta + entities + 未来扩展 section）。使用 `#[serde(flatten)]` 兜底未知 section |
| F2.2 | `SerializedEntity` 数据结构 | 每个 Entity 的序列化形式：id、parent、components map |
| F2.3 | Component 序列化/反序列化 | 通过 F4 的 `EntityComponentInspector` trait 提供的 `try_deserialize`/`try_serialize` 方法。读取时遍历所有已注册 ComponentMeta |
| F2.4 | SceneWorld → 文件 | Cmd+S 时，遍历 SceneWorld 中所有 Entity，调用每个 Component 的序列化方法，写入 .world 文件 |
| F2.5 | 文件 → SceneWorld | 编辑器启动/双击打开 .world 时，解析 TOML，为每个 entity section 创建 Entity + 反序列化 Component 并 insert |
| F2.6 | 文件 → GameWorld | Play 时：复用 F2.5 的加载逻辑，但目标 World 是全新的 GameWorld（而非 SceneWorld） |
| F2.7 | Round-trip 安全 | 未知 section（通过 `#[serde(flatten)]` 吸收）在保存时原样写回，不丢数据 |
| F2.8 | 多文件支持 | `res/worlds/` 目录扫描，支持加载不同 .world 文件。切换场景时提示未保存修改 |

**注意**：F2.1 的 `SerializedWorld` 是纯数据中间表示，引擎不应围绕它构建编辑器状态模型（D-006 确定编辑器操作的是 SceneWorld 运行时实例）。`SerializedWorld` 仅作为磁盘 ↔ World 的桥梁。

**关键设计决策**（待 Phase 3）：

- Entity ID 的序列化策略——`.world` 中的 `entity.player` 的 key（player）是 User-facing name 还是稳定 ID？需考虑：Entity 改名后引用是否断裂？Child-parent 引用如何保持？
- 反序列化时的 Entity 去重——防止重复加载导致 Entity 泄漏

---

### F3：Hierarchy Window UI

> 依赖：F1 Transform 层级系统、F2 序列化
> 被依赖：F4 Entity Inspector（通过选中联动）

**目标**：在编辑器中渲染 Entity 树，支持所有交互操作。

**功能清单**：

| ID | 功能 | 说明 | 研究参考 |
|----|------|------|----------|
| F3.1 | 树形渲染 | 递归遍历活跃 World 中的 Entity（通过根节点 + Children 组件），渲染树形 UI。节点显示 Entity 名称 + 图标 | Unity: 原生 UI Toolkit TreeView；UE: Slate STreeView |
| F3.2 | 选中/多选 | 单击选中单个 Entity（同步到 Inspector + Scene 窗口高亮）。Ctrl+Click 多选，Shift+Click 范围选 | Unity: Selection.objects；UE: GEditor->GetSelectedActors() |
| F3.3 | 右键菜单 | 右键节点弹出 ContextMenu：Create Empty Entity / Delete Entity / Duplicate / Rename / Copy / Paste / Add Component（子菜单列出 ComponentKind::ALL） | Unity: GenericMenu；UE: FMenuBuilder |
| F3.4 | 拖拽重定父 | 拖拽 Entity A 到 Entity B → 改变 A.Parent = B，同步维护 B.Children。视觉反馈：拖拽时高亮潜在父节点 | Unity: Transform.SetParent + DragAndDrop；UE: AttachTo + 拖拽预览 |
| F3.5 | 搜索/过滤 | 输入框，按名称过滤 Entity。匹配的 Entity 高亮，不匹配的仍显示（灰色/dim）保持树结构可见 | Unity: SearchableEditorWindow.SearchField；UE: SceneOutliner 搜索栏 |
| F3.6 | 剪切/复制/粘贴/复制 | 复制：深拷贝 Entity + 所有 Component（不包括运行时状态）。粘贴：作为当前选中节点的兄弟或子节点 | Unity: Unsupported.CopyGameObjectsToPasteboard |
| F3.7 | 重命名 | 单击已选中节点进入编辑模式（类似 Project Window 的 renaming_buffer），输入新名称 | Unity: 慢双击；UE: F2 或慢双击 |
| F3.8 | 删除 | Delete 键或右键 → Delete，调用 `World::despawn(entity)`，级联 despawn 所有子节点 |
| F3.9 | 展开/折叠 | 记录折叠状态，支持 Alt+Click 递归展开/折叠子树 |
| F3.10 | 快捷键 | Delete 删除、Ctrl+D 复制、F2 重命名、Ctrl+C/V 复制粘贴 |

**与 F5 的集成**：
- 面板通过 `EditorMode` 获取活跃 World
- 编辑模式 → 操作 SceneWorld
- Play 模式 → 操作 GameWorld

**关键设计决策**（待 Phase 3）：

- 树形 UI 的渲染方式——使用 egui 的 `CollapsingHeader` vs 自定义 Tree 绘制？
- 大量 Entity（1000+）时的性能策略——虚拟滚动？
- 选中状态的存储位置——Hierarchy 面板内部？全局 `EditorSelection` resource？

---

### F4：Entity Inspector（选中 Entity 后的 Inspector 面板）

> 依赖：F3 Hierarchy Window UI（选中联动）、F2 序列化（Component 的 serde 方法）
> 被依赖：无

**目标**：选中 Hierarchy 中的 Entity 后，在 Inspector 面板中展示该 Entity 的所有 Component，支持编辑。

**功能清单**：

| ID | 功能 | 说明 | 研究参考 |
|----|------|------|----------|
| F4.1 | 组件列表展示 | 遍历 Entity 上所有已注册的 Component，使用 CollapsingHeader 逐组件展示 | Bevy: flat component list；Unity: 折叠 section；Godot: 按继承链分组 |
| F4.2 | 组件字段编辑 | 每个 Component 通过其 `EntityComponentInspector::draw()` 渲染字段编辑器。Transform: DragValue；枚举: ComboBox；路径: 文本+文件选择器 | D-007 分布式注册方案 |
| F4.3 | 添加组件 | "Add Component"按钮/右键菜单 → 子菜单列出所有 `ComponentKind::ALL` 中当前 Entity 没有的组件 → 点击后 `world.insert(entity, Component::default())` | Unity: Add Component 搜索栏 |
| F4.4 | 移除组件 | 每个 Component section 右上角 ⋮ 菜单 → "Remove Component" → `world.remove::<T>(entity)` | Unity: 右键 component header |
| F4.5 | Entity 名称编辑 | Inspector 顶部显示 Entity 名称，可直接编辑 |
| F4.6 | Play 模式行为 | D-006 确定：允许临时修改 GameWorld 中的 Component，Stop 后丢弃 |
| F4.7 | 空状态 | 未选中任何 Entity 时显示 "Select an Entity in Hierarchy" |

**与现有 InspectorWindow 的关系**：

- 现有的 `InspectorWindow`（资产 Inspector：Material、Texture 等）和新的 Entity Inspector 可以共用同一个 `InspectorWindow` 面板
- 通过 `InspectorNodeInfo` 区分上下文：资产 Inspector vs Entity Inspector
- 或者：Entity Inspector 作为独立的 Inspector 上下文，`InspectorWindow` 根据选中来源切换

---

### F5：EditorMode 状态机 + World 管理

> 依赖：F2 序列化（Play 时加载 GameWorld）
> 被依赖：F3 Hierarchy Window UI、F4 Entity Inspector、Scene Window

**目标**：管理编辑器的 Edit/Play/Stop 状态转换，以及 SceneWorld ↔ GameWorld 的创建/切换/销毁。

**功能清单**：

| ID | 功能 | 说明 |
|----|------|------|
| F5.1 | EditorMode 枚举 | `enum EditorMode { Edit, Play }`，编辑器启动时默认为 Edit |
| F5.2 | World 管理 | `struct WorldManager { scene_world: World, game_world: Option<World>, mode: EditorMode }` |
| F5.3 | 获取活跃 World | `fn active_world(&self) -> &World` 和 `fn active_world_mut(&mut self) -> &mut World`，根据 mode 返回 SceneWorld 或 GameWorld |
| F5.4 | Play 流程 | 1) 检查未保存修改，提示保存；2) 从 .world 文件加载 GameWorld；3) mode 切换到 Play；4) 面板（Hierarchy、Scene、Inspector）自动指向 GameWorld |
| F5.5 | Stop 流程 | 1) 停止游戏 Systems；2) 销毁 GameWorld；3) mode 切回 Edit；4) 面板切回 SceneWorld |
| F5.6 | 工具栏 Play/Stop 按钮 | 连接现有的 ToolBar UI，新增 Play（▶）和 Stop（■）按钮 |

**对 `KairosEditorRuntime` 的改造**：

当前结构：
```rust
struct KairosEditorRuntime { engine: Engine /* 含 world: World */ }
```

改造为：
```rust
struct KairosEditorRuntime {
    scene_world: World,
    game_world: Option<World>,
    editor_mode: EditorMode,
    // ... 其他字段
}
```

---

### F6：Project Window 集成

> 依赖：F2 序列化
> 被依赖：F5 EditorMode（双击打开 .world 触发场景加载）

**目标**：`.world` 文件作为 Project Window 中的一等公民资产，支持双击打开、右键新建、拖拽到 Hierarchy。

**功能清单**：

| ID | 功能 | 说明 |
|----|------|------|
| F6.1 | AssetKind::World | 扩展 `AssetKind` 枚举，新增 `World` variant |
| F6.2 | 双击打开 | Project Window 双击 `.world` 文件 → 检查 SceneWorld 是否有未保存修改 → 弹出确认对话框 → 加载新 .world 到 SceneWorld |
| F6.3 | 右键新建 | Project Window 右键菜单 → "Create World" → 创建空的 .world 文件（含最小 meta + 空 entities） |
| F6.4 | 拖拽到 Hierarchy | 从 Project Window 拖拽 .world 文件到 Hierarchy → 加载该 .world（同双击） |
| F6.5 | 图标 | .world 文件在 Project Window 中的图标，加入 `global_styles` 的 project_node_icons |

---

## 2.3 依赖关系

```
F0: ECS Change Detection
 │
 └─▶ F1: Transform 层级系统
      │
      ├─▶ F3: Hierarchy Window UI
      │    │
      │    └─▶ F4: Entity Inspector
      │
      └─▶ F2: .world 文件序列化
           │
           ├─▶ F5: EditorMode 状态机
           │    │
           │    └─▶ (集成 F3 + F4 的 World 切换)
           │
           └─▶ F6: Project Window 集成
```

**独立可并行的功能对**：
- F3 + F4：可并行（F4 需要 F3 的选中联动，但可定义接口后并行开发）
- F6：相对独立（仅依赖 F2 的 AssetKind 扩展）

---

## 待确认事项

Phase 2 功能分析完成。以下为待 Phase 3 技术选型中解决的开放问题：

| # | 问题 | 关联功能 |
|---|------|----------|
| T1 | Change Detection 的标记粒度和清除时机 | F0 |
| T2 | Parent/Children 在 despawn 时的级联策略 | F1 |
| T3 | Entity 的稳定 ID vs 用户可见名称 | F2 |
| T4 | 大量 Entity 的树形 UI 性能策略 | F3 |
| T5 | Inspector 是共用还是独立的 InspectorWindow | F4 |
| T6 | 选中状态存储位置（面板内部 vs 全局） | F3 |
