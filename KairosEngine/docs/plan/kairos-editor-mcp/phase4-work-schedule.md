# Phase 4: 实施排期 — `kairos_editor_mcp`

**日期**: 2026-07-24  
**父Ticket**: [#71](https://github.com/WhitePetal/KairosEngine/issues/71)  
**状态**: 最终交付

---

## 1. 总览

基于 Phase 1-3 的全部设计决策，将实施工作拆分为 6 个 Layer、56 个任务。按依赖关系排序，分 5 个 Milestone 交付。

```
Layer 0 ──► Layer 1 ──► Layer 2 ──► Layer 3 (MVP) ──► Layer 4 (Enh) ──► Layer 5 (Complete)
  1d         3-5d        3-4d          5-7d                 3-4d               3-4d
```

**总估算**: 18-25 人天（不含 Layer 5 P2 工具）

---

## 2. 依赖图

```
                        ┌─────────────────────────────────────────┐
Layer 0 (1d)           │ 1. egui_inspection  2. crate scaffold   │
                        └──────┬──────────────────┬──────────────┘
                               │                  │
                        ┌──────▼──────────────────▼──────────────┐
Layer 1 (3-5d)         │ 3. types  4. plugin  5. QueryHandler   │
   核心中间层           │ 6. ProjectWindow  7. InspectorWindow   │
                        │ 8. wire into render loop  9. tests     │
                        └──────┬──────────────────┬──────────────┘
                               │                  │
                        ┌──────▼──────────────────▼──────────────┐
Layer 2 (3-4d)         │ 10. integrate UiServer  11. channel     │
   MCP Server 基础     │ 12. lifecycle  13. stderr  14. crash   │
                        └──────────────┬─────────────────────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
     ┌────────▼────────┐    ┌──────────▼──────────┐    ┌───────▼───────┐
     │ 15-22 读取查询   │    │ 23-28 写入命令       │    │ 29-32 场景    │
     │ (8 tools)       │    │ (6 tools)            │    │ (4 tools)     │
     └────────┬────────┘    └──────────┬──────────┘    └───────┬───────┘
              │                        │                        │
              └────────────────────────┼────────────────────────┘
                                       │
                        ┌──────────────▼─────────────────────────┐
Layer 3 (5-7d)         │ 33. wait_for_console                    │
   P0 工具 (20+3)      │ 34. cargo_build   35. cargo_run         │
                        └──────────────┬─────────────────────────┘
                                       │
                        ┌──────────────▼─────────────────────────┐
Layer 4 (3-4d)         │ 36-46  P1 工具 (11 tools)               │
   P1 Enhancement      │ camera_fly, game_*, save_asset, tabs   │
                        └──────────────┬─────────────────────────┘
                                       │
                        ┌──────────────▼─────────────────────────┐
Layer 5 (3-4d)         │ 47-53  P2 工具 (7 tools)                │
   P2 Complete          │ preferences, audio, material, batch     │
                        └────────────────────────────────────────┘
```

---

## 3. 任务清单

### Layer 0: Prerequisites (1 天)

| # | 任务 | 文件 | 估时 |
|---|------|------|------|
| 1 | 添加 `egui_inspection` 依赖，注册 `InspectionPlugin` | `kairos_engine/Cargo.toml`, `runtime.rs` | 0.5d |
| 2 | 创建 `kairos_editor_mcp` crate 骨架 | `kairos_editor_mcp/` | 0.5d |

### Layer 1: Core Middleware (3-5 天)

| # | 任务 | 文件 | 估时 | 依赖 |
|---|------|------|------|------|
| 3 | 从原型移植 `types.rs`（EditorQuery/Response/Command/snapshot 类型） | `ui/editor_channel/types.rs` (新) | 0.5d | — |
| 4 | 从原型移植 `EditorChannelPlugin` | `ui/editor_channel/plugin.rs` (新) | 1d | 3 |
| 5 | 在 `Drawer` trait 增加 `as_query_handler()` 默认方法 | `ui.rs` | 0.5d | 3 |
| 6 | `ProjectWindow` impl `QueryHandler`（get_project_tree, get_asset_info, get_asset_registry, get_selected_asset） | `project_window.rs` | 1d | 5 |
| 7 | `InspectorWindow` impl `QueryHandler`（get_inspector_state） | `inspector_window.rs` | 0.5d | 5 |
| 8 | 在 `KairosEditorRuntime::redraw()` 中集成 plugin hook | `runtime.rs` | 0.5d | 4,6,7 |
| 9 | 编写中间层单元测试（dispatch 逻辑 + mock handlers） | `editor_channel/test.rs` | 0.5d | 4 |

### Layer 2: MCP Server Foundation (3-4 天)

| # | 任务 | 文件 | 估时 | 依赖 |
|---|------|------|------|------|
| 10 | 集成 `egui_mcp` 作为 library，创建 `UiServer` 实例 | `kairos_editor_mcp/src/server.rs` | 1d | 1,2 |
| 11 | 实现 `EditorChannel`（MCP 端，mpsc sender） | `kairos_editor_mcp/src/channel.rs` | 0.5d | 3,10 |
| 12 | 实现 lifecycle 工具（attach/disconnect/status，复用 egui_mcp） | `kairos_editor_mcp/src/tools/lifecycle.rs` | 0.5d | 10 |
| 13 | 实现 stderr 操作日志 | `kairos_editor_mcp/src/logging.rs` | 0.5d | 10 |
| 14 | 实现 crash/panic notification（编辑器注册 panic_hook + MCP notification） | `runtime.rs` + `kairos_editor_mcp` | 1d | 8,11 |

### Layer 3: P0 工具 — MVP (5-7 天)

#### 读取查询（8 tools）

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 15 | `get_project_tree` | 0.5d | 6,11 |
| 16 | `get_asset_info` | 0.25d | 6,11 |
| 17 | `get_asset_registry` | 0.25d | 6,11 |
| 18 | `get_selected_asset` | 0.25d | 6,11 |
| 19 | `get_inspector_state` | 0.25d | 7,11 |
| 20 | `get_console_logs` | 0.25d | 8,11 |
| 21 | `clear_console` | 0.25d | 8,11 |
| 22 | `get_editor_state` | 0.5d | 6,7,11 |

#### 写入命令（6 tools）

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 23 | `select_asset` | 0.25d | 6,11 |
| 24 | `open_asset` | 0.25d | 6,11 |
| 25 | `create_asset` | 0.5d | 6,11 |
| 26 | `delete_asset` | 0.25d | 6,11 |
| 27 | `rename_asset` | 0.25d | 6,11 |
| 28 | `inspect_asset` | 0.25d | 7,11 |

#### 场景视图（4 tools）

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 29 | `get_scene_camera` | 0.5d | 8,11 |
| 30 | `camera_orbit` | 0.25d | 8,11 |
| 31 | `camera_zoom` | 0.25d | 8,11 |
| 32 | `scene_screenshot` | 0.5d | 8,11 |

#### 等待 + 编译工具（5 tools）

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 33 | `wait_for_console` | 0.5d | 20,11 |
| 34 | `cargo_build` | 0.5d | 2 |
| 35 | `cargo_run` | 0.5d | 2,34 |

### Layer 4: P1 工具 — Enhancement (3-4 天)

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 36 | `camera_fly` | 0.25d | 29-31 |
| 37 | `game_screenshot` | 0.5d | 8,11 |
| 38 | `get_game_state` | 0.25d | 8,11 |
| 39 | `refresh_project` | 0.25d | 6,11 |
| 40 | `duplicate_asset` | 0.25d | 25 |
| 41 | `set_asset_field` | 0.5d | 7,11 |
| 42 | `save_asset` | 0.25d | 6,11 |
| 43 | `close_tab` | 0.25d | 8,11 |
| 44 | `open_tab` | 0.25d | 8,11 |
| 45 | `get_dock_layout` | 0.25d | 8,11 |
| 46 | `wait_for_asset` | 0.5d | 16,33 |

### Layer 5: P2 工具 — Complete (3-4 天)

| # | 工具 | 估时 | 依赖 |
|---|------|------|------|
| 47 | `search_assets` | 0.5d | 17 |
| 48 | `get_editor_preferences` | 0.25d | — |
| 49 | `set_editor_preference` | 0.25d | 48 |
| 50 | `audio_preview_play` | 0.25d | — |
| 51 | `audio_preview_pause` | 0.25d | 50 |
| 52 | `material_set_shader` / `material_set_texture` | 0.5d | 7 |
| 53 | `wait_for_state_change` | 0.5d | 33 |

### Layer 6: Polish (2-3 天)

| # | 任务 | 估时 |
|---|------|------|
| 54 | Error handling 加固（所有 unwrap → proper error） | 0.5d |
| 55 | 集成测试（端到端：启动编辑器 → attach → 操作 → 验证） | 1d |
| 56 | 更新 `AGENTS.md` / `CONTEXT.md`，添加 MCP 使用说明 | 0.5d |

---

## 4. Milestone 规划

| Milestone | 内容 | 工具数 | 估时 | 交付物 |
|-----------|------|--------|------|--------|
| **M0: Scaffold** | Layer 0 + Layer 1 | — | 4-6d | 编辑器可被 egui_mcp 独立 binary 连接并驱动 |
| **M1: MVP** | Layer 2 + Layer 3 | 20 + 3 wait + 3 cargo | 8-11d | Agent 可完成完整开发-验证闭环 |
| **M2: Enhancement** | Layer 4 | +11 | 3-4d | 资产修改、游戏视口、tab 管理 |
| **M3: Complete** | Layer 5 | +7 | 3-4d | 偏好设置、音频、材质高级编辑 |
| **M4: Polish** | Layer 6 | — | 2-3d | 生产就绪 |

**总工期**: 20-28 人天

---

## 5. 关键路径

```
1. egui_inspection ──► 10. UiServer ──► 12. lifecycle ──► 15-35. P0 tools
                    ──► 3. types ──► 4. plugin ──► 5. QueryHandler
                                                      ├── 6. ProjectWindow
                                                      └── 7. InspectorWindow
                                                           └── 8. render loop wire
                                                                └── 9. tests
```

**最晚开始**：`cargo_build`、`cargo_run`（不依赖编辑器端改动，仅依赖 crate scaffold）

**最早可并行**：
- Layer 1（编辑器端）与 Layer 2（MCP Server 端）可部分并行：types 定义后两端同时开发
- P0 读取查询和写入命令可并行（不同工具，无冲突）

---

## 6. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `egui_inspection` API 不稳定 | 中 | 高 | 锁定 0.35.0 版本；若上游 breaking change，考虑 fork |
| AccessKit 覆盖不足（某些 widget 不可见） | 中 | 中 | M0 阶段用 `query_tree` 审计；不可见 widget 走坐标 fallback |
| SceneWindow camera 查询需 `&mut` 冲突 | 低 | 中 | D3 已设计 read-only camera snapshot；必要时加 `RefCell` |
| `cargo_run` 跨平台兼容（macOS/Windows） | 中 | 低 | 用 `std::process::Command`；CI 多平台测试 |
