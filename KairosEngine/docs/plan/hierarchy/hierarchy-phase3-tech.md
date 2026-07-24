# Hierarchy 面板设计案 — Phase 3：技术选型与坑点

> **状态**：✅ 完成
> **创建日期**：2026-07-24
> **前置文档**：[Phase 1 需求分析](./hierarchy-phase1-requirements.md) | [Phase 2 功能分析](./hierarchy-phase2-features.md)

## 3.1 技术选型决策

| 编号 | 决策 | 方案 | 关键细节 |
|------|------|------|----------|
| T1 | ECS Change Detection | Per-Component Generation Counter（`Vec<u32>` ticks） | 接受误报（读取 `&mut` 也标记 dirty），与 Bevy 一致 |
| T2 | despawn 级联 | Cascade Despawn | 先用递归实现，遇到深层嵌套再改显式栈 |
| T3 | Entity 标识 | 路径式命名（key 即路径） | 同层级唯一，完整路径作为全局标识；编辑器保证引用一致性 |
| T4 | 树形 UI 性能 | 阶段 2 为 MVP 交付标准 | 折叠的子树跳过递归遍历 |
| T5 | Inspector 组织 | 共用 InspectorWindow + 内部路由 | 根据选中来源切换到 Asset Inspector 或 Entity Inspector |
| T6 | 选中状态管理 | Messager 事件驱动 | 沿用现有 `Message` 枚举模式，各面板各自维护选中状态 |

---

## 3.2 识别的坑点

### 坑点 1：`set_parent` 循环引用检测

拖拽重新定父时，需沿 Parent 链向上检查，防止 A→B→A 的循环。

### 坑点 2：.world 两步加载

TOML section 读取顺序不确定，需先创建所有 Entity + insert 所有 Component，再统一 set_parent。

### 坑点 3：`inventory` 跨 crate 注册

Component 定义在 `kairos_engine`，`EntityComponentInspector` trait 在 `kairos_editor`。`inventory::submit!` 需在 editor 侧触发（由 `EntityComponentInspector` impl 所在箱发出），`#[derive(Component)]` 仅生成 trait bound 约束。

---

## 3.3 架构影响汇总

```
T1: Per-Component ticks
 ↓   每个 Component 列增加 Vec<u32>，写入路径增加 tick += 1
 ↓   新增查询过滤器：Changed<T>、Added<T>
 ↓
T2: Cascade Despawn
 ↓   World::despawn 递归处理 Children
 ↓
T3: 路径式 Entity key
 ↓   .world TOML: [entity."Room1/Camera"]
 ↓   parent = "Room1"（相对引用）
 ↓   编辑器拖拽时自动更新路径 key
 ↓
T4: 阶段 2 折叠跳过渲染
 ↓   折叠的 CollapsingHeader 子树不执行递归遍历
 ↓
T5: 共用 InspectorWindow
 ↓   InspectorContext enum { Asset, Entity }
 ↓   handle() 中根据 Message 切换上下文
 ↓
T6: Messager 事件驱动选中
 ↓   新增 Message::SelectHierarchyEntity(Option<Entity>)
 ↓   各面板 handle() 中响应
```
