# Editor UI 重写计划

> **配套**：[架构设计](./README.md) · [ADR 0008](../../adr/0008-editor-ui-architecture.md)
> **状态**：提案（实施中：docking / core / component 已落地，见 [panel-viewport-design.md](./panel-viewport-design.md) §6.1）
> **前置**：本计划假定采用"新 crate 并行、旧 UI 冻结、按面板迁移"的 strangler 路径，而不是一次性删库重写。

---

## 0. 总原则

1. **旧 UI 冻结**：`kairos_editor/src/ui/**` 停止加功能，只修阻塞 bug。新功能一律进新架构。
2. **每一步可交付**：每个 phase 结束时编辑器可运行、可手动验收；不允许"中间态不能启动"。
3. **先核心后视图，再先 widget 后面板**：domain/reducer/effect 先写并单测通过；然后先做纯外观 widget kit（Phase 0.5），最后才用 widget 拼面板。避免「先在面板里手绘、以后再抽」——那正是现状 17.6k 行 UI 的来源。
4. **单 crate 验证**：每 phase 结束跑 `cargo test-crate kairos_editor_core`（若有）+ `cargo test-crate kairos_editor` + `cargo check --workspace --all-targets`；不要跑 `cargo test-full`。
5. **验收以交互为准**：每个 phase 附手动 smoke 清单（打开/编辑/保存/关闭/重启），并逐步补 `egui_kittest` 交互测试。

---

## 1. 目标 crate 布局

```
kairos_editor_widgets/      新 crate（L0+L1 纯外观：只依赖 egui + kairos_color）
  src/theme.rs icon.rs atoms.rs chrome.rs rows.rs overlays.rs media.rs gallery/
  tests/                    headless egui 测试（无 GPU）
kairos_editor_core/         新 crate（纯领域）
  src/state.rs action.rs effect.rs reduce/ document.rs command.rs query.rs
kairos_editor_component/           新 crate（L2 功能组件：依赖 widgets + core，不依赖 host）
  src/frame.rs components/ view_state.rs docking/
kairos_editor/              现有 crate 收缩为 host + bin
  src/app.rs host.rs runtime.rs install.rs main.rs
  src/ui/**                 旧 UI：逐 phase 删除
```

**依赖图**（单向 + 编译期强制）：

```
kairos_editor_widgets      （leaf，只依赖 egui + kairos_color；widget 里写业务类型会编译失败）
kairos_editor_core         （leaf，纯领域；只允许 engine 纯数据类型）
         │                          │
         └──────► kairos_editor_component ◄┘
                        │
                  kairos_editor
```

widgets crate **不得**依赖 core/engine；ui crate **不得**依赖 host（不做 IO）。这两条由 Cargo 依赖图保证，并建议加 CI 断言（`cargo tree -p kairos_editor_widgets | grep kairos | grep -v kairos_color` 为空；`kairos_color` 是 UI 无关的叶子值 crate）。

core 允许依赖 `kairos_engine` 的**纯数据类型**（`Handle`、`Mesh`、`RenderState`、`TextureEdit` 等），禁止依赖 `egui`/`winit`/`wgpu` 的 API 与 `World` 操作。若后期想让 core 完全零引擎依赖，可把文档载荷换成 core 自己的 DTO，属于可延期决策。

---

## 2. Phase 计划

### Phase 0 — 骨架与领域核心（约 5-7 天）

**交付**
- 新 crate `kairos_editor_core`，含：`EditorState`、各领域 `Action`、`Effect`、`reduce()`、`EditorCore::dispatch`、`CommandRegistry`、`Document`/`EditTransaction` 骨架。
- 迁移纯逻辑：项目树操作（`project_path_tree` 的增删改查）、资产 registry 读写、watcher 事件应用，从 `ProjectWindow` 中拆出为 `ProjectState` + reducer/effect；现有 74 个测试按"领域测试"归位（数据结构测试继续留在原处或跟随迁移）。
- 旧 UI 不动；新 core 先以"旁路"方式编译（不接线），跑 `cargo test-crate kairos_editor_core`。
- ADR 0008 合入。

**验收**
- [ ] core 单测覆盖：项目树增删改、选中、文档打开/编辑/保存/回滚、命令 enabled、effect 构造。
- [ ] `cargo check --workspace --all-targets` 通过。
- [ ] 旧编辑器行为零变化。

