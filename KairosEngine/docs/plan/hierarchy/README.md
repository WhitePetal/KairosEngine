# Hierarchy 面板设计案

> 参照 Material Inspector 设计案的 4 阶段流程

| 阶段 | 文件 | 状态 |
|------|------|------|
| Phase 1：需求分析 | [hierarchy-phase1-requirements.md](./hierarchy-phase1-requirements.md) | ✅ 完成（9 项决策） |
| Phase 2：功能分析 | [hierarchy-phase2-features.md](./hierarchy-phase2-features.md) | ✅ 完成（6 大功能域） |
| Phase 3：技术选型与坑点 | [hierarchy-phase3-tech.md](./hierarchy-phase3-tech.md) | ✅ 完成（6 项技术决策 + 3 个坑点） |
| Phase 4：功能排期 | [hierarchy-phase4-schedule.md](./hierarchy-phase4-schedule.md) | ✅ 完成（37 个任务，7 周排期） |

## 研究参考

| 引擎 | 文件 |
|------|------|
| Unity Hierarchy | [../research/hierarchy/unity-hierarchy.md](../research/hierarchy/unity-hierarchy.md) |
| UE World Outliner | [../research/hierarchy/ue-outliner.md](../research/hierarchy/ue-outliner.md) |
| Bevy & Godot | [../research/hierarchy/bevy-godot-hierarchy.md](../research/hierarchy/bevy-godot-hierarchy.md) |

## 关键架构决策

```
ECS Change Detection → Transform 层级系统 → Hierarchy 面板
                                              │
                    .world 序列化 ←────────────┤
                                              │
                    EditorMode 状态机 ←────────┤
                                              │
                    Entity Inspector ←────────┘
```
