# Handoff: Material 动态属性系统 — 实现阶段

## 会话背景

完成了 Material 动态属性系统的完整设计（需求分析 → 竞品调研 → 功能分析 → 技术选型 → 排期 → PRD → Tickets）。现在进入实现阶段。

## 当前状态

- **PRD**: https://github.com/WhitePetal/KairosEngine/issues/102
- **Tickets**: 6 个，全部 `ready-for-agent`，依赖关系已通过 GitHub native blocking 连线
- **前沿（可立即开始）**: #103 + #104

## 实现顺序

```
#103 (Shader 反射) ──┬── #108 (Material 模型) ──┬── #107 (GPU 管线) ──┬── #106 (编辑器) ── #105 (收尾)
                     │                          │                    │
#104 (reload) ───────┘                          └────────────────────┘
```

每次取一个未被阻挡的 ticket 执行，建议顺序：#103 → #104 并行 → #108 → #107 → #106 → #105。

## 关键设计文档

| 文档 | 路径 |
|------|------|
| 设计案索引 | `docs/plan/material-dynamic-properties.md` |
| 需求分析 | `docs/plan/material-dynamic-properties/phase1-requirements.md` |
| 竞品调研 | `docs/research/material-dynamic-properties-competitor-analysis.md` |
| 功能分析 | `docs/plan/material-dynamic-properties/phase3-functional-analysis.md` |
| 技术选型 | `docs/plan/material-dynamic-properties/phase4-tech-decisions.md` |
| 实现排期 | `docs/plan/material-dynamic-properties/phase5-implementation-plan.md` |

## 核心技术决策速查

- **架构**: Unity/Godot Shader 驱动模式，naga 反射 WGSL
- **Shader 约定**: 每个 Shader 有且仅有一个单层 struct `var<uniform>`（如 `PerMaterialCBuffer`）
- **数据模型**: `Material.properties: HashMap<String, MaterialProperty>`（enum 覆盖 f32/vec2/vec3/vec4/int/bool/texture）
- **GPU 序列化**: `encase` crate，std140 布局
- **属性元数据**: WGSL 注释 `// @range(0,1) @slider @color @group "PBR"`
- **文件监听**: `notify` crate，仅在 `kairos_editor`
- **运行时 API**: 静默失败 + `log::warn!`（与 Unity 一致）
- **序列化**: TOML `[material.properties]` + `[material.textures]`，不向后兼容
- **测试**: 仅纯逻辑 seam（naga 反射/序列化往返/协调逻辑），Test Harness 不可用

## 涉及的关键文件

- `kairos_engine/src/graphics/material.rs` — Material + SerializedMaterial 扩展
- `kairos_engine/src/graphics/shader_reflection.rs` — **新建**，naga 反射
- `kairos_engine/src/graphics/render_pipeline.rs` — 动态 bind group
- `kairos_engine/src/asset_loader/assets/asset.rs` — Assets::reload()
- `kairos_editor/src/ui/inspector/material.rs` — Inspector 动态 UI
- `kairos_editor/src/file_watcher.rs` — **新建**，notify 文件监听

## 新增依赖

- `encase` — `kairos_engine/Cargo.toml`
- `notify` — workspace `Cargo.toml` 或 `kairos_editor` 级别

## Suggested Skills

- `/implement` — 逐个 ticket 实现
- `kairos-test` — 编写纯逻辑单元测试（naga 反射、序列化、协调）
- `codebase-design` — 如遇模块接口设计问题可参考