**风险**：把 `ProjectWindow` 的逻辑硬搬到 core 时带入 `Arc<Mutex>`/plumbing——发现一处删一处，core 里出现 `Cell`/`Mutex`/`Receiver` 即为设计错误。

**估算校准**：合并原型 [`prototypes/editor-ui/`](../../../prototypes/editor-ui/) 的 `editor-core` 只做 5 个 slice × 6 个 action 域 + 异步 ticket + undo + 11 条命令，就约 1.6k 行 src + 0.5k 行测试（全 workspace 4.6k src + 1.7k tests），而它还没覆盖样式/状态栏/Play/Query/真实项目树。所以不要把 Phase 0 想成「半天骨架」：可运行的领域核心 + 测试大约 5-7 天，且这是后续所有阶段的依赖。

**原型已验证的模式**（照抄即可，不要重新发明）：
- reducer 纯函数 + 跨 slice 规则（选中资产同时改 selection/layout/loading 并发 Effect）；
- 关闭 dirty 文档 → 数据化 `Modal::Confirm`，确认 = `SaveThenClose`（保存完成后才关闭），取消 = `Dismiss`；
- 保存中继续编辑用 `PendingSave{ticket, content}` 保护，completion 只清对应内容的 dirty；
- 未知/陈旧 ticket 直接忽略；quit 有未保存文档先确认；
- `CommandRegistry` 的 enabled 谓词 + `fn(&EditorState) -> Action`（不要静态 Action 字段，否则出现 `SaveActive`/`UndoActive` 变体）；
- 实测教训：跨 slice 借用要"先申请 ticket 再操作 / read-then-write"，状态方法不能返回跨 slice 的长生命周期 `&mut`；异步 completion 与并发编辑的竞态编译器看不见，必须用类型化 `RequestSlot<T>` 表达。

### Phase 0.5 — Widget kit + Theme + Gallery（约 1-1.5 周，**先于面板迁移**）

> **原型已验证（2026-10-30，2026-11-09 扩展）**：合并原型 [`prototypes/editor-ui/`](../../../prototypes/editor-ui/) 在 egui 0.35 上跑通 **126 个测试**（125 headless + 1 doctest，5 个 crate）；除 widget kit 纯度、components 适配函数、真实 reducer/effect 闭环外，2026-11-09 又验证了 **Panel trait/Registry/Host**（面板不写进 `EditorApp`、新增面板零 core 改动）、**四槽 tab 停靠 + Console 只读 host 日志**，以及 **egui↔wgpu 视口接缝**（offscreen RTT→`TextureId`→egui image；orbit/pan/zoom/pick→`ViewportAction`→reducer→`Effect`→host camera/raycast，11 个 viewport slice 测试 + 35 个 camera/pick 测试 + 窗口 smoke 截图）。Phase 1/2 的「App shell/视口」从「待验证」变成「有可抄形状」，见 [panel-viewport-design.md](./panel-viewport-design.md)。

**交付**
- 新 crate `kairos_editor_widgets`（只依赖 egui + `kairos_color`）：`Theme` tokens（从现有 `Preferences/Styles/*.toml` 1:1 映射，保证视觉不变）、`IconId/IconProvider`、首版 20-25 个 widget（[widget-layer.md](./widget-layer.md) §2.2 的目录）、`gallery` 开发 bin。
- `kairos_editor_component` 骨架：`Frame`/`UiOut`/`ViewStates` + 一个 **vertical slice**：只迁 ProjectPanel 一页（验证 TreeView/ContextMenu/StatusBar/PanelChrome 是否够用）。
- widget 级 headless 测试（egui `Context::run` + 合成指针输入）；每个 widget 至少 smoke 一帧，交互控件有 click/emit 断言。

**验收**
- [ ] `cargo tree -p kairos_editor_widgets | grep kairos | grep -v kairos_color` 为空（widget 层零领域依赖；`kairos_color` 是 UI 无关的叶子值 crate）。
- [ ] gallery 在 headless 下跑通；widget 测试无需 GPU/window。
- [ ] Theme 映射后，ProjectPanel 外观与旧 UI 肉眼一致。
- [ ] widget API 经一次 review 后再铺开（避免 props 过早定型）。

**风险**：widget props/Output API 过早泛化（避免自造组件框架）；如果 Phase 0.5 结束发现 TreeView/PropertyRow 抽象撑不住 ProjectPanel/Inspector，宁可改 API 也不要在面板里绕过去。这一阶段的质量决定后续 10 个面板的迁移成本。

