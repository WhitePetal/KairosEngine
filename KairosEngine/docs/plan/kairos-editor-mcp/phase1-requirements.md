# Phase 1: 需求分析 — `kairos_editor_mcp`

**日期**: 2026-07-24  
**父Ticket**: [#71](https://github.com/WhitePetal/KairosEngine/issues/71)  
**关联Ticket**: [D1](https://github.com/WhitePetal/KairosEngine/issues/73)

---

## 1. 总览

`kairos_editor_mcp` 是一个 MCP (Model Context Protocol) 工具，让 AI Agent 能够**全流程自驱地**操控 KairosEngine 编辑器、验证 UI 组件的行为和布局。它替代已废弃的 `kairos-test-harness`（TOML 运行时测试框架），成为编辑器 UI 交互验证的唯一路径。

Agent 的核心工作流：

```
Agent 写代码 → Agent 通过 MCP 触发编译+运行 → Agent 操作编辑器 → Agent 获取反馈 → Agent 判断通过/失败
```

---

## 2. 核心角色

```
┌──────────────┐    JSON-RPC (stdio)    ┌────────────────────┐    TCP (:5719)    ┌───────────────────┐
│  AI Agent    │ ◄────────────────────► │ kairos_editor_mcp  │ ◄───────────────► │  KairosEngine     │
│  (Claude/等)  │                       │  (独立进程)         │                   │  Editor (egui)    │
└──────────────┘                        └────────────────────┘                   └───────────────────┘
                                               │
                                               │ 可触发 cargo build + run
                                               ▼
                                        ┌───────────────────┐
                                        │  Cargo / 编译系统   │
                                        └───────────────────┘
```

| 角色 | 职责 |
|------|------|
| **AI Agent** | 决策者：写代码 → 触发编译运行 → 操作编辑器 → 分析反馈 → 判断结果 |
| **kairos_editor_mcp** | 中间人：提供 MCP Tools，转发 Agent 命令到编辑器，回报状态 |
| **KairosEngine Editor** | 被控端：响应 MCP 指令，执行操作，返回状态 |

---

## 3. 使用场景

### 3.1 UI 开发验证（核心场景）

**描述**：Agent 开发完一个 UI 组件后，验证布局和交互行为是否符合预期。

**流程**：
1. Agent 写完 Rust 代码
2. Agent 通过 MCP 触发 `cargo build` + `cargo run`（启动/重启编辑器）
3. Agent 操作编辑器（打开窗口、选中资产、操作 widget）
4. Agent 获取反馈，判断验证是否通过

**验证手段（三种组合）**：
- **截图** (`screenshot`)：视觉效果检查（需要多模态能力的 Agent）
- **AccessKit 树查询** (`query_tree` / `get_node`)：确认 widget 存在、bounds、状态
- **源码日志注入**：Agent 修改源码插入 `log::info!()` 调用 → 编译运行 → 通过 `get_console_logs` 获取日志验证

**典型调用量**：单次验证循环约 10 次 MCP 调用（待实际使用验证）。

**示例**：Agent 给 MaterialInspector 新增了一个"预览旋转角度"滑块：
```
1. cargo_build → 编译成功
2. cargo_run → 编辑器启动
3. attach → 连接编辑器
4. get_project_tree → 找到 material 资产
5. open_asset("Material.mat") → 打开 Inspector
6. query_tree { role: "Slider", label_contains: "旋转角度" } → 确认 widget 存在
7. drag { start: slider, end: {x:+100, y:0} } → 拖动滑块
8. wait_for { value_contains: "90" } → 等值变化
9. scene_screenshot → 截图供视觉确认
```

### 3.2 回归检查

**描述**：Agent 修改了布局代码后，手动触发批量检查，确保没有破环已有功能。

**触发方式**：Agent **手动触发**（不是 CI 自动触发）。

**检查范围**：由 Agent **自行确定**。Agent 分析自己完成的任务需要做哪些测试校验，然后分步测试校验。

**典型流程**：
```
Agent: "我刚改了 docking_tab 的布局逻辑，帮我验证所有窗口 tab 仍然正常显示"
→ Agent 自行决定检查什么：
  1. get_editor_state → 确认所有 tab 可打开
  2. open_tab("Inspector") → query_tree → 确认 widget 树完整
  3. open_tab("Console") → query_tree → 确认 widget 树完整
  4. ...
→ 每一步的结果 Agent 自行判断
```

### 3.3 交互流程测试

**描述**：多步骤操作序列的端到端验证，如"选中资源 → 修改属性 → 保存 → 重新打开 → 确认持久化"。

**验证策略**（基于 egui_mcp 推荐模式）：
- **同步原语**：`wait_for` —— 轮询 AccessKit 树，等待条件满足（如 console 出现 "Saved"、按钮状态变化）
- **批量操作**：`batch` —— 单次 MCP 往返执行多个动作，减少延迟
- **状态验证**：`query_tree` / `get_node` / `get_console_logs` —— 读取操作后的状态
- **文件验证**：重新 `open_asset` + `inspect_asset` —— 确认修改持久化到磁盘

**失败处理**：
- **操作-响应日志**：每一步的请求和响应必须可追溯（"step 3: click save → response: ok: true"）
- **按需截图**：Agent 决定是否截图（MCP 不主动截图）
- **不自动恢复**：失败后 Agent 自行分析，决定下一步

**egui_mcp 多步骤模式**：
```
// egui_mcp 没有内置测试框架。Agent 自己编排工具调用序列：
1. click("Save")              → { ok: true }
2. wait_for {                 → 轮询 AccessKit 树，直到 console 出现 "Saved"
     content_contains: "Saved successfully",
     timeout_secs: 5
   }
3. close_tab("Inspector")     → 关闭
4. open_asset("Material.mat") → 重新打开
5. inspect_asset("Material.mat") → 读取属性
6. → Agent 对比属性值判断持久化是否成功
```

### 3.4 编辑器功能探索

**描述**：Agent 需要了解编辑器当前状态——有哪些窗口/面板、它们的当前内容和布局。

**触发场景**：
- Agent 刚接入一个已运行的编辑器，需要了解当前状态
- Agent 执行操作后，确认没有意外关闭/打开窗口
- Agent 写代码前需要了解编辑器现有结构（如 MaterialInspector 有哪些字段）

**探索深度**：**完整 UI 树** —— 所有 widget 的层级结构、bounds、状态（role、label、value、focused、disabled、hidden、parent_id）。

**实现基础**：egui_mcp 的 `query_tree` 已能返回完整 AccessKit 树（角色过滤可选，不设过滤即返回全部）。

### 3.5 调试辅助

**描述**：Agent 遇到 UI bug 时，通过修改源码添加日志来定位问题。

**日志注入模式**：
- Agent 修改 KairosEngine 源码，插入 `log::info!("hierarchy count: {}", n);`
- Agent 通过 MCP 触发 `cargo build` + `cargo run`
- Agent 操作编辑器触发相关路径
- Agent 调用 `get_console_logs` 获取日志分析

**不需要**：运行时动态日志 hook 或 `agent_log()` 工具。Agent 有源码修改权限。

**崩溃处理**：
- MCP 检测到编辑器进程崩溃，回报崩溃信息（backtrace、最后状态快照）
- **不自动恢复**：Agent 自行分析崩溃原因
- Agent 可自行重启 MCP 和编辑器（`cargo_run` 重新启动）

### 3.6 与 `kairos-test` 的关系

- **MCP 验证** 替代已废弃的 `kairos-test-harness`（`tests/runtime/` TOML 测试框架）
- MCP 成为编辑器 UI 交互验证的**唯一路径**
- Rust 集成测试（`kairos_engine/tests/integration/`）保持不变，专注逻辑/数据层验证
- 分工：**Rust 集成测试 → 数据/逻辑** | **MCP → 编辑器 UI 交互**

---

## 4. 功能需求汇总

### 4.1 已由 egui_mcp 直接提供的能力

| 能力 | egui_mcp 工具 | 说明 |
|------|-------------|------|
| 连接管理 | `attach`, `disconnect`, `status` | TCP 连接编辑器 |
| Widget 发现 | `query_tree`, `get_node` | AccessKit 树查询 |
| 用户操作 | `click`, `hover`, `scroll`, `drag`, `type_text`, `press_key` | 事件注入 |
| 异步等待 | `wait_for` | 轮询条件满足 |
| 批量操作 | `batch` | 单次往返执行多个动作 |
| 截图 | `screenshot` | PNG 帧缓冲抓取 |
| 窗口调整 | `resize` | 视口尺寸 |

### 4.2 已在 D2 工具清单中定义的编辑器专属能力

> 详见 `docs/plan/kairos-editor-mcp/phase2-tools-catalog.md`

| 类别 | 工具（P0 MVP） |
|------|---------------|
| 项目管理 | `get_project_tree`, `get_asset_info`, `get_asset_registry` |
| 资产操作 | `select_asset`, `open_asset`, `create_asset`, `delete_asset`, `rename_asset` |
| Inspector | `inspect_asset`, `get_inspector_state` |
| 场景视图 | `get_scene_camera`, `camera_orbit`, `camera_zoom`, `scene_screenshot` |
| 控制台 | `get_console_logs`, `clear_console` |
| 编辑器 | `get_editor_state` |

### 4.3 D1 揭示的新增需求（D2 未覆盖）

| 新增需求 | 描述 | 影响 |
|---------|------|------|
| **编译启动** | Agent 通过 MCP 触发 `cargo build` + `cargo run`，启动/重启编辑器 | 需要新增 `cargo_build` 和 `cargo_run` 工具 |
| **崩溃检测** | MCP 检测编辑器进程崩溃，回报 backtrace 和最后状态 | 影响 MCP Server 的进程监控设计 |
| **操作日志追溯** | 每一步操作请求和响应需要可追溯（用于失败定位） | 影响 MCP Server 的日志设计 |

---

## 5. 非功能需求

| 需求 | 描述 | 优先级 |
|------|------|--------|
| **独立进程** | MCP Server 与编辑器分离，通过 TCP 通信 | 已确认 |
| **Agent 全流程自驱** | 从编译到运行到验证，全由 Agent 控制，不走 CI | 已确认 |
| **非多模态兼容** | 验证不依赖截图（截图可用但非必须），支持纯文本 Agent | 已确认 |
| **崩溃可恢复** | 编辑器崩溃不影响 MCP Server，Agent 可重启 | 已确认 |
| **模块独立** | 引擎状态通过中间层暴露，不破坏现有架构 | 已确认（D3 详规） |

---

## 6. 范围边界

### v1 范围内

- ✅ 基于 egui_mcp 的 Design A 架构（复用 UiServer + 新增 EditorTools）
- ✅ 所有 P0 工具（见 D2 工具清单 Phase 1）
- ✅ `cargo build` + `cargo run` 能力
- ✅ 崩溃检测与 backtrace 回报
- ✅ 操作-响应日志追溯
- ✅ 全 UI 树查询

### v2 范围外

- ❌ 3D 场景视图交互（GPU picking、场景点击）
- ❌ CI 自动触发
- ❌ 编辑器崩溃自动恢复
- ❌ 动态日志 hook（Agent 通过改源码 + 重新编译实现日志注入）
- ❌ 多窗口/多 Viewport 管理

---

## 7. 成功标准

一个典型的 Agent 开发-验证循环应该能做到：

1. Agent 修改源码 → `cargo_build` 成功
2. `cargo_run` 启动编辑器 → `attach` 连接成功
3. Agent 操作编辑器 → 每一步获得可追溯的响应
4. Agent 获取反馈（tree / logs / screenshot）→ 能判断验证通过或失败
5. 失败时 → Agent 能从操作日志中定位失败步骤，获取足够的上下文信息

---

## 8. 决策记录

| 决策 | 结论 | 来源 |
|------|------|------|
| 进/线程模型 | 独立进程 + TCP | 调研确认 |
| 架构模式 | Design A（复用 UiServer + 独立 ToolRouter） | 调研确认 |
| ECS 暴露 | 中间层，拒绝 Arc\<RwLock\<World>> | 用户决策 |
| 验证手段 | AccessKit 树 + Console 日志 + 截图（可选） | 用户决策 |
| 回归触发 | Agent 手动 | 用户决策 |
| 崩溃恢复 | 不自动恢复，Agent 自行重启 | 用户决策 |
| 日志注入 | 修改源码 + 重新编译 | 用户决策 |
| 与 kairos-test 关系 | MCP 替代废弃的 TOML 测试框架 | 用户决策 |
| 3D 场景交互 | v2 | 用户决策 |

---

## 9. 对后续 Phase 的影响

| 影响 | 关联 Ticket |
|------|------------|
| D2 工具清单需补充 `cargo_build`、`cargo_run`、崩溃检测工具 | D2（已完成，需修订） |
| D3 中间层需考虑编译触发和进程管理的接口 | D3 |
| D4 可观测性需设计操作日志追溯格式 | D4 |
| D6 事件循环需考虑编辑器重启后的重连时序 | D6 |
