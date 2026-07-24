# Phase 3: 中间层架构设计 — `kairos_editor_mcp`

**日期**: 2026-07-24  
**父Ticket**: [#71](https://github.com/WhitePetal/KairosEngine/issues/71)  
**关联Ticket**: [D3](https://github.com/WhitePetal/KairosEngine/issues/75)

---

## 1. 问题定义

`kairos_editor_mcp`（独立进程）需要查询和操作 KairosEngine 编辑器的内部状态（ECS 实体、资产注册表、项目树、选中状态、控制台日志等），但：

1. **状态分散**：编辑器状态散落在 `KairosEngine` → `ui_context` → 各 `Drawer`（`ProjectWindow`、`InspectorWindow`、`SceneWindow` 等）内部，没有统一的"编辑器状态"入口
2. **线程隔离**：MCP Server 在独立进程/线程中，不能直接访问编辑器内存
3. **渲染循环同步**：所有状态读写必须在 egui render loop 中执行
4. **架构约束**：不使用 `Arc<RwLock<World>>`，模块独立，可测试

---

## 2. 当前状态分布

```
KairosEngine
├── engine: Engine
│   ├── world: World              ← ECS 实体/组件
│   ├── assets_server             ← 资产加载系统 (mpsc 异步)
│   └── time: Time                ← 帧时间
├── game: KairosGame              ← 游戏逻辑（场景初始化）
├── ui_context: ui::Context       ← Drawer 集合
│   ├── ProjectWindow.model
│   │   ├── asset_registry        ← GUID ↔ Path 双向映射
│   │   ├── project_path_graph    ← 文件系统树 (petgraph)
│   │   └── selected_node         ← 当前选中节点
│   ├── InspectorWindow.model
│   │   └── selected              ← 当前 Inspector 内容
│   ├── SceneWindow               ← 轨道相机 (v2 交互)
│   ├── GameWindow                ← 游戏视口
│   ├── ConsoleWindow             ← (stub)
│   ├── ToolBar                   ← 播放/停止/保存按钮
│   └── DockingTab (DockState)   ← Tab 布局状态
└── log: Log                      ← 控制台日志缓冲
```

---

## 3. 设计方案：Trait + Channel 双层架构

### 3.1 总览

```
┌──────────────────────────────────────────────────────────────┐
│  MCP Server (kairos_editor_mcp)                               │
│                                                               │
│  EditorTools                                                 │
│  ├── get_project_tree() ──────────┐                           │
│  ├── get_console_logs() ──────────┤                           │
│  ├── create_asset() ──────────────┤                           │
│  └── ...                          │                           │
│                                    ▼                           │
│  ┌─────────────────────────────────────────────┐              │
│  │  EditorChannel                              │  ← Trait    │
│  │  fn query(EditorQuery) → EditorResponse     │    (API)    │
│  │  fn command(EditorCommand) → EditorResponse │              │
│  └──────────────────┬──────────────────────────┘              │
│                     │                                         │
└─────────────────────┼─────────────────────────────────────────┘
                      │ mpsc::channel (同进程) 或 TCP (跨进程)
┌─────────────────────┼─────────────────────────────────────────┐
│  KairosEngine Editor                                          │
│                     │                                         │
│  ┌──────────────────▼──────────────────────────┐              │
│  │  EditorChannelPlugin                        │  ← Channel  │
│  │  rx: mpsc::Receiver<ChannelMessage>         │    impl     │
│  │                                              │              │
│  │  process(&mut self, engine, ui_ctx, log) {  │              │
│  │    while let Ok(msg) = self.rx.try_recv() { │              │
│  │      match msg {                            │              │
│  │        Query(q, reply) → dispatch_query()   │              │
│  │        Command(c, reply) → dispatch_cmd()   │              │
│  │      }                                      │              │
│  │    }                                        │              │
│  │  }                                          │              │
│  └──────────────────────────────────────────────┘              │
│                                                               │
│  dispatch_query(q, engine, ui_ctx, log):                      │
│    for drawer in ui_ctx.drawers():                            │
│      if let Some(handler) = drawer.as_query_handler() {       │
│        if let Some(resp) = handler.handle(q, engine, log) {   │
│          return resp;  // first handler wins                  │
│        }                                                      │
│      }                                                        │
│    return EditorResponse::Error("unhandled query")             │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 核心抽象

#### 3.2.1 `QueryHandler` Trait

每个需要暴露状态的 Drawer 选择性实现：

```rust
/// 无副作用的状态查询。Drawer 收到自己能处理的查询时返回 Some，
/// 否则返回 None（让下一个 Drawer 尝试）。
pub trait QueryHandler {
    fn handle_query(
        &self,
        query: &EditorQuery,
        engine: &Engine,
        log: &Log,
    ) -> Option<EditorResponse>;
}
```

在 `Drawer` trait 上增加默认方法，保持向后兼容：

```rust
pub trait Drawer: Any {
    // ... 现有方法 ...

    /// 可选：为此 Drawer 提供状态查询能力
    fn as_query_handler(&self) -> Option<&dyn QueryHandler> { None }
}
```

#### 3.2.2 `EditorQuery` → `EditorResponse`

每个查询一个 enum variant，每个响应一个 enum variant，类型安全：

```
EditorQuery (请求)                    EditorResponse (响应)
─────────────────────────────────     ─────────────────────────────────
GetProjectTree                        ProjectTree { root: TreeNode }
GetAssetInfo { guid | path }         AssetInfo { guid, name, path, kind }
GetAssetRegistry { kind?, search? }  AssetRegistry { assets: [Entry] }
GetSelectedAsset                      SelectedAsset(Option<AssetInfo>)
GetInspectorState                     InspectorState { selected, kind }
GetSceneCamera                        SceneCamera(CameraState)
GetGameState                          GameState { viewport, running }
GetConsoleLogs { level?, pattern? }   ConsoleLogs([LogEntry])
GetEditorState                        EditorState { tabs, selection, ... }
ClearConsole                          Ok
```

#### 3.2.3 `EditorCommand` (副作用操作)

对于需要 `&mut` 访问的操作（创建资产、删除、重命名、选中、相机操作等），通过 Command 通道：

```
EditorCommand (请求)                  EditorResponse (响应)
─────────────────────────────────     ─────────────────────────────────
SelectAsset { guid | path }          Ok + SelectedAsset
CreateAsset { parent_path, name, kind } Ok + created AssetInfo
DeleteAsset { guid | path }          Ok + deleted info
RenameAsset { guid, new_name }       Ok + renamed info
CameraOrbit { dx, dy, dt? }          Ok + yaw/pitch
CameraZoom { delta, dt? }            Ok + distance
CameraFly { right?, forward?, dt? }  Ok + pivot
```

### 3.3 Drawer → Query 映射

| Drawer | 实现的 QueryHandler 查询 | 原因 |
|--------|------------------------|------|
| `ProjectWindow` | `GetProjectTree`, `GetAssetInfo`, `GetAssetRegistry`, `GetSelectedAsset` | 持有 `ProjectPathGraph` + `AssetRegistry` + 选中状态 |
| `InspectorWindow` | `GetInspectorState` | 持有当前选中的 Inspector |
| `SceneWindow` | `GetSceneCamera` | 持有轨道相机 |
| `GameWindow` | `GetGameState` | 持有游戏视口状态 |
| `ToolBar` | — (无查询，只有 command) | 播放/停止是副作用操作 |
| `Log` (非 Drawer) | `GetConsoleLogs` | 全局日志缓冲 |

### 3.4 渲染循环中的集成点

当前渲染循环（`KairosEditorRuntime::redraw`）：

```rust
self.egui_ctx.run_ui(raw_input, |ui| {
    graphics_commands.append(&mut self.kairos_engine.render_ui());    // 1. pre-frame
    self.kairos_engine.handle_ui(ui);                                   // 2. handle events
    self.kairos_engine.handle_asset_server();                           // 3. process assets
    self.kairos_engine.draw_ui(ui);                                     // 4. draw
});
```

中间层的处理时机有两种选择：

| 时机 | 位置 | 优点 | 缺点 |
|------|------|------|------|
| `handle_ui` 之前 | 在输入处理前处理 command | 状态变更是"本帧"生效 | MCP 查询看到的是上一帧的旧状态 |
| `draw_ui` 之后 | 在渲染完成后处理 | 查询返回最新渲染结果 | command 要下一帧才生效 |
| **推荐：拆分** | Query 在 draw 后，Command 在 handle 前 | 各得其所 | 需要两个 hook 点 |

推荐方案：

```rust
self.egui_ctx.run_ui(raw_input, |ui| {
    // === 1. Pre-frame: process MCP commands (mutations) ===
    self.editor_channel.process_commands(&mut self.kairos_engine);

    graphics_commands.append(&mut self.kairos_engine.render_ui());
    self.kairos_engine.handle_ui(ui);
    self.kairos_engine.handle_asset_server();
    self.kairos_engine.draw_ui(ui);

    // === 2. Post-frame: process MCP queries (reads latest state) ===
    self.editor_channel.process_queries(&self.kairos_engine);
});
```

这样确保：
- **Commands**：修改后本帧渲染就反映变化
- **Queries**：读取的是最新渲染后的状态  
- **最大延迟**：1 帧（16ms @ 60fps）

### 3.5 序列化策略：Snapshot 模式

MCP 不持有引擎内部引用。所有查询结果在 dispatch 时**立即序列化**为纯数据 snapshot：

```rust
// 在引擎 render loop 中（持有 &self 访问权）
fn dispatch_query(query: &EditorQuery, engine: &Engine, ui_ctx: &Context, log: &Log) -> EditorResponse {
    match query {
        EditorQuery::GetProjectTree => {
            // 读取 project_path_graph → 转为 TreeNode 树 → 返回副本
            let root = ui_ctx.project_window()
                .map(|pw| pw.build_tree_snapshot())
                .unwrap_or_default();
            EditorResponse::ProjectTree { root }
        }
        EditorQuery::GetConsoleLogs { level, pattern, after, limit } => {
            // 读取 log 缓冲 → 过滤 → 转为 Vec<LogEntry>
            let entries = log.filter_entries(level, pattern, *after, *limit);
            EditorResponse::ConsoleLogs(entries)
        }
        // ...
    }
}
```

**为什么这样做**：
- MCP 拿到的数据与引擎生命周期解耦
- 不需要 `Arc`/`RwLock`/生命周期标注
- 序列化后的数据可直接通过 mpsc channel 发送

---

## 4. 传输层设计

### 4.1 同进程模式（开发/测试）

```rust
use tokio::sync::mpsc;

// 编辑器端
let (tx, rx) = mpsc::unbounded_channel::<ChannelMessage>();
let plugin = EditorChannelPlugin::new(rx);
// ... 在 render loop 中调用 plugin.process()

// MCP 端
let channel = EditorChannel::new(tx);
let response = channel.query(EditorQuery::GetProjectTree).await;
```

### 4.2 跨进程模式（生产）

复用 egui_mcp 的 `Bridge` + `Transport` 抽象：

```rust
// 在 egui_mcp 的 Bridge 基础上增加编辑器工具
struct KairosBridge {
    egui_bridge: Bridge,              // 通用 egui 工具
    editor_channel: EditorChannel,    // 编辑器专属工具
}
```

编辑器专属查询通过独立的 TCP 端口或通过扩展现有的 `egui_inspection` 协议传输。

### 4.3 `EditorChannel` 接口

```rust
/// MCP 端持有的编辑器通信句柄
pub struct EditorChannel {
    sender: mpsc::UnboundedSender<ChannelMessage>,
}

impl EditorChannel {
    pub async fn query(&self, query: EditorQuery) -> EditorResponse {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ChannelMessage::Query(query, tx)).ok()?;
        rx.await.unwrap_or(EditorResponse::Error("channel closed".into()))
    }

    pub async fn command(&self, cmd: EditorCommand) -> EditorResponse {
        let (tx, rx) = oneshot::channel();
        self.sender.send(ChannelMessage::Command(cmd, tx)).ok()?;
        rx.await.unwrap_or(EditorResponse::Error("channel closed".into()))
    }
}
```

---

## 5. 可测试性设计

### 5.1 Mock QueryHandler

```rust
struct MockQueryHandler {
    project_tree: Option<EditorResponse>,
    selected_asset: Option<EditorResponse>,
}

impl QueryHandler for MockQueryHandler {
    fn handle_query(&self, query: &EditorQuery, _: &Engine, _: &Log) -> Option<EditorResponse> {
        match query {
            EditorQuery::GetProjectTree => self.project_tree.clone(),
            EditorQuery::GetSelectedAsset => self.selected_asset.clone(),
            _ => None,
        }
    }
}
```

### 5.2 单元测试模式

```rust
#[test]
fn test_get_project_tree() {
    let (tx, rx) = mpsc::unbounded_channel();
    let channel = EditorChannel::new(tx);
    let plugin = EditorChannelPlugin::new(rx);

    // 模拟一个 mock handler
    plugin.add_handler(Box::new(MockQueryHandler {
        project_tree: Some(EditorResponse::ProjectTree { ... }),
        selected_asset: None,
    }));

    // MCP 端发起查询
    let resp = channel.query(EditorQuery::GetProjectTree);
    assert!(matches!(resp, EditorResponse::ProjectTree { .. }));
}
```

---

## 6. 对现有架构的改动

| 改动 | 文件 | 影响 |
|------|------|------|
| 新增 `Drawer::as_query_handler()` | `ui.rs` (Drawer trait) | 向后兼容（默认返回 None），不影响现有 Drawer |
| `ProjectWindow` impl `QueryHandler` | `project_window.rs` | 新增 trait impl + 序列化方法 |
| `InspectorWindow` impl `QueryHandler` | `inspector_window.rs` | 新增 trait impl |
| `SceneWindow` impl `QueryHandler` | `scene_window.rs` | 新增 trait impl（v2 camera 查询） |
| 新增 `EditorChannelPlugin` | `ui/editor_channel.rs` (新文件) | 独立模块，零耦合 |
| `KairosEngine` 集成 plugin | `kairos_editor.rs` | 新增一个字段 + 一行 `process()` 调用 |
| 新增 `EditorChannel` (MCP 端) | `kairos_editor_mcp` crate | 新 crate 依赖 |

**不修改的部分**：
- ❌ 不修改 ECS `World` 内部结构
- ❌ 不修改 `AssetRegistry` 或 `AssetsServer` 内部
- ❌ 不修改现有 Drawer trait 方法签名（除新增一个默认方法）
- ❌ 不修改渲染循环结构（只在前后插入 hook）

---

## 7. 与 egui_mcp 的关系

```
kairos_editor_mcp
├── UiServer (from egui_mcp)     ← 通用 egui 工具 (click, query_tree, screenshot, ...)
│   └── Bridge → egui_inspection (TCP :5719)
│
├── EditorTools                  ← 编辑器专属工具 (get_project_tree, create_asset, ...)
│   └── EditorChannel → EditorChannelPlugin (mpsc / TCP)
│
└── Server (rmcp)                ← MCP 协议生命周期
    └── 合并两个 ToolRouter
```

两条通道在 `kairos_editor_mcp` 的 `Server` 层合并为统一的工具集，Agent 看到的是单一的 MCP Server。

---

## 8. 设计决策

| 决策 | 结论 | 理由 |
|------|------|------|
| 架构模式 | Trait + Channel 双层 | 命令通道处理传输，Trait 定义 API 边界 |
| 查询粒度 | 每查询一个 enum variant | 类型安全，编译期检查 |
| Drawer 暴露方式 | `QueryHandler` trait + `as_query_handler()` 默认方法 | 不破坏现有 Drawer，可选实现 |
| 渲染循环集成 | Commands 在 draw 前，Queries 在 draw 后 | 命令本帧生效，查询读最新状态 |
| 数据传递 | Snapshot 模式（序列化副本） | 生命周期解耦，无需锁 |
| 跨进程传输 | 复用 egui_mcp Transport trait | 统一传输抽象 |

---

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Channel 满/阻塞 | 使用 `unbounded_channel`，agent 调用量可控（~10次/循环） |
| 查询未覆盖 | 新查询只需加 enum variant + 一个 match arm，成本低 |
| Drawer 访问不到（私有字段） | 通过 `QueryHandler` 在 Drawer 源码中实现，内部字段天然可访问 |
| 死锁（oneshot 永远不回） | 增加超时机制；如果编辑器崩溃，channel 自动 drop，oneshot 返回 error |