### Phase 1 — App shell：项目/层级/Inspector/控制台（约 1.5-2 周）

**交付**
- `kairos_editor` 新增 `app.rs`（编排 dispatch/effect/draw）+ `host.rs`（`EffectExecutor` 实现：IO、tokio、原生对话框）；`runtime.rs` 改为调用 `App::frame`。
- `kairos_editor_component`：`Frame`/`UiOut`/`Panel` trait、`PanelInstanceId`、dock 迁移（`DockState<PanelInstanceId>`）、项目/层级/Inspector/控制台四个面板的只读绘制版。
- 选择成为全局服务；Inspector 从"选中资产 → 打开 Document"改为读 `SelectionState` + `DocumentsState`。
- 用编译期 feature（如 `new_editor_ui`）或环境变量切换新旧 UI；默认先旧，验收后默认新。

**验收**
- [ ] 打开项目树、选中资产、Inspector 显示（text/toml/material 只读展示即可）。
- [ ] 标签页开关/拖拽/dock 正常；关闭重开后布局不崩。
- [ ] 项目 watcher 外部变更仍能刷新树。
- [ ] 旧 `Message` 中项目/选择/层级相关变体不再被新 UI 使用。

**风险**：dock 迁移别顺手重写；只改泛型参数与 `TabDrawer` 签名。另外，Phase 1 的第一个可交付物建议是**一条 headless 交互测试**（`egui_kittest`：点击删除 → 确认 modal → 确认 → 节点消失），先证明 `Frame`/`UiOut`/`ViewState` 的借用与多 pass 语义可工作，再铺开移植面板。

### Phase 2 — 场景/游戏视口与异步回绑（约 1 周）

**交付**
- `ViewportState` + `Effect::CreateViewport/DestroyViewport/SyncViewport`；host 侧异步创建 render target，完成事件走 `EffectOutcome`。
- 场景相机输入从消息改为 `Action::Viewport(CameraInput { .. })` → reducer 更新输入态/直接写 `SceneViewInput` effect；保留 `camera.rs` 现有 system 不变。
- `SceneWindow`/`GameWindow` 的 `render()` 逻辑迁移为 `App::render_viewports`（按 viewport 注册表，而非 drawer 顺序）。
- 删除 `Registe*ViewBind`/`TryReceTextureId`/`Update*WindowSize` 等消息。

**验收**
- [ ] 场景视口渲染、相机 orbit/zoom/fly、关闭后不再渲染/不泄漏纹理。
- [ ] 游戏视口尺寸随 dock 变化更新。
- [ ] 材质/网格 3D 预览（若已迁移）同样走 viewport 注册表。

### Phase 3 — 文档编辑：保存/放弃/撤销 + 模态（约 2 周）

**交付**
- `Document` 覆盖 text/toml/material/texture/shader；load/save/revert/undo/redo 全链路。
- 材质编辑从 edit-in-place 改为"文档缓冲 + preview effect"；保存失败进 `ModalState::Error`；关闭有未保存修改 → `ModalState::Confirm`（纯数据模态）。
- `kairos_editor_component` 的 text/toml/material/texture/shader 编辑器面板（可从旧 `inspector/*.rs` 迁移绘制代码，但改为 `&Frame + &mut UiOut` 签名）。
- 删除 `Box<dyn Inspector>`、`InspectorCreater`、`get_inspector_mut`、`on_exit` 对话框回调、`Arc<Mutex<...>>` 快照消息。

**验收**
- [ ] 编辑 → 预览实时更新；保存 → dirty 清除、重开一致；放弃 → 恢复上次保存；关闭 → 正确确认流程。
- [ ] 撤销/重做覆盖文本与材质字段编辑。
- [ ] 保存失败（只读文件/序列化错误）有可见错误而非仅 log。

### Phase 4 — 命令系统 + 样式/偏好 + 布局持久化（约 1 周）

**交付**
- `CommandRegistry` 接入工具栏、菜单、快捷键、命令面板（先做工具栏+快捷键即可）。
- `StyleState` + `StyleSchema` 注册表；偏好设置页由 schema 生成；`get_style_fileds`/`update_style` 删除。
- 布局持久化（`Effect::PersistLayout`）。
- MCP 桥接：`Query` 快照 + `EditorCommand` → `Action`；替代 `docs/plan/kairos-editor-mcp/phase3` 的 `Drawer::QueryHandler` 中间层。

