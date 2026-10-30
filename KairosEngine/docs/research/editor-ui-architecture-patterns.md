# 成熟引擎/编辑器 UI 架构模式调研

> **Date:** 2026-10-30
> **Status:** Complete（供 Lead 设计文档引用；本调研不阅读 KairosEngine 源码）
> **Scope:** Unreal Slate、Godot 4.x、Unity 6.x、Blender 4.x/5.x、VS Code、JetBrains Platform、Zed/GPUI、Rerun Viewer，外加 Bevy/egui 生态的 Rust 侧参考。
> **方法:** 一手资料：官方文档、官方 API reference、引擎源码（GitHub 镜像 / 官方仓库 raw 文件）、项目内设计文档。每条非显然结论旁附 URL。个别 Unreal/Unity 文档站是 SPA（正文由 JS 渲染），引用的是其官方 API 页面 URL；机制层面的结论来自多个独立来源交叉印证。文末列出未验证/时效性限制。

---

## 0. 结论先行（TL;DR）

1. **所有成熟编辑器都把"用户意图"建模为数据（command / action / operator），而不是把 UI 回调直接连到数据修改。** 区别只在于数据载体的类型强度：Unreal 的 C++ 命令对象、VS Code 的字符串 command id、Zed 的 Rust 类型化 action、Godot 的 Callable + `EditorCommandPalette` 注册、Rerun 的 `SystemCommand` enum。**类型化且可序列化的 action 是 Rust 侧的最佳落点**（Zed 是直接证据）。
2. **状态所有权高度集中，视图高度无状态。** Zed 的 App 拥有全部 Entity（[GPUI ownership 文档](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs)）；Rerun 的 app state 只允许通过 command 写（[command_sender.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/command_sender.rs)）；Blender 的 UI 由 RNA 元数据渲染（[RNA dev doc](https://developer.blender.org/docs/features/core/rna/)）。egui 的 immediate mode 恰好放大"视图无状态"这一优点。
3. **Selection 是一等应用服务（app-level service），不是某个面板的私有字段。** Godot 有 `EditorSelection` 单例（[EditorSelection.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorSelection.xml)）；Unity 有静态 `Selection`（[Selection.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/Selection/Selection.cs)）；Rerun 有双缓冲的 `ApplicationSelectionState`（[selection_state.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs)）；Zed 通过 `Workspace/Project/Editor` 实体 + 订阅传播（[workspace.rs](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/workspace.rs)）。
4. **Inspector 必须是 "属性模型 + 编辑器注册表"，而不是 "每种资产一个 inspector"。** Unity `SerializedObject` 的源码注释直接写明它是 "generic way that automatically handles undo and styling UI"（[SerializedObject.bindings.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/SerializedObject.bindings.cs)）；Rerun 用 `ComponentUiRegistry` + `DataUi`（[component_ui_registry.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/component_ui_registry.rs)、[data_ui](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_data_ui/src/lib.rs)）；Unreal 用 `FPropertyEditorModule` 的 customization 注册（[Details Panel Customization](https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customization-in-unreal-engine)）。
5. **Undo 是"按文档（per-document）的事务栈"，不是全局的 UI 撤销。** Godot 把历史按场景 tab 分成不同 history（[EditorUndoRedoManager.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorUndoRedoManager.xml)）；Blender 明确"通常只存数据、不存 UI 变更"（[Undo System](https://developer.blender.org/docs/features/core/undo/)）；Zed 把文本 undo 与 selection undo 分成两组 action（[actions.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/actions.rs)）。
6. **布局（layout）是数据，不是窗口的副作用。** Rerun 的 `ViewportBlueprint` 里直接存 `egui_tiles::Tree<ViewId>`，并在帧末统一应用变更（[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）；VS Code 在 `onWillSaveState` 里保存 workbench grid 状态（[layout.ts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/layout.ts)）；Godot 用 `EditorDock` + `EditorDockManager` + `LAYOUT_SAVE/LAYOUT_DEFAULT` 菜单（[editor_dock.h](https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock.h)、[editor_node.h](https://github.com/godotengine/godot/blob/master/editor/editor_node.h)）。
7. **插件边界的本质是"注册表 + 上下文"，不是"改中心文件"。** VS Code 的 contribution points（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）、Godot 的 `EditorPlugin` 扩展点（[EditorPlugin.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorPlugin.xml)）、JetBrains 的 AnAction/DataContext（[action-system](https://plugins.jetbrains.com/docs/intellij/action-system.html)）、Rerun 的 `ViewClassRegistry`/`ComponentUiRegistry` 都是同一模式。
8. **对 kairos_editor 的直接结论：** 用 `Action` 数据 + `ActionRegistry` 取代 god `Message` enum 和中心 `match`；用单一 `EditorStore`（分层子 store）取代让 `Drawer` struct 持有业务状态；把 selection、layout、inspector、undo、task 全部提升为 store 中的一等服务；视图退化为"读 store + 发 action"的纯函数；Rust 侧用 trait object 注册表 + 类型化 op 实现。详见第 9、10 节。

---

## 1. 总览：成熟编辑器的共同分层

所有被调研的编辑器都能归入同一张分层图（名称各异，职责一致）：

```
┌──────────────────────────────────────────────────────────────────────┐
│  Views / Panels / Widgets（无业务状态，只读快照，产出 intent）          │
│  Godot Control+EditorPlugin UI, Unity VisualElement/IMGUI,            │
│  Slate SWidget, egui Ui, GPUI Element                                  │
├──────────────────────────────────────────────────────────────────────┤
│  ViewModels / Presenters（把文档模型投影成 UI 需要的只读数据）           │
│  e.g. Rerun StoreViewContext + views, Godot EditorInspector          │
├──────────────────────────────────────────────────────────────────────┤
│  Application State / Store（唯一所有者：documents, selection, layout,  │
│  tasks, undo stacks；所有写入经过 command/dispatcher）                  │
│  Zed App+Entities, Rerun AppState, Unity EditorWindow 状态,           │
│  Blender Main/wmWindowManager, Unreal GEditor                        │
├──────────────────────────────────────────────────────────────────────┤
│  Commands / Actions / Operators（数据化的意图；可绑定快捷键/菜单/        │
│  palette；可序列化则还能 record/replay）                               │
│  FUICommandList, VS Code CommandsRegistry, Zed Action,                │
│  Rerun SystemCommand, Blender operator, Godot UndoRedo 事务           │
├──────────────────────────────────────────────────────────────────────┤
│  Model / Documents（资产、场景、prefab、buffer；dirty/版本/事件）        │
│  Unity SerializedObject, Unreal UObject, Godot Resource/Node,         │
│  Zed Buffer/MultiBuffer, Rerun EntityDb/Blueprint store               │
├──────────────────────────────────────────────────────────────────────┤
│  Effects / Tasks（线程池、导入、IO、网络；进度/取消/事件回传）            │
│  JetBrains ProgressManager, Bevy AssetServer, Unreal SlowTask,        │
│  Rerun command queue at frame end                                     │
└──────────────────────────────────────────────────────────────────────┘
```

几条跨引擎不变量：

| 不变量 | 证据 |
|---|---|
| UI 读取状态、发出意图，**不直接改模型** | Rerun：selection 只能通过 `SystemCommand::set_selection` 写（[selection_state.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs)）；Zed：所有修改走 `Entity::update` + 事件（[app/context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)） |
| 命令/动作可绑定多种入口（菜单、快捷键、palette、脚本） | VS Code `contributes.commands/menus/keybindings`（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）；Blender operator 可来自按钮、菜单、搜索（[Operators manual](https://docs.blender.org/manual/en/latest/interface/operators.html)） |
| enabled/checked 是**动态求值**，不是缓存 flag | JetBrains `AnAction.update()`（[action-system](https://plugins.jetbrains.com/docs/intellij/action-system.html)）；Unreal `FCanExecuteAction/FIsActionChecked`（[FUICommandList API](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/Framework/Commands/FUICommandList)） |
| 变更广播 = "失效通知 + 重读"，而不是推送完整新状态 | Blender notifier（`WM_event_add_notifier`，[wm_event_system.cc](https://github.com/blender/blender/blob/main/source/blender/windowmanager/intern/wm_event_system.cc)）；Zed `observe/subscribe`（[app/context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)） |
| 布局与面板内容分离 | Rerun `ViewportBlueprint` vs 各 `View` 实现（[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）；VS Code `ViewContainer`/`ViewPane`（[viewPaneContainer.ts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/parts/views/viewPaneContainer.ts)） |

---

## 2. Command / Action 系统

### 2.1 Unreal Engine：Slate `FUICommandList` + `TCommands` + `GEditor`

- **命令声明与绑定分离。** 工具包/模块用 `TCommands<FMyCommands>` 子类在构造时用 `UI_COMMAND(CommandName, "Label", "Tooltip", Icon, Chord)` 声明一批命令；运行时再用 `FUICommandList::MapAction(CommandInfo, ExecuteAction, CanExecuteAction, IsCheckedAction?)` 把命令映射到 handler。参考：[FUICommandList](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/Framework/Commands/FUICommandList)、[TCommands](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/TCommands)。
- **命令的可用状态是 delegate。** `CanExecute` / `IsChecked` 是每次菜单/工具栏绘制时求值的委托，因此"命令灰不灰、勾不勾"永远与当前编辑器上下文一致，不需要手动同步 flag。
- **命令列表可叠加（context stacking）。** `FUICommandList::Append` 让视口/子编辑器把局部命令列表叠加到全局列表上，形成"上下文越深优先级越高"的命令解析。全局的 `GEditor` 持有一份 editor-wide 命令列表；每个 asset editor toolkit 持有一份自己的列表。
- **快捷键在命令声明中给出默认绑定**，并允许编辑器快捷键配置覆盖（Slate 的 `FInputGesture`）。
- 文档总览：[Slate UI Framework overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-overview-for-unreal-engine)。

**可借鉴：** "命令 = 元数据（id/label/tooltip/icon/默认快捷键）+ 行为委托（execute/can-execute/checked）" 的三元组；**可批评：** 命令声明分散在 TCommands 构造函数中，注册顺序和宏模板对新人较陡。

### 2.2 Godot 4.x：`EditorPlugin` + 菜单枚举 + `EditorCommandPalette` + `UndoRedo`

- Godot 的**主菜单是中心式枚举**：`EditorNode::MenuOption` 里有 `EDITOR_COMMAND_PALETTE`、`LAYOUT_SAVE`、`LAYOUT_DELETE`、`LAYOUT_DEFAULT` 等分支（[editor_node.h](https://github.com/godotengine/godot/blob/master/editor/editor_node.h)）。这说明"中心枚举 + 中心 switch"在编辑器早期确实存在；**Godot 让插件不受这个 enum 伤害的办法是给插件单独的扩展点注册表**（见第 8 节）。
- **命令面板是注册式的：** `EditorCommandPalette` 是 ConfirmationDialog 单例，通过 `EditorInterface.get_command_palette()` 取得；插件调用 `add_command(command_name, key_name, callable, shortcut_text)` 注册命令，key_name 用 `/` 分组（例如 `"test/command"`）——**命令是数据三元组，不需要改枚举**（[EditorCommandPalette.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorCommandPalette.xml)，文档页 [class_editorcommandpalette](https://raw.githubusercontent.com/godotengine/godot-docs/master/classes/class_editorcommandpalette.rst)）。
- **`EditorPlugin` 是插件生命周期与编辑器的唯一接缝：** `_enter_tree/_exit_tree` 初始化/清理；`add_dock/remove_dock`（新 API，`add_control_to_dock` 已弃用）；`add_inspector_plugin`；`add_import_plugin`；`add_undo_redo_inspector_hook_callback`（在 inspector 修改属性生成 undo action 时插入钩子，用于保存会被覆盖的其他属性）；`get_editor_interface` 已弃用（全局单例可直接访问）。参考：[EditorPlugin.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorPlugin.xml)、[making_plugins.rst](https://github.com/godotengine/godot-docs/blob/master/tutorials/plugins/editor/making_plugins.rst)。
- **命令执行与 undo 是两件事：** 菜单命令/插件命令执行后，真正的可逆编辑由 `UndoRedo`/`EditorUndoRedoManager` 显式记录（`create_action → add_do_method/add_undo_method → commit_action`），也就是说命令本身不入历史，**只有命令产生的数据修改入历史**（[UndoRedo.xml](https://github.com/godotengine/godot/blob/master/doc/classes/UndoRedo.xml)、[EditorUndoRedoManager.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorUndoRedoManager.xml)）。这个区分非常重要，egui 重写时必须保留。

### 2.3 Unity：`EditorWindow` + `MenuItem` + UI Toolkit（对比样本）

- Unity 的命令入口主要是 `[MenuItem("Tools/...")]` 静态方法 + `ShortcutManager`，用字符串菜单路径做标识；可用性靠同一路径的 validate 函数（`[MenuItem(path, true)]`）返回 bool；`EditorApplication.ExecuteMenuItem(path)` 供 UI/脚本调用。字符串 id 的优缺点与 VS Code 相同：**注册便宜、无编译期校验**。
- 编辑窗口统一为 `EditorWindow : ScriptableObject`：可用 `CreateGUI()`（UIElements/UI Toolkit，retained-mode 的 VisualElement 树）或 `OnGUI()`（IMGUI 立即模式）；窗口可 dock、可序列化自身状态。源码：[EditorWindow.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/EditorWindow.cs)。
- UI Toolkit 的编辑器 UI 通过 **`SerializedObject` 绑定** 与数据连接，而不是每类资产手写一套窗口（见第 4 节）；这也是 Unity 能把 Inspector 做成通用基础设施的原因。

### 2.4 VS Code：`CommandsRegistry` + contribution points + `when` 子句

- 运行时的命令是**字符串 id → handler 函数**的注册表：`CommandsRegistry.registerCommand(id, handler)`、`registerCommandAlias`、`getCommand(s)`、`onDidRegisterCommand`（源码 [commands.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/commands/common/commands.ts)）。
- 扩展通过 `package.json` 的 **contribution points** 声明 UI：`contributes.commands`（title/category/icon/enablement）、`contributes.menus`（放在哪些菜单/位置）、`contributes.keybindings`、`contributes.views`、`contributes.viewsContainers`。默认情况下贡献的命令会出现在 Command Palette 中（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）。
- **启用条件用 `when` 子句 + context keys 表达**（焦点在编辑器 / 资源类型 / 调试状态……），命令实现里不需要写一堆 if。文档：[command guide](https://code.visualstudio.com/api/extension-guides/command)。
- 源码按 `base → platform → editor → workbench` 单向分层，command 是 platform 层的公共服务，workbench 通过依赖注入使用（[Source Code Organization](https://github.com/microsoft/vscode/wiki/Source-Code-Organization)）。

### 2.5 JetBrains Platform：`AnAction` + `DataContext`

- 动作是 `AnAction` 子类；**每个 action 实例存活于整个应用生命周期，因此禁止持有字段状态**（官方原文："Classes based on AnAction must not have class fields of any kind"）；必须 override `actionPerformed()`、应 override `update()` 来计算 enabled/visible（[action-system](https://plugins.jetbrains.com/docs/intellij/action-system.html)）。这和 egui 每帧重画的思路天然一致：**可用性永远现算，不缓存**。
- **上下文即动作参数。** `DataContext` 是 `DataKey<T> → 值` 的只读映射，动作通过 `AnActionEvent.getData(DataKey)` 取出当前选中的文件/节点/编辑器等；树节点等 UI 元素可以作为 `DataProvider` 被平台组装进 DataContext。源码：[DataContext.java](https://github.com/JetBrains/intellij-community/blob/master/platform/core-ui/src/openapi/actionSystem/DataContext.java)。这避免了"把选中对象作为参数层层传递"的巨型签名。
- 动作在 `plugin.xml` 中用 `<actions>`/`<group>` 注册，同一 action 可被菜单、工具栏、弹出菜单复用；快捷键由 Keymap 管理、用户可改。
- 后台进程 API 见第 6 节（`ProgressManager`/`ProgressIndicator`）。
- 插件扩展点文档：[Extension Points](https://plugins.jetbrains.com/docs/intellij/plugin-extension-points.html)、[Extensions](https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html)。

### 2.6 Zed / GPUI：`action!` 宏 + `Keymap` + context/dispatch

这是与本项目最接近的 Rust 参考，值得逐条抄：

- **动作是 Rust 类型。** `actions!(editor, [MoveUp, MoveDown, ...])` 生成无字段的 unit struct；复杂动作用 `#[derive(Action)]` + `#[action(namespace = editor)]` 声明带字段 action，并要求 `Clone + PartialEq`（以及 `serde::Deserialize`、`schemars::JsonSchema`，除非 `#[action(no_json)]`）。**动作因此可以从 JSON keymap / 配置文件反序列化**（[action.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/action.rs)）。
- **Keymap 是数据。** `Keymap { bindings: Vec<KeyBinding>, binding_indices_by_action_id, disabled_binding_indices, version }`，`KeyBinding = keystrokes + context_predicate + action`；`KeymapVersion` 在绑定变化时递增（[keymap.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/keymap.rs)）。`Unbind`/`NoAction` 可以显式禁用某条绑定。
- **分发按 context 栈解析：** 一个窗口维护 `KeyContext` 栈（focus 的编辑器类型、是否在 vim 模式、modal 状态等），绑定按 context predicate 匹配，最深/最具体的上下文优先；action listener 注册在元素树上，未处理则继续冒泡。
- **实体/订阅模型：** `App` 拥有所有 `Entity<T>`；`Entity<T>` 只是句柄，`Context<T>` = App + 当前实体；`cx.observe(&entity, cb)` 监听"被通知"，`cx.subscribe(&entity, cb)` 监听其事件，两者都返回 RAII `Subscription`，drop 即退订（[app/context.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs)、[ownership 文档](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs)）。这正是 "谁拥有状态、谁订阅谁" 的显式化。
- **Undo 拆成两组动作：** `Undo/Redo`（编辑）与 `UndoSelection/RedoSelection`（选择历史）分开（[actions.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/actions.rs)）。这对 egui 编辑器是个强提示：**选择这种"视图态但跨面板共享"的变化，值得有自己的轻量历史，不要塞进文档 undo**。

### 2.7 Blender：Operator + RNA 元数据

- Blender 的动作单位是 **operator**（`bpy.ops.*` / C 侧 `wmOperatorType`）："Operators execute an action the moment they're activated"，可以来自按钮、弹出菜单、搜索菜单；工具（tool）则不同，需要交互输入（[Operators manual](https://docs.blender.org/manual/en/latest/interface/operators.html)）。
- Operator 的参数就是 RNA 属性（带类型、范围、UI 元数据），因此同一 operator 可以被 UI、Python、快捷键、宏、redo 面板复用——**"命令即带 schema 的数据对象"** 在 Blender 里是彻头彻尾的实现方式。
- RNA 的自我描述："RNA is a system for defining structured data with rich metadata and runtime behavior… used to drive the UI, animation system, library overrides, and the Python API"（[RNA dev doc](https://developer.blender.org/docs/features/core/rna/)）。属性面板大量由 RNA 自动生成；复杂/定制 UI 才写 Python `Panel`/`UIList` 脚本。这个"元数据优先、定制兜底"的取舍与第 4 节 inspector 设计直接相关。

### 2.8 Rerun：`SystemCommand` + CommandSender（数据命令 + 帧末 drain）

- Rerun 把所有系统级变更建模为 `SystemCommand` enum（ActivateApp/CloseApp/LoadDataSource/SetSelection/Screenshot/...），通过 `CommandSender` channel 发送，由 viewer 在合适时机接收处理；selection 有便捷构造器 `SystemCommand::set_selection(...)`、`clear_selection()`（[command_sender.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/command_sender.rs)）。
- 关键设计：**命令是数据，UI 只发命令，不内联修改 app state**；命令在帧边界统一处理，避免"UI 正在遍历数据时被修改"的借用/顺序问题（`ViewportBlueprint` 的 `deferred_commands` 在帧末应用，见第 7 节）。
- 相比之下 Zed 是 retained-mode 的实体/订阅模型，Rerun 是 immediate-mode（egui）+ 数据命令——**对 egui 项目来说 Rerun 是同构度最高的样本**。

### 2.9 横向对比

| 维度 | Unreal Slate | Godot | Unity | VS Code | JetBrains | Zed/GPUI | Blender | Rerun |
|---|---|---|---|---|---|---|---|---|
| 命令载体 | `FUICommandInfo` + delegate | 菜单项/Callable + palette 注册 | MenuItem 路径 + validate | string id → handler | `AnAction` 对象 | Rust 类型 + serde | operator + RNA 参数 | `SystemCommand` enum |
| 注册方式 | TCommands 声明 + MapAction | 代码里 add_item / palette add_command | 特性属性扫描 | package.json 贡献点 | plugin.xml | actions! 宏/derive + keymap | RNA 注册 | enum + sender |
| enabled 求值 | CanExecute delegate | 手动 set_item_disabled | validate 函数 | `when` 子句 | `update()` | context predicate + listener | poll() | 由发送方/状态决定 |
| 快捷键 | 命令内默认 chord | Shortcut 资源 | ShortcutManager | contributes.keybindings | Keymap | Keymap（数据） | Keymap | 内置键位 |
| 命令面板 | 有（各编辑器 UI 不一） | `EditorCommandPalette` 注册式 | Quick Search（全局） | Command Palette（自动） | Search Everywhere | Command Palette（action 数据驱动） | 搜索菜单（operator） | 部分命令 |
| 可序列化 | 部分（配置） | Callable（否） | 否 | 是（id+args） | 部分 | **是（serde+schemars）** | operator + props | 是（enum） |
| 主要教训 | 委托式 can-execute/check | 中心枚举 vs 插件注册表并存 | 字符串路径无类型安全 | 贡献点 + when 子句 | 动作无状态 + DataContext | 类型化 action 是 Rust 最佳形态 | 命令=schema 化数据 | 命令队列 + 帧末处理 |

---

## 3. Document / View 分离与 Selection 服务

### 3.1 Godot：`EditorSelection` 单例 + `EditorInterface`

- `EditorSelection` 管理 SceneTree 的选择：`add_node/remove_node/clear/get_selected_nodes/get_top_selected_nodes`，并发出 `selection_changed` 信号（[EditorSelection.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorSelection.xml)）。
- **选择与"在 Inspector 中编辑"是分离的概念**：官方文档明确写 "The newly selected node will not be automatically edited in the inspector. If you want to edit a node, use `EditorInterface.edit_node`"；`EditorInterface` 还提供 `inspect_object`、`get_inspector`、`get_selection`、`get_editor_undo_redo`（[EditorInterface.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorInterface.xml)）。这允许"锁定某个对象的 inspector"或"点选 A、在 inspector 看 B"这类高级用法。

### 3.2 Unity：静态 `Selection` + Inspector window lock

- `Selection` 是静态类（`Selection.objects`、`Selection.activeObject`、`selectionChanged` 事件），任何窗口都能读；Inspector 只是它的一个消费者（[Selection.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/Selection/Selection.cs)、[Selection script reference](https://docs.unity.com/en-us/engine/6000.3/script-reference/unityeditor/selection)）。
- **多选与 window lock：** Inspector 支持多对象同时编辑（显示共同属性），每个 Inspector 窗口还有 lock 按钮，锁定后不再跟随 Selection 变化（[Inspector options](https://docs.unity.com/zh-cn/engine/6000.0/manual/unity-editor/using-the-editor/using-the-inspector/inspector-options)）。**lock 是面板的 view state，不是全局 selection 的一部分**——这一点在 egui 重写里要明确：`InspectorLock { pinned: Option<EntityId> }` 属于面板实例状态。

### 3.3 Unreal：`USelection`（编辑器对象）

- Unreal 把选择放在编辑器对象 `USelection` 里，`GEditor` 提供 `GetSelectedActors()/GetSelectedObjects()` 等访问，属性/细节面板监听 selection 变化（[USelection API](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/USelection)）。选择是编辑器级数据，不是某个 Details 面板的字段。

### 3.4 Zed：App 拥有实体，订阅传播

- `Workspace` 持有 pane/dock/item 分组与 active item；`Project` 持有 worktree、buffer、LSP；`Editor` 持有 `MultiBuffer` 与 selection；UI 通过 `cx.observe/subscribe` 精确订阅某个实体的事件（[workspace.rs](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/workspace.rs)、[editor.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/editor.rs)）。
- **没有全局"selection 服务"类**，而是每个 Editor 实体自己持有 selection，再由 `Workspace` 聚合"当前活跃 item/editor"给需要跨面板的场景。这与"全局 selection 单例"是两个可选设计，取决于编辑器是否需要跨文档选择（引擎编辑器通常需要，见 Rerun/Godot）。

### 3.5 Rerun：双缓冲 `ApplicationSelectionState` + selection history

- selection 与 hover 都在应用级状态里，**且是双缓冲的**：帧内读到的是上一帧的选择，本帧的写入下一帧才可见；文档注释写明 "Both hover and selection are double buffered: Changes from one frame are only visible in the next frame"，写入入口只有 `SystemCommand::set_selection`（[selection_state.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs)）。
- 这带来两个好处：**a)** UI 绘制期间 selection 不会中途改变；**b)** 所有"选中导致的联动"（selection panel、blueprint tree、viewport 高亮）在下一帧统一看到一致状态。这是 immediate-mode 下非常值得抄的技巧。
- Rerun 还给 selection 单独做了 undo/redo 历史（"Selection history… undo/redo previous selections"），见 [Selection docs](https://rerun.io/docs/reference/viewer/selection)。

### 3.6 共同模式与 Rust 映射

| 模式 | 做法 | Rust/egui 落点 |
|---|---|---|
| selection 属于应用而非面板 | 单例/store 字段 | `EditorStore.selection: SelectionState`（可 per-context：scene/asset/timeline） |
| 视图只读、通过版本号失效 | selection 有 epoch/revision | `SelectionRevision(u64)`；面板缓存上一次 revision 决定是否重建 projection |
| 面板级 lock/pin 属于面板状态 | Inspector lock 是 view state | `PanelState`（egui Memory 或 store 的 view-state 子 store） |
| 选择历史独立于文档 undo | Zed/Rerun | 单独的 `SelectionHistory`（可选） |
| 跨面板多路广播 | signal/event/observer | 单一 `Revision` 计数比 N 个回调更简单（egui 每帧重读即可） |

---

## 4. Inspector / 属性系统

### 4.1 Unity：`SerializedObject` / `SerializedProperty` / `PropertyDrawer`

- 官方源码注释一句话总结："SerializedObject and SerializedProperty are classes for editing properties on objects in a completely generic way that automatically handles undo and styling UI for prefabs"（[SerializedObject.bindings.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/SerializedObject.bindings.cs)）。**generic + 自动 undo + 统一 prefab 语义** 是它存在的全部理由。
- 每个自定义 Inspector 用 `serializedObject.FindProperty("field")` 拿属性路径，编辑 `SerializedProperty`，最后 `ApplyModifiedProperties()`——所有编辑自动进 undo、自动标 dirty、prefab override 也自动处理。
- 类型级定制走 `PropertyDrawer`：`[CustomPropertyDrawer(typeof(MyType))]` 让**该类型在任何 inspector 中**都使用同一绘制逻辑（IMGUI 的 `OnGUI` 或 UIElements 的 `CreatePropertyGUI`），而不是在资产 inspector 里写分支（[PropertyDrawer script reference](https://docs.unity.com/en-us/engine/6000.5/script-reference/unityeditor/propertydrawer)）。
- UIElements 的绑定 API 把字段/属性直接绑定到控件（[Bindable data types and fields](https://docs.unity.com/en-us/engine/6000.6/manual/uitoolkits/uielements/uie-data-binding/uie-editor-binding/uie-binding-data-type-conversion)）。

### 4.2 Unreal：Details Panel + `FPropertyEditorModule` customization

- Details 面板默认由 `UPROPERTY` 的反射元数据驱动（EditAnywhere/VisibleAnywhere、Category、ClampMin/ClampMax、EditCondition、ToolTip 等）；改变时通过 `PostEditChangeProperty` 通知对象（并自动记录事务）。
- 定制通过注册表：`FPropertyEditorModule` 提供 `RegisterCustomClassLayout`（整个类的 Details 定制）和 `RegisterCustomPropertyTypeLayout`（某个 struct 类型的属性编辑行）；实现 `IDetailCustomization` / `IPropertyTypeCustomization`。官方文档：[Details Panel Customization](https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customization-in-unreal-engine)。
- **注意粒度的区别：** 类级定制只决定"这个类型怎么展示"，类型级定制决定"这个字段类型怎么画"。绝大多数字段复用默认渲染器。

### 4.3 Godot：`_get_property_list` + `EditorInspectorPlugin` + `EditorProperty`

- 对象可以用 `_get_property_list()` 动态返回属性（可用列表、分组、sub-group、保存策略），并在需要时 `notify_property_list_changed()`；`@export` 是脚本层的声明糖（[Object.xml](https://github.com/godotengine/godot/blob/master/doc/classes/Object.xml)）。**属性是运行时元数据，不是 C++ 编译期字段表**。
- 插件用 `EditorInspectorPlugin._can_handle(object)` 决定是否接管，然后 `_parse_property` / `_parse_begin` / `_parse_group` 插入 `EditorProperty` 自定义控件；官方教程：[inspector_plugins.rst](https://github.com/godotengine/godot-docs/blob/master/tutorials/plugins/editor/inspector_plugins.rst)。同样是"注册 + 兜底"结构。

### 4.4 Blender：RNA 元数据自动生成 UI

- RNA 定义 struct/properties/functions 的元数据（UI 名称、tooltip、subtype、range、枚举项……），Properties 编辑器据此**自动生成**属性 UI；同一套定义同时服务 Python API、动画系统、库覆盖（[RNA dev doc](https://developer.blender.org/docs/features/core/rna/)）。
- 定制 UI 走 Python `Panel`/`UILayout` 脚本（add-on 教程：[Add-on Tutorial](https://docs.blender.org/manual/en/latest/advanced/scripting/addon_tutorial.html)）。**默认自动 + 少量手写**的结构让 Blender 能维持大量属性而 UI 代码量可控。

### 4.5 Bevy：Reflect + `bevy-inspector-egui`

- Bevy 的反射（`#[derive(Reflect)]`、`TypeRegistry`、动态 `PartialReflect`、path navigation、patching）是通用的元数据层（[bevy_reflect docs](https://docs.rs/bevy_reflect/latest/bevy_reflect/)）。
- `bevy-inspector-egui` 提供了显示任意 `Reflect` 值的通用机制（`reflect_inspector`），可给字段/枚举挂 `inspector_options`，并提供资源/实体/资产 inspector 与 drop-in 插件（[bevy-inspector-egui](https://github.com/jakobhellermann/bevy-inspector-egui)）。**这是一个"Rust + egui 上不写 per-asset inspector 也能活"的存在证明。**
- 官方 Bevy Editor 的早期原型仓库已归档，明确方向是继续在主仓库推进 `.bsn` 资产格式、UI 与工具；早期原型讨论过的 undo/redo 设计 issue 仍可参考（[bevy_editor_prototypes](https://github.com/bevyengine/bevy_editor_prototypes)、[undo-redo issue #28](https://github.com/bevyengine/bevy_editor_prototypes/issues/28)）。

### 4.6 Rerun：`ComponentUiRegistry` + `DataUi` trait

- `ComponentUiRegistry` 用 `ComponentUiIdentifier` 作 key，注册"单行编辑/多行编辑/只读显示"回调，并提供 `fallback_ui`；返回值 `TryShowEditUiResult { Shown { edited_value }, NotShown }`，让调用方知道 UI 是否真的被展示（[component_ui_registry.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/component_ui_registry.rs)）。
- `DataUi` trait 是"能把自己画进 `egui::Ui`"的类型：`fn data_ui(&self, ctx: &StoreViewContext, ui: &mut egui::Ui, ui_layout: UiLayout)`；另有 `AppUi` 用于"活在记录之外"的数据（[re_data_ui/src/lib.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_data_ui/src/lib.rs)）。
- **注册表 + fallback + 可选覆盖** 是 Rerun 能支撑"数百种 component 类型 + 自定义可视化"的根基。

### 4.7 为什么"单一 asset inspector + per-type 编辑器"会烂（opinionated）

结合以上样本，可以给出六条明确的腐烂机制：

1. **组合爆炸：** N 种资产 × M 种属性编辑形态 × K 种上下文（多选、数组元素、嵌套 struct、prefab override、只读）→ 中心 `match` 的规模是乘法而不是加法。
2. **复用断裂：** 数组/嵌套/多选/引用字段在任何 inspector 里都应该长得一样；per-asset 写法的结果是要么复制粘贴，要么各资产行为不一致。
3. **Undo 旁路：** 手工 UI 直接改结构体字段，很容易绕过 `UndoRedo`/事务与 dirty 标记；Unity 的 `SerializedObject`、Unreal 的 `FPropertyEditorModule`、Godot 的 inspector 钩子都在解决同一类问题。
4. **元数据无处安放：** label、tooltip、范围、只读、条件显示、分组这些信息如果散落在每个 inspector 代码里，就没有单一事实来源。
5. **插件不可扩展：** 第三方资产类型只能改核心代码；Godot/VS Code/Unreal 的做法都是"注册 + 默认兜底"。
6. **测试不可能：** UI 与数据写死在一起，无法 headless 验证"设置属性 → undo → 值恢复"。

**正确形态：** 先有 `PropertyModel`（路径 + 值 + 元数据 + flags），再有 `PropertyEditorRegistry`（按 `TypeId`/`PropertyKind` 找编辑器 + fallback），最后才是少量 `override`（某个类/某个字段需要特殊 UI 时）。所有编辑产出 `SetProperty { path, value }` 动作，由事务层统一记录 undo。

---

## 5. Undo / Redo 与事务

### 5.1 Godot：`UndoRedo` 与 `EditorUndoRedoManager`

Godot 的事务 API 是"**记录 do/undo 对，再提交**"：

```
var undo_redo = UndoRedo.new()
func _on_my_button_pressed():
    var node = get_node("MyNode2D")
    undo_redo.create_action("Move the node")
    undo_redo.add_do_method(do_something)
    undo_redo.add_undo_method(undo_something)
    undo_redo.add_do_property(node, "position", Vector2(100, 100))
    undo_redo.add_undo_property(node, "position", node.position)
    undo_redo.commit_action()
```

- 必须 `create_action` → 若干 `add_do_*`/`add_undo_*` → `commit_action`；可以注册多个方法/属性；`merge_mode` 控制连续编辑是否合并（拖动滑条场景）（[UndoRedo.xml](https://github.com/godotengine/godot/blob/master/doc/classes/UndoRedo.xml)）。
- **编辑器插件必须用 `EditorUndoRedoManager` 而不是裸 `UndoRedo`**：它把历史按场景 tab 分成不同 history（`get_history_undo_redo(id)`），支持 `custom_context`（本地化/嵌套资源场景）、`backward_undo_ops`、`is_committing_action()` 以及 `add_undo_redo_inspector_hook_callback`（[EditorUndoRedoManager.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorUndoRedoManager.xml)、[EditorPlugin.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorPlugin.xml)）。
- 关键点：**可逆编辑按"文档/场景"分栈，UI 命令本身不入历史**。

### 5.2 Unreal：`FScopedTransaction` + `UObject::Modify()`

- 事务用 RAII：`FScopedTransaction Transaction(LOCTEXT("...", "Move Actor"));` 打开事务，作用域结束自动提交；修改对象前调用 `Obj->Modify()` 把原状态存入 transaction buffer；对多个对象的复合编辑放在同一事务里；属性面板的编辑自动套事务并触发 `PostEditChangeProperty`，撤销走 `PostEditUndo`（[FScopedTransaction](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/FScopedTransaction)）。
- 这种"**先 Modify 再改**"的契约确保 undo 能恢复被改对象的旧值，不依赖每个操作手写逆操作。

### 5.3 Blender：Undo Step / Undo Type

- 官方设计文档：undo 是 "undo stack storing a list of undo steps"；**通常只存数据，不存 UI 变更**；step 可以是 relative（只能从相邻 step 载入）或 absolute；不同编辑器可注册自己的 undo type（memfile 与 per-ID 两种实现）（[Undo System dev doc](https://developer.blender.org/docs/features/core/undo/)，源码 [BKE_undo_system.hh](https://github.com/blender/blender/blob/main/source/blender/blenkernel/BKE_undo_system.hh)）。
- 用户可见的 undo/redo/历史菜单由 `bpy.ops.ed.*` 暴露（[Undo & Redo manual](https://docs.blender.org/manual/en/latest/interface/undo_redo.html)）。

### 5.4 Zed：编辑历史与选择历史分离

- Zed 的 Undo/Redo 是 action，且另有 `UndoSelection`/`RedoSelection`（[actions.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/actions.rs)）；编辑历史属于文本 buffer，选择历史是另一套。这里传达的信号：**不是所有"可回退"都进同一个 undo 栈**。

### 5.5 Rerun：selection history 作为"轻量 undo"

- Rerun 不是文档编辑器，但它给 selection 做了独立的 undo/redo 历史（[Selection docs](https://rerun.io/docs/reference/viewer/selection)）。结合它双缓冲的选择状态，可理解为"UI 交互级的历史"与"数据编辑级历史"的分离。

### 5.6 Rust 版本长什么样

推荐的骨架（**数据化操作 + 事务**）：

```
/// 可序列化的文档操作：命令模式的数据化形态
pub trait EditOp: Send + Sync + 'static {
    fn apply(&mut self, world: &mut World);
    fn revert(&mut self, world: &mut World);
    /// 连续拖动时合并（Godot merge_mode / Unreal 连续事务）
    fn merge_key(&self) -> Option<MergeKey> { None }
}

pub struct Transaction {
    pub label: String,
    pub ops: Vec<Box<dyn EditOp>>,
    pub merge: Option<MergeKey>,
}

pub struct UndoStack {
    done: Vec<Transaction>,
    undone: Vec<Transaction>,
    limit: usize,                       // Blender/UE 都有内存上限
}

impl UndoStack {
    pub fn begin(&mut self, label: impl Into<String>) -> TransactionGuard<'_> { /* ... */ }
    pub fn push(&mut self, tx: Transaction) { /* merge or truncate undone */ }
    pub fn undo(&mut self, world: &mut World) { /* revert in reverse order */ }
    pub fn redo(&mut self, world: &mut World) { /* re-apply in order */ }
}
```

设计要点（逐条对应证据）：

- **per-document 栈**：`HashMap<DocumentId, UndoStack>`，对应 Godot 的 per-tab history；UI（selection/layout）不进文档栈。
- **所有写入走记录器**：属性编辑器不直接 `*value = new`，而是 `recorder.set_property(path, new)`；record 层负责生成 `EditOp` 并 apply。这等价于 Unreal 的 `Modify()` 契约 + Unity 的 `ApplyModifiedProperties`。
- **合并策略**：拖动滑条/移动 gizmo 时按帧产生大量 op，必须有 `merge_key`（如 `(DocumentId, PropPath)`）+ 时间窗合并，等价于 Godot `MergeMode`；在 egui 里用 `response.drag_started` 开事务、`drag_stopped` 提交。
- **序列化收益**：如果 `EditOp` 是可序列化 enum（而不是 trait object），还能 record/replay、做 headless 测试、未来协作/脚本宏。代价是穷举所有 op；可以采用"**核心文档编辑用可序列化 enum op，外围一次性 UI 操作用闭包 op**"的混合策略。
- **不变量测试**：`apply(tx) → undo(tx) == 初始状态`、`redo == 最终状态`、合并后栈长度、limit 截断行为——这些可以在没有 UI 的情况下测。

---

## 6. 异步 / 后台工作、进度、取消、资产事件

### 6.1 JetBrains：`ProgressManager` + `ProgressIndicator`

- 后台进程用 `ProgressManager` 执行，`ProgressIndicator` 关联运行中的进程，**负责取消和进度（分数/文本）**；支持模态对话框、非模态（状态栏）、不可见（仅取消）三种形态；`ProgressManager.getProgressIndicator()` 可在任意线程取当前 indicator；不需要进度跟踪的后台任务用 `Application.executeOnPooledThread`；2024.1+ 推荐 Kotlin coroutines（[Background Processes](https://plugins.jetbrains.com/docs/intellij/background-processes.html)）。
- **关键词是"取消"**：每一个后台进程都自带可取消的 indicator，而不是事后补取消按钮。

### 6.2 VS Code：`withProgress` + `CancellationToken`

- 扩展用 `vscode.window.withProgress({ location, title, cancellable }, task)` 把长任务挂到通知/状态栏，任务收到 `CancellationToken`（[VS Code API — Progress](https://code.visualstudio.com/api/references/vscode-api#Progress)）。UI（进度条/取消按钮）由宿主负责，扩展只报告进度和响应取消——**进度 UI 是基础设施，不是每个功能自己画**。

### 6.3 Bevy：`AssetServer` 的异步加载模型

- Bevy 用 async 任务在后台加载资产，`AssetServer::load` 立刻返回 `Handle`；`Assets<T>` 存强类型资源；`Handle` 是 id 引用，可先占位再填充（[Bevy asset docs](https://docs.rs/bevy/latest/bevy/asset/index.html)）。引擎级资产系统提供 `AssetEvent`（新增/修改/移除）作为"资产就绪/变化"的事件源。
- 对编辑器的启示：**导入/加载是引擎资产管线的一部分，UI 只是订阅状态与事件**；不在 UI 线程做解码/IO。

### 6.4 Godot：导入管线在编辑器侧

- Godot 的导入由编辑器在资源导入阶段完成（生成 `.import` 元数据、reimport 面板），文档描述导入流程与配置（[Import process](https://github.com/godotengine/godot-docs/blob/master/tutorials/assets_pipeline/import_process.rst)）。编辑器 UI 提供进度/报告，而不是把导入塞进面板绘制。

### 6.5 Unreal：`FScopedSlowTask`

- 慢任务用 `FScopedSlowTask` 报告进度并支持取消（默认显示进度对话框）（[FScopedSlowTask](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Core/Misc/FScopedSlowTask)）；资产导入有独立的 import task 流程。

### 6.6 Rerun：命令队列在帧末 drain

- 如前所述：所有变更命令在帧边界统一处理，UI 绘制期间数据结构不被修改（[command_sender.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/command_sender.rs)、[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）。这是 immediate-mode 下保持响应性与确定性的核心手段。

### 6.7 推荐的 Rust/egui 形态

```
pub struct TaskManager { tasks: HashMap<TaskId, TaskState> }
pub struct TaskState {
    pub kind: TaskKind,             // Import / Save / Thumbnail / Bake ...
    pub label: String,
    pub progress: Arc<AtomicU32>,   // 0..=10000，避免锁
    pub cancel: CancellationToken,  // 或 Arc<AtomicBool> + channel
    pub status: TaskStatus,         // Queued / Running / Done / Failed / Cancelled
}
// Worker 线程只发消息回主线程；UI 在帧末 drain
pub enum TaskEvent { Progress(TaskId, u32), Finished(TaskId, Result<(), TaskError>), Cancelled(TaskId) }
```

要点：

- **UI 只发 `Action::RunImport { asset, options }`**；TaskManager 在后台跑；UI 从 `&Tasks` 渲染进度条/取消按钮（对应 JetBrains indicator、VS Code withProgress）。
- **取消是任务的一等公民**：切 tab/关资产/退出编辑器都要 cancel（JetBrains 的 indicator 语义）。
- **资产事件用版本号而不是细粒度消息**：`AssetRevision(u64)`（或 `Changed(AssetId)` 的批量列表）足够驱动 inspector/view 重读；Blender notifier 与 Rerun command 都是这个思路的极端形式。
- **异步结果回主线程只能通过命令/事件**，不能在后台线程碰 UI 状态；这与 Rerun 命令队列、Zed 的 `cx.spawn` 语义一致。

---

## 7. Docking / Layout 持久化

### 7.1 Godot：`EditorDock` + `EditorDockManager` + 布局菜单

- `EditorDock` 是可停靠容器：`title`、`layout_key`、`icon`、`default_slot` 等属性；槽位枚举覆盖左右上下与 main screen（`DOCK_SLOT_LEFT_UL` … `DOCK_SLOT_MAIN_SCREEN`）（[editor_dock.h](https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock.h)、[EditorDock.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorDock.xml)）。
- `EditorDockManager` 管所有 dock：`add_dock/remove_dock/open_dock/focus_dock/make_dock_floating/set_docks_visible`（[editor_dock_manager.h](https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock_manager.h)）。
- 布局的保存/恢复/删除通过编辑器菜单项 `LAYOUT_SAVE/LAYOUT_DELETE/LAYOUT_DEFAULT`（[editor_node.h](https://github.com/godotengine/godot/blob/master/editor/editor_node.h)、`EditorLayoutsDialog`）。**布局是编辑器级配置，dock 内容由插件注册**。

### 7.2 VS Code：workbench grid + 持久化 + contribution 的 view containers

- workbench 由 grid 里的 part 组成（sidebar、panel、auxiliary bar、editor part 等）；`layout.ts` 在 `storageService.onWillSaveState` 时保存 sidebar/auxiliary bar 尺寸与显隐等状态（[layout.ts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/layout.ts)）。
- 视图由 `ViewContainer`/`ViewPane` 承载，容器可放在 sidebar/panel 等位置（[viewPaneContainer.ts](https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/parts/views/viewPaneContainer.ts)）；扩展通过 `contributes.viewsContainers` + `contributes.views` 声明（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）。
- 用户可自定义布局（拖动视图、移动 view container、编辑区自定义布局），官方文档：[Custom Layout](https://code.visualstudio.com/docs/configure/custom-layout)。
- 启示：**"布局容器/位置"数据与"视图内容"分离**；插件只声明"我提供哪些视图/容器"，放哪儿由布局系统与用户决定。

### 7.3 Unity：Window > Layouts

- Unity 的布局（Layout）可保存/加载/删除/恢复默认，是 workspace 级配置，编辑窗口（Inspector/Scene/Game/Project）只是被摆放的窗口（[Customize your workspace layout](https://docs.unity.com/en-us/engine/6000.7/manual/extending-the-editor/customizing-your-workspace)）。

### 7.4 Zed：`DockStructure` 持久化

- `DockPosition`（Left/Right/Bottom）+ `DockStructure`；workspace 保存时序列化 `SerializedWorkspace { center_group, dock_structure, paths, ... }`，`PreviousWorkspaceState { dock_structure, open_file_paths, active_file_path, focused_dock }` 用于新窗口恢复；默认 dock state 存在 kv store 的 `default_dock_state` 键（[persistence.rs](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/persistence.rs)、[dock.rs](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/dock.rs)、[workspace.rs](https://github.com/zed-industries/zed/blob/main/crates/workspace/src/workspace.rs)）。
- 即：**布局树（dock 结构）+ 内容标识（打开的文件路径）分开序列化**。

### 7.5 Rerun：`ViewportBlueprint` + `egui_tiles::Tree`

- `ViewportBlueprint` 字段直接是数据：`views: BTreeMap<ViewId, ViewBlueprint>`、`containers: BTreeMap<ContainerId, ContainerBlueprint>`、`root_container`、`tree: egui_tiles::Tree<ViewId>`、`maximized: Option<ViewId>`；变更进 `deferred_commands`，在帧末应用并写回 blueprint store（[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）。
- Blueprint 文档明确它控制面板可见性、**布局结构（Grid/Horizontal/Vertical/Tabs 容器）**、视图类型与配置、视觉属性；可交互改、可保存为 `.rbl` 文件、可程序化生成（[Blueprints docs](https://rerun.io/docs/concepts/blueprint)）。
- 这正是 egui 项目要的答案：**布局 = 一棵 panel-id 树，内容 = 每个 panel 的实现；树可 serde、可持久化；帧末统一应用变更**。

### 7.6 推荐 split（egui 版）

```
#[derive(Serialize, Deserialize)]
pub struct EditorLayout {
    pub version: u32,
    pub tree: LayoutTree,                    // 纯布局：Split / Tabs / Leaf(PanelId)
    pub panels: HashMap<PanelId, PanelSlot>, // PanelId → kind + 可选内容引用
    pub floating: Vec<FloatingWindow>,
}

pub enum LayoutNode {
    Split { axis: Axis, children: Vec<LayoutNode>, fractions: Vec<f32> },
    Tabs { active: usize, children: Vec<LayoutNode> },
    Leaf { panel: PanelId },
}

pub struct PanelRegistry {
    constructors: HashMap<PanelKind, Box<dyn Fn(&mut EditorStore) -> Box<dyn Panel>>>,
}

pub trait Panel {
    fn id(&self) -> PanelId;
    fn title(&self) -> String;
    fn render(&mut self, ui: &mut egui::Ui, state: &EditorState, actions: &mut ActionQueue);
}
```

- **布局树只存 `PanelId`**，不存面板状态；面板状态（scroll、选中 tab、Inspector lock、预览图）放 `PanelState` 或 `egui::Memory` 的 `Id` 键下。
- **持久化分层：** 布局树 + 面板 kind + 文档引用 → 磁盘（serde，带 `version` 迁移）；滚动位置/折叠状态 → `egui::Memory`（`eframe::App::save` / `persist_egui_memory` 已经处理），见 [egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)、[eframe::App](https://docs.rs/eframe/latest/eframe/trait.App.html)。
- **库选择：** `egui_tiles`（Rerun 生产使用；支持横竖/网格布局、tabs、拖拽 docking，[egui_tiles](https://github.com/rerun-io/egui_crates/blob/main/crates/egui_tiles/README.md)）；`egui_dock` 也是成熟选择（tabs、拖出成窗口、可定制外观，[egui_dock](https://github.com/Adanos020/egui_dock)）。无论选哪个，**以自有 `EditorLayout` 数据为 source of truth**，库只负责渲染与拖拽交互，避免把布局状态锁进第三方控件。

---

## 8. 生态规模：插件 / 模块边界

### 8.1 Godot：`EditorPlugin` 的扩展点面

插件能做的事情全部通过 `EditorPlugin` 的注册 API 完成，而不是改 `EditorNode` 的中心 switch：dock（`add_dock`）、inspector（`add_inspector_plugin`）、导入（`add_import_plugin`）、主屏幕（main screen plugins）、3D gizmo、撤销挂钩（`add_undo_redo_inspector_hook_callback`）、命令面板（`EditorCommandPalette.add_command`）。插件元数据在 `plugin.cfg`（[making_plugins.rst](https://github.com/godotengine/godot-docs/blob/master/tutorials/plugins/editor/making_plugins.rst)）。**Godot 的"中心枚举"只服务于内置菜单，插件世界走注册表。**

### 8.2 VS Code：contribution points + 分层源码

- 所有扩展能力都是 `package.json` 的贡献点：commands、menus、keybindings、views、viewsContainers、configuration、walkthroughs……宿主读取后统一装配；扩展代码跑在 extension host 进程，与 UI 隔离（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）。
- 源码分层 `base → platform → editor → workbench`，依赖注入服务（[Source Code Organization](https://github.com/microsoft/vscode/wiki/Source-Code-Organization)）。**模块边界靠编译期依赖方向保证，而不是口头约定。**

### 8.3 JetBrains：action groups + extension points

- action 通过 `plugin.xml` 的 `<actions>/<group>` 注册，同一 action 可被多个 UI 位置引用；扩展点（extension points）让插件向宿主贡献 UI/服务（[action-system](https://plugins.jetbrains.com/docs/intellij/action-system.html)、[Extension Points](https://plugins.jetbrains.com/docs/intellij/plugin-extension-points.html)）。
- 动作无字段状态 + DataContext 让"功能插件"不需要全局单例、也不需要中心分发开关。

### 8.4 Unreal：Module + Plugin（`.uplugin`）

- Unreal 的插件是独立 module（`.uplugin` 描述、`IModuleInterface` 启动/关闭），可贡献 editor mode、asset type、details customization、tab、command list 等；官方文档：[Plugins](https://dev.epicgames.com/documentation/en-us/unreal-engine/plugins-in-unreal-engine)。
- 每个 asset editor toolkit 拥有自己的 command list/FTabManager layout，因此新增资产编辑器不需要触碰全局编辑器代码（[FTabManager](https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/Framework/Docking/FTabManager)）。

### 8.5 Blender：Add-on + RNA

- Add-on 是 Python 包（`bl_info` + `register/unregister`），注册 operator/menu/panel/keymap；RNA 让新增类型与属性自动进入 UI/动画/序列化体系（[Add-on Tutorial](https://docs.blender.org/manual/en/latest/advanced/scripting/addon_tutorial.html)、[RNA dev doc](https://developer.blender.org/docs/features/core/rna/)）。

### 8.6 Rerun：crate 边界 + 三个注册表

- `re_viewer_context` 提供 context/命令/selection/各类 registry；`re_view_*` 是视图类型；`re_ui` 是设计系统；`re_viewport_blueprint` 是布局；`re_data_ui` 是 `DataUi`；面板在 `re_*_ui`（[Cargo.toml](https://github.com/rerun-io/rerun/blob/main/Cargo.toml)）。
- 关键注册表：`ViewClassRegistry`（视图类型）、`ComponentUiRegistry`（component 的编辑/显示）、`FallbackProviderRegistry`（component fallback）；新增视图/可编辑类型 = 实现 trait + 注册，不改中心文件（[viewer_context.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/viewer_context.rs)、[lib.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/lib.rs)）。

### 8.7 Zed：crate-per-feature

- `crates/workspace`、`crates/project`、`crates/editor`、`crates/gpui`、`crates/ui` 等按能力拆 crate；编辑器扩展（语言、主题、agent）通过独立 crate/扩展进程接入。对 Rust 引擎编辑器：**feature crate + 注册表** 比动态插件 ABI 更现实。

### 8.8 综合边界模型

```
/// 每个 feature crate 在组合根注册一次；core 不知道 feature 的具体类型
pub trait EditorFeature {
    fn id(&self) -> FeatureId;
    fn register(&self, reg: &mut EditorRegistry);
}

pub struct EditorRegistry {
    pub actions:     ActionRegistry,      // ActionId → meta + handler
    pub panels:      PanelRegistry,       // PanelKind → constructor
    pub inspectors:  InspectorRegistry,   // TypeId/PropertyKind → editor
    pub importers:   ImporterRegistry,    // 扩展名/资产类型 → importer
    pub menus:       MenuRegistry,        // 菜单/上下文菜单位置 → ActionId
    pub keybindings: KeybindingRegistry,  // KeyChord+Context → ActionId
}
```

- 组合根（`main.rs`/app crate）只列 feature 清单，**这是唯一允许"中心化"的地方**。
- 其余代码禁止出现"按资产类型/面板类型"的中心 `match`；新增功能只允许新增文件 + 注册行（可用架构测试/lint 守住）。
- 编译期 feature 组合 + `cfg` 已够用；动态插件（`dylib`/WASM）是后期选项，不作为第一版目标。

---

## 9. 标准形态：engine editor UI 架构的 canonical shape

综合以上，给出一个不依赖具体框架的"标准形状"：

```
                 ┌──────────────────────── 用户输入 / 快捷键 / 菜单 / palette ───────────────────────┐
                 ▼                                                                                    │
        ┌─────────────────┐    1. 产出 Action（纯数据：id + payload）                                  │
        │   Views/Panels  │───────────────┐                                                          │
        │ （无业务状态）   │               │                                                          │
        └─────────────────┘               ▼                                                          │
                 ▲                ┌──────────────────┐   2. Dispatcher：入队、排序、合并              │
                 │                │   ActionQueue    │──────────────────────────────┐               │
                 │                └──────────────────┘                              ▼               │
                 │ 5. 下一帧读新状态                              ┌─────────────────────────────────┐ │
                 │                                                │  Store / AppState（单写者）      │ │
                 └────────────────────────────────────────────────┤  documents · selection · layout  │ │
                                                                  │  tasks · undo stacks · registries│ │
                                                                  └───────────────┬─────────────────┘ │
                                                                                  │ 3. 产生 Effect/Task  │
                                                                                  ▼                      │
                                                                  ┌─────────────────────────────────┐ │
                                                                  │ Effects：线程池/资产编译/IO      │ │
                                                                  │ 结果以 Action/TaskEvent 回队     │ │
                                                                  └─────────────────────────────────┘ │
                                                                                  │ 4. 帧末：drain、持久化 │
                                                                                  └────────────────────────┘
```

各组件职责与"必须满足的约束"：

| 组件 | 职责 | 约束 |
|---|---|---|
| `App/Store` | 唯一状态所有者；分层子 store（world/documents/selection/layout/tasks/undo/registries） | 只有 Dispatcher/Effect 回调能写；视图只读 |
| `Action` | 用户意图的数据化表达；id + typed payload | 可序列化（至少核心 action）；不含 UI 句柄；不执行 IO |
| `ActionRegistry` | id → label/icon/shortcut/enabled/check/handler | 由 feature 注册；支持 query（palette/menu/keymap 自动生成） |
| `Dispatcher` | 把 action 应用到 store；开事务；产生 effect | 帧内/帧末二选一（推荐帧末统一 drain）；可记录日志 |
| `Document model` | 资产/场景的编辑模型；dirty/revision；每文档一个 undo 栈 | 编辑一律通过 `EditOp`；异步结果不直接改模型而走 action |
| `Selection service` | 应用级选择；多 context；版本号 | 面板 lock 属于面板状态；选择变更不写文档 |
| `ViewModel` | 从 store 投影只读数据（排序/过滤/格式化） | 可用 revision 缓存；不包含可变业务状态 |
| `View/Panel` | 渲染 + 发 action；持有的仅是 view state（scroll/折叠） | 不直接调 engine/IO；不跨面板持有引用 |
| `Inspector` | `PropertyModel` + `PropertyEditorRegistry` + fallback | 属性写入产出 action；per-type override 是插件 |
| `Undo` | per-document transactions + merge + limit | UI/selection 不进文档栈（除非独立历史） |
| `Layout` | 纯数据树 + PanelRegistry + 版本化持久化 | 布局树不存面板状态；库只负责渲染/拖拽 |
| `Tasks/Effects` | 后台执行、进度、取消、事件回传 | UI 线程零阻塞；结果只经队列回主线程 |
| `Plugins/Features` | 注册 actions/panels/inspectors/importers/menus/keys | 中心文件只做组合根；无中心 match |

---

## 10. 对 egui-based Rust engine editor 的 10 条具体建议

> 背景（来自任务描述，非本次源码审计）：kairos_editor 现状 = god `Message` enum + 中心 `match` + `Drawer` struct 持有状态 + 每种资产一个 inspector。以下每条给出"替换什么 / 怎么落 / 验收"。

### 建议 1：用类型化 `Action` + `ActionRegistry` 取代 god `Message` enum 与中心 match

- 现状问题：所有 UI 事件塞进一个 enum，每个发送点改 enum，每个接收点改 match；无法序列化、无法生成 palette、无法单独测试。
- 目标：`Action` 是数据（id + payload），注册表持有 label/icon/shortcut/enabled/check/handler；菜单、快捷键、command palette、右键菜单都从注册表生成。
- egui 落地：每个面板用 `actions.push(Action::Select(...))`（或通过 `&mut ActionQueue`），帧末统一 `dispatch`；不写 `match message { ... }` 的集中大块。
- 证据：Zed 类型化 action（[action.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/action.rs)）、VS Code registry（[commands.ts](https://github.com/microsoft/vscode/blob/main/src/vs/platform/commands/common/commands.ts)）、Rerun `SystemCommand`（[command_sender.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/command_sender.rs)）。
- 验收：新增一个面板命令 = 1 个 action variant + 1 条 registry 注册，**不改任何 `match`**；palette 自动出现该命令。

### 建议 2：引入单一 `EditorStore`；`Drawer` 退化为无状态 view

- 现状问题：`Drawer` struct 既是视图又持有业务状态，面板之间靠 `Message` 互相猜状态。
- 目标：`EditorStore` 拥有 documents/selection/layout/tasks/undo；面板只持 view-state（scroll、折叠、lock、展开项），render 签名 `fn render(&mut self, ui: &mut egui::Ui, state: &EditorState, actions: &mut ActionQueue)`。
- egui 落地：`&EditorState` 与 `&mut ActionQueue` 分离借用可避免"同时可变借用两个字段"的经典冲突；view-state 用 `egui::Id` + `Memory::data` 或 `PanelState` map（[egui Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)）。
- 证据：Rerun 把状态控制在 app state、UI 只读（[selection_state.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs)）；Zed 的 App 所有权模型（[ownership](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs)）。
- 验收：任何 `Drawer` 里搜不到 `&mut` 业务字段（`Document`/`Scene`/`Asset`）；删除 `Message::*StateChanged` 类消息。

### 建议 3：Selection 服务化（应用级、多选、版本号、面板 lock、独立历史）

- 目标：`SelectionState` 属于 store；面板通过 revision 感知变化；Inspector 默认跟随 selection，但 lock 后绑定到 pin 的目标。
- egui 落地：`SelectionRevision(u64)`，面板缓存上次 revision 只在变化时重建 projection（避免每帧重算）；双缓冲可选（Rerun 模式）——单帧内写入、下一帧读，能消除"点击 A 后同一帧 B 面板读到的还是旧 A"类 bug。
- 证据：Godot `EditorSelection` + `selection_changed`（[EditorSelection.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorSelection.xml)）、Unity lock（[Inspector options](https://docs.unity.com/zh-cn/engine/6000.0/manual/unity-editor/using-the-editor/using-the-inspector/inspector-options)）、Rerun 双缓冲（[selection_state.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs)）。
- 验收：scene tree / viewport / inspector / asset browser 四处选择行为一致；多选共同属性可编辑；lock 不污染全局 selection。

### 建议 4：Inspector 改为"属性模型 + 编辑器注册表"，删除 per-asset inspector

- 目标：`PropertyModel { path, label, value, metadata, flags }`；`PropertyEditorRegistry` 按 `TypeId`/`PropertyKind` 解析编辑器，找不到走 fallback；资产特有 UI 仅作为 `override`（`register_asset_override(AssetTypeId, ...)`），不是默认路径。
- 元数据来源：若引擎已有反射（bevy_reflect 或自研），直接用；没有就先给核心类型写 `impl Inspector`（`bevy-inspector-egui` 的 `InspectorEguiImpl` 模式，[bevy-inspector-egui](https://github.com/jakobhellermann/bevy-inspector-egui)）。
- 属性写入必须走 `Action::SetProperty { path, value }` → 事务 → undo；禁止在 UI 里直接 `*field = v`。
- 证据：Unity `SerializedObject` 的 generic+undo（[SerializedObject.bindings.cs](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/SerializedObject.bindings.cs)）、Unreal 的 class/type-level customization（[Details Panel Customization](https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customization-in-unreal-engine)）、Rerun registry+fallback（[component_ui_registry.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/component_ui_registry.rs)）。
- 验收：新增一种资产类型时，inspector 代码量为 0（有默认渲染）；数组/嵌套/多选复用同一套编辑器；所有属性编辑可 undo。

### 建议 5：Undo 事务化 + per-document 栈 + merge

- 目标：`begin/commit/cancel` 事务；`EditOp { apply/revert }`；`HashMap<DocumentId, UndoStack>`；拖动类编辑按 `merge_key` 合并。
- egui 集成：`response.drag_started()` 开事务、拖动中记录但合并、`drag_stopped()` 提交；滑条/颜色选择用 Godot 式 merge_mode 思路（[UndoRedo.xml](https://github.com/godotengine/godot/blob/master/doc/classes/UndoRedo.xml)）。
- 分工：文档 undo 只记数据编辑；selection/layout 用独立历史（Zed 的 `UndoSelection`，[actions.rs](https://github.com/zed-industries/zed/blob/main/crates/editor/src/actions.rs)）或干脆不记（Blender "通常不存 UI 变更"，[Undo System](https://developer.blender.org/docs/features/core/undo/)）。
- 验收：拖动 gizmo 一秒 = 1 个 undo 步；undo/redo 后 dirty/revision 正确；`apply→undo==初始` 的 headless 测试全绿。

### 建议 6：UI 只发 intent，Effect/Task 在 store 外执行

- 目标：`Action::RunImport/OpenDocument/Save` 只入队；`TaskManager` 在工作线程执行；进度/取消/完成以事件回主线程；UI 渲染任务列表/状态栏（对应 JetBrains ProgressIndicator、VS Code withProgress）。
- 资产事件用 `AssetRevision`/批处理事件驱动失效，而不是让面板轮询文件系统。
- 证据：[Background Processes](https://plugins.jetbrains.com/docs/intellij/background-processes.html)、[VS Code Progress](https://code.visualstudio.com/api/references/vscode-api#Progress)、[Bevy asset docs](https://docs.rs/bevy/latest/bevy/asset/index.html)。
- 验收：导入大资产时 UI 仍 60fps 且可取消；关面板/关资产触发 cancel；无 UI 线程阻塞 IO。

### 建议 7：布局数据树 + PanelRegistry + 版本化持久化

- 目标：自有 `EditorLayout { version, tree, panels, floating }`；布局树只存 `PanelId`；`PanelRegistry` 从 `PanelKind` 构造面板；渲染交给 `egui_tiles` 或 `egui_dock`（[egui_tiles](https://github.com/rerun-io/egui_crates/blob/main/crates/egui_tiles/README.md)、[egui_dock](https://github.com/Adanos020/egui_dock)）。
- 持久化：布局树 + 面板 kind + 文档引用写磁盘（serde + version 迁移）；滚动/折叠写 `egui::Memory`（eframe 自动保存，[eframe::App](https://docs.rs/eframe/latest/eframe/trait.App.html)）。
- 证据：Rerun `ViewportBlueprint` 的 `tree: egui_tiles::Tree<ViewId>` + 帧末 deferred commands（[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）；Godot `EditorDock`/`EditorDockManager`（[editor_dock.h](https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock.h)）。
- 验收：保存/加载布局跨版本稳定；面板内容与布局互不污染；同一面板可开多实例（若需要）由 `PanelId` 区分。

### 建议 8：用 `EditorFeature` 注册边界消灭"每加功能改核心"

- 目标：core crate 只提供 store/action/bus/registry/context；每个 feature crate `impl EditorFeature` 注册 actions、panels、inspectors、importers、menus、keybindings；组合根只列清单。
- egui 落地：feature 注册的是 `PanelKind` 构造函数与 `ActionId` handler，不是 `match` 分支；菜单/palette 从 registry 生成。
- 证据：VS Code contribution points（[contribution-points](https://code.visualstudio.com/api/references/contribution-points)）、Godot EditorPlugin 扩展点（[EditorPlugin.xml](https://github.com/godotengine/godot/blob/master/doc/classes/EditorPlugin.xml)）、Rerun registries（[lib.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/lib.rs)）。
- 验收：新增一个面板/一种资产编辑器不修改 core crate（CI 用 `cargo metadata`/依赖图断言）；`ui` crate 禁止直接依赖 engine 内部（只依赖 adapter）。

### 建议 9：遵守 immediate-mode 纪律（这是 egui 项目成败关键）

- **规则 1：UI 构建期只读 + 入队**，帧末 drain；任何"边遍历边改"都会触发借用冲突和顺序 bug（Rerun 的 deferred commands 就是为此存在，[viewport_blueprint.rs](https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs)）。
- **规则 2：view-state 不进入业务 store**；用稳定的 `egui::Id`（egui 自动 id 依赖路径，面板顺序变化会漂移；显式 `ui.push_id(...)`）。
- **规则 3：每帧不重建昂贵投影**；用 revision 缓存（selection revision、asset revision、document revision）。
- **规则 4：不要把 `Ui`/`Response` 存进 store**；事件在当帧转成 action，之后只依赖数据。
- **规则 5：对话框/模态也先入队**，由 store 决定打开哪个 modal，而不是在 widget 里 `if open { Window::new... }` 混着状态改。
- 证据：egui 的 immediate mode 说明与 `Memory.data`（[egui docs](https://docs.rs/egui/latest/egui/)、[Memory](https://docs.rs/egui/latest/egui/struct.Memory.html)）。

### 建议 10：可测试性、可观测性、record/replay

- **action log**：Dispatcher 记录 `(frame, action, origin)`，加一个 debug 面板可看/可导出；出 bug 时可直接复现输入序列。
- **headless store 测试**：不进 egui，直接 `store.apply(action)` 断言状态；undo 不变量、selection 传播、layout 序列化 round-trip 都能测。
- **命令日志 + 可序列化 action** = 未来的 record/replay、脚本化、协作编辑的基础（Rerun 的 enum 命令与 Zed 的 serde action 都朝这个方向）。
- **架构测试**：`ui` crate 不得依赖 engine 内部；不得出现按资产类型的中心 `match`；面板不得持有业务 `&mut`。把规则写进 CONTEXT.md/AGENTS.md，用 CI 检查。

### 附加建议：迁移路径（strangler，避免大爆炸）

1. **Phase 0**：冻结旧 `Message` 的新增；新增 `Action`/`EditorStore`/`ActionQueue` 骨架；写 `Messager::send → Action` 适配层，新老并存。
2. **Phase 1**：把 selection、documents、tasks 搬进 store；`Drawer` 改为只读 + 发 action；删掉对应的 `Message` 状态类变体。
3. **Phase 2**：按面板逐个迁移（scene tree → inspector → asset browser → viewport）；每迁一个删一批 `Message` 变体和 match 分支。
4. **Phase 3**：inspector registry + per-type editor 替换 per-asset inspector；属性写入统一走事务。
5. **Phase 4**：layout 数据树 + PanelRegistry + 持久化迁移（旧布局文件升级或丢弃，需产品决策）。
6. **Phase 5**：删除 `Messager` 与 god enum；补架构测试；更新 CONTEXT/AGENTS 文档。

风险提示：迁移期新老两条路径并存时，务必让 **同一状态的唯一写者** 只有一个（旧 Messager 只做转发），否则会出现"改了一半状态"的经典半迁移 bug。

---

## 11. 参考来源

**Unreal Engine**
- Slate overview: https://dev.epicgames.com/documentation/en-us/unreal-engine/slate-overview-for-unreal-engine
- FUICommandList: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/Framework/Commands/FUICommandList
- TCommands: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/TCommands
- FScopedTransaction: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/FScopedTransaction
- USelection: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Editor/UnrealEd/USelection
- Details Panel Customization: https://dev.epicgames.com/documentation/en-us/unreal-engine/details-panel-customization-in-unreal-engine
- FTabManager: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Slate/Framework/Docking/FTabManager
- FScopedSlowTask: https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Core/Misc/FScopedSlowTask
- Plugins: https://dev.epicgames.com/documentation/en-us/unreal-engine/plugins-in-unreal-engine

**Godot**
- EditorPlugin: https://github.com/godotengine/godot/blob/master/doc/classes/EditorPlugin.xml
- EditorDock: https://github.com/godotengine/godot/blob/master/doc/classes/EditorDock.xml ；实现 https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock.h
- EditorDockManager: https://github.com/godotengine/godot/blob/master/editor/docks/editor_dock_manager.h
- EditorNode（MenuOption / LAYOUT_*）: https://github.com/godotengine/godot/blob/master/editor/editor_node.h
- EditorCommandPalette: https://github.com/godotengine/godot/blob/master/doc/classes/EditorCommandPalette.xml
- EditorSelection: https://github.com/godotengine/godot/blob/master/doc/classes/EditorSelection.xml
- EditorInterface: https://github.com/godotengine/godot/blob/master/doc/classes/EditorInterface.xml
- UndoRedo: https://github.com/godotengine/godot/blob/master/doc/classes/UndoRedo.xml
- EditorUndoRedoManager: https://github.com/godotengine/godot/blob/master/doc/classes/EditorUndoRedoManager.xml
- Object/_get_property_list: https://github.com/godotengine/godot/blob/master/doc/classes/Object.xml
- Inspector plugins: https://github.com/godotengine/godot-docs/blob/master/tutorials/plugins/editor/inspector_plugins.rst
- Making plugins: https://github.com/godotengine/godot-docs/blob/master/tutorials/plugins/editor/making_plugins.rst
- Import process: https://github.com/godotengine/godot-docs/blob/master/tutorials/assets_pipeline/import_process.rst

**Unity**
- SerializedObject source: https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/SerializedObject.bindings.cs
- EditorWindow source: https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/EditorWindow.cs
- Selection source: https://github.com/Unity-Technologies/UnityCsReference/blob/master/Editor/Mono/Selection/Selection.cs ；文档 https://docs.unity.com/en-us/engine/6000.3/script-reference/unityeditor/selection
- PropertyDrawer: https://docs.unity.com/en-us/engine/6000.5/script-reference/unityeditor/propertydrawer
- UI Toolkit binding: https://docs.unity.com/en-us/engine/6000.6/manual/uitoolkits/uielements/uie-data-binding/uie-editor-binding/uie-binding-data-type-conversion
- Inspector options（lock）: https://docs.unity.com/zh-cn/engine/6000.0/manual/unity-editor/using-the-editor/using-the-inspector/inspector-options
- Customize workspace layout: https://docs.unity.com/en-us/engine/6000.7/manual/extending-the-editor/customizing-your-workspace

**Blender**
- RNA dev doc: https://developer.blender.org/docs/features/core/rna/ （raw: https://projects.blender.org/blender/blender-developer-docs/raw/branch/main/docs/features/core/rna.md）
- Undo System dev doc: https://developer.blender.org/docs/features/core/undo/ （raw: .../docs/features/core/undo.md）
- Depsgraph dev doc: https://developer.blender.org/docs/features/core/depsgraph/ （raw: .../docs/features/core/depsgraph.md）
- BKE_undo_system.hh: https://github.com/blender/blender/blob/main/source/blender/blenkernel/BKE_undo_system.hh
- Notifier 实现: https://github.com/blender/blender/blob/main/source/blender/windowmanager/intern/wm_event_system.cc
- Operators manual: https://docs.blender.org/manual/en/latest/interface/operators.html
- Undo & Redo manual: https://docs.blender.org/manual/en/latest/interface/undo_redo.html
- bpy.msgbus: https://docs.blender.org/api/current/bpy.msgbus.html
- Add-on tutorial: https://docs.blender.org/manual/en/latest/advanced/scripting/addon_tutorial.html

**VS Code**
- CommandsRegistry source: https://github.com/microsoft/vscode/blob/main/src/vs/platform/commands/common/commands.ts
- Contribution points: https://code.visualstudio.com/api/references/contribution-points
- Command guide: https://code.visualstudio.com/api/extension-guides/command
- Custom layout: https://code.visualstudio.com/docs/configure/custom-layout
- Source organization: https://github.com/microsoft/vscode/wiki/Source-Code-Organization
- Layout service: https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/layout.ts
- View panes: https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/parts/views/viewPaneContainer.ts
- Progress API: https://code.visualstudio.com/api/references/vscode-api#Progress

**JetBrains**
- Action system: https://plugins.jetbrains.com/docs/intellij/action-system.html
- DataContext source: https://github.com/JetBrains/intellij-community/blob/master/platform/core-ui/src/openapi/actionSystem/DataContext.java
- Background processes: https://plugins.jetbrains.com/docs/intellij/background-processes.html
- Extension points: https://plugins.jetbrains.com/docs/intellij/plugin-extension-points.html
- Extensions: https://plugins.jetbrains.com/docs/intellij/plugin-extensions.html

**Zed / GPUI**
- action.rs: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/action.rs
- keymap.rs: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/keymap.rs
- app/context.rs（observe/subscribe）: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/app/context.rs
- ownership & data flow: https://github.com/zed-industries/zed/blob/main/crates/gpui/src/_ownership_and_data_flow.rs
- workspace.rs: https://github.com/zed-industries/zed/blob/main/crates/workspace/src/workspace.rs
- persistence.rs: https://github.com/zed-industries/zed/blob/main/crates/workspace/src/persistence.rs
- dock.rs: https://github.com/zed-industries/zed/blob/main/crates/workspace/src/dock.rs
- editor actions: https://github.com/zed-industries/zed/blob/main/crates/editor/src/actions.rs
- editor.rs: https://github.com/zed-industries/zed/blob/main/crates/editor/src/editor.rs

**Rerun**
- SystemCommand / CommandSender: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/command_sender.rs
- SelectionState: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/selection_state.rs
- ComponentUiRegistry: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/component_ui_registry.rs
- DataUi: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_data_ui/src/lib.rs
- ViewerContext / registries: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewer_context/src/viewer_context.rs
- ViewportBlueprint: https://github.com/rerun-io/rerun/blob/main/crates/viewer_support/re_viewport_blueprint/src/viewport_blueprint.rs
- Blueprints docs: https://rerun.io/docs/concepts/blueprint
- Selection docs: https://rerun.io/docs/reference/viewer/selection
- egui_tiles: https://github.com/rerun-io/egui_crates/blob/main/crates/egui_tiles/README.md

**Bevy / egui 生态**
- bevy_reflect: https://docs.rs/bevy_reflect/latest/bevy_reflect/
- Bevy assets（异步加载）: https://docs.rs/bevy/latest/bevy/asset/index.html
- bevy-inspector-egui: https://github.com/jakobhellermann/bevy-inspector-egui
- bevy_editor_prototypes: https://github.com/bevyengine/bevy_editor_prototypes
- bevy editor undo/redo issue: https://github.com/bevyengine/bevy_editor_prototypes/issues/28
- egui（immediate mode）: https://docs.rs/egui/latest/egui/
- egui Memory（持久化/data）: https://docs.rs/egui/latest/egui/struct.Memory.html
- eframe App::save: https://docs.rs/eframe/latest/eframe/trait.App.html
- egui_dock: https://github.com/Adanos020/egui_dock

---

## 12. 局限与未验证项

1. **版本时效性：** 引用的是各项目 main/master 分支（调研日 2026-10-30）。Godot 的 `EditorDock`/`EditorCommandPalette` 属于较新 API（4.5-dev/4.6-dev 区间），Godot 4.4 稳定版可能没有完全相同的类；Rerun 的主干是 0.42.0-alpha.1，crate 目录近半年重组过（`crates/viewer/*` → `crates/viewer_support|panels|views/*`），旧引用路径可能失效。
2. **Unreal/Unity 文档站抓取限制：** dev.epicgames.com 与 docs.unity.com 都是 SPA，web_fetch 只能拿到外壳 HTML。Unreal 的结论基于官方 API 页面 URL（经搜索索引确认存在）+ Slate/PropertyEditor 的公开机制描述；Unity 的结论基于 UnityCsReference 官方 C# 源码（可直接读取）+ 官方文档 URL。**没有**逐条执行 Unreal/Unity 源码级验证，机制描述属于架构层面而非代码行级。
3. **Blender 设计文档自述 WIP：** Undo 文档顶部标注 "WARNING: DISCLAIMER — All this is WIP"。作为架构方向引用是可靠的，但细节可能变化。
4. **Zed/Rerun 是快速演进项目：** 具体类型名与字段会变，但 "action 数据化 / 实体所有权 / 命令队列 / 帧末提交" 这些模式是稳定的。
5. **本调研未阅读 KairosEngine 代码**（按任务要求），第 10 节对 kairos_editor 症状的描述来自任务书，需要 Lead 与 ui-auditor 的 file:line 证据交叉校验后再写进设计文档。



