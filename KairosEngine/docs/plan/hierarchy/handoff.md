# Handoff: Hierarchy 面板实现

> 从 2026-07-24 的设计对话中提取，供接手 agent 继续工作。

## 背景

KairosEngine 需要实现完整的 Hierarchy 面板系统。通过 4 阶段设计流程产出了完整的架构决策、功能拆解、技术选型和功能排期。

## 关键产物

### GitHub Issues（WhitePetal/KairosEngine）

- **Spec Issue**：[#47 — Spec: Hierarchy 面板 + Entity Inspector](https://github.com/WhitePetal/KairosEngine/issues/47) — 30 条 User Stories，完整的实现决策和测试策略
- **23 张子任务票**：#48-#70，全部标记 `ready-for-agent`

### 设计文档

`docs/plan/hierarchy/`：
- `README.md` — 主索引
- `hierarchy-phase1-requirements.md` — Phase 1：9 项架构决策（D-001~D-009）
- `hierarchy-phase2-features.md` — Phase 2：6 大功能域（F0~F6），27 个子功能
- `hierarchy-phase3-tech.md` — Phase 3：6 项技术选型（T1~T6）+ 3 个坑点
- `hierarchy-phase4-schedule.md` — Phase 4：37 个任务，7 周排期，6 个检查点

### 调研文档

`docs/research/hierarchy/`：`unity-hierarchy.md` / `ue-outliner.md` / `bevy-godot-hierarchy.md`

## 当前状态

23 张票已发布。**4 张可立即开始**（无阻塞）：

| 票号 | 标题 |
|------|------|
| [#50](https://github.com/WhitePetal/KairosEngine/issues/50) | ECS Change Detection |
| [#51](https://github.com/WhitePetal/KairosEngine/issues/51) | Parent/Children 组件 |
| [#48](https://github.com/WhitePetal/KairosEngine/issues/48) | LocalTransform 重命名 + GlobalTransform 组件 |
| [#49](https://github.com/WhitePetal/KairosEngine/issues/49) | EntityComponentInspector Registry |

## 关键架构决策速查

- **双 World 模型**：SceneWorld（编辑器）+ GameWorld（Play 时从 .world 文件创建），面板通过 EditorMode 切换
- **Transform 层级**：Bevy 模式 — Parent/Children/LocalTransform/GlobalTransform + Change Detection + 传播系统
- **Entity 标识**：路径式命名（`entity."Room1/Camera"`），编辑器维护一致性
- **Component Inspector**：`inventory` 分布式注册 + `EntityComponentInspector<T>` trait
- **测试**：纯逻辑走 `cargo test`；UI 手动 checklist；test-harness 不用（待重构）

## 下一步

领一张可立即开始的票（推荐 #50 或 #51），按票面 Acceptance Criteria 实现。

## Suggested Skills

- `kairos-test` — 测试策略参考
- `codebase-design` — deep-module 接口设计