**验收**
- [ ] 工具栏/快捷键/命令触发同一 `Command`，`enabled` 状态正确。
- [ ] 样式修改即时生效并落盘；重启保持。
- [ ] 布局重启保持。
- [ ] MCP 查询选中资产/项目树/日志可用。

### Phase 5 — 收尾（约 1 周，可并行）

**交付**
- 删除 `kairos_editor/src/ui/**` 中旧文件与 `Messager`/`Message`/`Drawer`/`UIReader`（保留 dock 控件与通用绘制工具到 `kairos_editor_component`）。
- 场景编辑文档/实体选择 + 事务撤销（可选，取决于 §7.3 决策）。
- `egui_kittest` 交互测试补齐关键路径；`docs/research/editor-ui-*` 与 ADR 状态更新。

**验收**
- [ ] `grep -rn "Messager\|Message::" kairos_editor/src` 无 UI 总线残留。
- [ ] 交互测试覆盖：打开资产、编辑保存、关闭确认、撤销。
- [ ] `cargo test-crate kairos_editor` 绿；手动 smoke 清单全过。

---

## 3. 工作量与排期（估算，单人）

| Phase | 内容 | 估算 |
|---|---|---|
| 0 | core 骨架 + 领域迁移 | 5-7 天 |
| 0.5 | widget kit + Theme + gallery + ProjectPanel vertical slice | 1-1.5 周 |
| 1 | App shell + 4 面板 + dock | 1.5-2 周 |
| 2 | 视口 + 异步 | 1 周 |
| 3 | 文档编辑 + undo + 模态 | 2 周 |
| 4 | 命令 + 样式 + MCP | 1 周 |
| 5 | 收尾 + 测试 | 1 周 |
| **合计** | | **8.5-10.5 周（单人）** |

两人并行（core+widgets/gallery 与 host+viewport 分工）可压到约 5-6 周；主要串行瓶颈是 Phase 0 的领域设计 + Phase 0.5 的 widget API。**估算的前提**是保留现有 dock、字体/样式资源与资产加载，不重写引擎侧。

> 本阶段（2026-10-30 方案 + 原型；2026-11-09 面板/视口重构）只做**方案 + 原型**：合并原型见 [`prototypes/editor-ui/`](../../../prototypes/editor-ui/)（一个 workspace 五个 crate：core + ui-widgets + editor-components + editor-viewport + editor-app；120 测试，含 7 个 full-stack、14 个 panel host、11 个 viewport slice，窗口 smoke 截图）；生产实施从 Phase 0 起，需作者拍板后再排期。

---

## 4. 旧模块 → 新模块映射

| 现状 | 去向 |
|---|---|
| 各 `*Style` TOML + `global_styles.rs` + `ui_style_fields.rs` | `kairos_editor_widgets::Theme`（tokens）+ `core::StyleState`（值）+ 偏好设置 schema |
| 各面板里手写的按钮/行/树/菜单/模态/下拉绘制 | `kairos_editor_widgets` 的 atoms/compounds（每类一份实现） |
| `egui_ext.rs`（`UiExt` 通用 helper） | 迁入 widgets（`Ui` 扩展 trait / helper） |
| `ui.rs` `Message`/`Messager`/`UIReader`/`Context::handle` | 删除；由 `core::Action`/`reduce` + `App` 替代 |
| `ui.rs` `Drawer` trait | 删除；由 `ui::Panel` trait + `PanelInstanceId` 替代 |
| `project_window/*` | `core::ProjectState` + `ui::panels::project`（绘制保留） |
| `hierarchy_window.rs` | 并入 project 面板（hierarchy/content 已是 project 的两个 panel，见现有代码） |
| `inspector_window.rs` + `inspector/*` | `core::DocumentsState` + `ui::panels::inspector` + 各文档编辑器 |
| `scene_window/*` | `core::ViewportState` + `ui::panels::scene` + `App::render_viewports` + `camera.rs`（保留） |
| `game_window.rs` | 同上（game viewport） |
| `preferences_window.rs` + `ui_style_fields.rs` | `core::StyleState` + `ui::style` + 偏好 schema |
| `console_window.rs` | `core::ConsoleState`（读 `Log` 快照） |
| `layout.rs`（假抽屉） | 删除；`LayoutState` + `PanelInstanceId` + dock 直接构造 |
| `dialog.rs`（`Cell<FnOnce>` 回调） | `core::ModalState` + `Effect::NativeDialog` |
| `docking_tab/*`（2.5k 行） | 保留为 ui 层组件；改泛型参数与 `TabDrawer` 签名 |
| `egui_ext.rs`/`global_styles.rs`/`paths.rs`/`ide_detection.rs`/`drag.rs` | 原样迁入 `kairos_editor_component`（低风险） |
| `camera.rs`/`asset_registry.rs`/`project_path_tree.rs`/`project_watcher.rs`/`editor_assets/*`/`syntax.rs` | 保留；领域逻辑逐步上移 core |

---

## 5. 质量门（每 phase 必过）

```sh
cargo test-crate kairos_editor_widgets   # 无 GPU，widget headless 测试
cargo test-crate kairos_editor_core      # reducer/领域测试
cargo test-crate kairos_editor_component        # 组件/适配函数测试
cargo test-crate kairos_editor
cargo check --workspace --all-targets
```

**新增硬约束**（建议加进 CI/AGENTS.md）：
- `kairos_editor_core` 不得 `use egui` / `use winit` / `use wgpu` / `use kairos_ecs::world::World`。
- `kairos_editor_widgets` 只能依赖 egui + `kairos_color`（`cargo tree -p kairos_editor_widgets | grep kairos | grep -v kairos_color` 必须为空）；widget 代码里禁止 `Cell/RefCell/Mutex/unsafe` 与任何领域类型。
- `kairos_editor_component` 不得依赖 `kairos_editor`（host），组件里禁止 IO / `&mut World` / `std::fs`。
- widget 的语义状态（选中/展开/值）必须由 props 传入；组件会话态放 `ViewStates`；egui `Memory` 只放纯瞬态。
- `kairos_editor_component` 不得 `use crate::host`（UI 不能直接做 IO）。
- 面板代码不得出现 `Cell`/`RefCell`/`Mutex`/`Rc`。

---

## 6. 回滚与并存策略

- **feature flag**：`new_editor_ui`（编译期）或 `KAIROS_NEW_UI=1`（运行期）切换新旧入口；旧入口保持可用直到 Phase 5。
- 共享数据（项目树、资产 registry）在迁移期由新旧两套读同一份磁盘数据；**避免双写**，写路径优先在新架构（旧 UI 只读），必要时临时保留旧写入路径。
- 每个 phase 独立可回滚（新代码在新 crate/模块，旧代码不动）。

---

## 7. 已知风险

| 风险 | 缓解 |
|---|---|
| Action/Effect 设计在 Phase 3 才发现缺能力（如 save-as、导入） | Phase 0 先把"文档生命周期"完整走一遍 text 类型再批量迁移 |
| dock 改造引发输入/拖拽回归 | Phase 1 只改泛型与签名；交互测试覆盖 tab 拖拽 |
| 预览/渲染与文档解耦后时序错乱 | 固定帧循环阶段（§3.3），viewport 注册表 + RequestId 校验 |
| 团队习惯回潮（视图里发消息/改状态） | 编译期隔离 + review 检查 + AGENTS.md 约束 |
| 估算低估 | 每个 phase 独立验收，Phase 1 结束时重新评估剩余工作量 |
| core 没有项目树/父寻址（合并原型暴露） | Phase 0 前定 `ProjectState` 结构：parent 用 path 还是稳定 `FolderId`；不要让组件长期用 FNV(path) 临时映射 |
| `DocumentAction::Open` 直接携带 content，而 ADR 要求异步 `LoadDocument` | Phase 0 就把 Open 改成「effect + completion」，否则双击打开的永远是占位内容 |
| widget 的 `u64` row id 与领域 `AssetId` 不对齐 | widget kit 接受 opaque id；组件维护 `NodeId -> AssetId` 映射，不要用 hash 权宜 |
| 拖拽类交互（slider/树拖拽）headless 测试成本高 | harness 必须支持 drag/double_click；或引入 vendor 的 `egui_kittest` |
| toolbar 直接发 Action，未走 `CommandRegistry` | Phase 4 前统一 toolbar item id == `CommandId`，菜单/快捷键/命令面板/MCP 同源 |
