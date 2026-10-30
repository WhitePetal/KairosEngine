# kairos_editor UI 合并原型（throwaway）

> **状态**：一次性设计验证原型，**不是生产代码**。独立 `[workspace]`，不进主 workspace 构建；
> 零网络、只有 egui 0.35 一个外部依赖。
> 目的：把原先两个分开的原型（`prototypes/editor-ui-core` 的 core reducer 与
> `prototypes/editor-ui-widgets` 的 widget kit/components）合并成**一个可编译、可测试的完整链路**：
>
> ```text
> App(reducer 状态) -> feature components -> pure widgets -> 合成点击/拖拽
>   -> UiOut(Action) -> app.dispatch -> reducer 状态迁移 + Effect -> App::complete
> ```
>
> 相关：`docs/plan/editor-ui-architecture/README.md`（MVU/Store/Action/Effect 设计）、
> `rewrite-plan.md`（Phase 1 App shell）、`docs/adr/0008-editor-ui-architecture.md`。
> 日期：2026-10-30。原 `editor-ui-core` / `editor-ui-widgets` 目录在合并验证通过后已删除。

---

## 0. TL;DR

- **一个 workspace、三个 crate、一份 Cargo.lock、一份 README、一次 `cargo test` 全绿。**
- **49 个测试 / 0 failed / 0 warnings / ~0.15s**（core 20 + ui-widgets 14 + editor-components 15）。
- 其中 **7 个 full-stack 测试**跑的是真实链路：`App::new()` → dispatch 真实 `Action` →
  用 ui-widgets 的 headless harness 渲染 `ProjectPanel`/`InspectorPanel` → 合成
  click/double-click/drag → 收集 `UiOut` → `app.dispatch` 真实 reducer → 断言
  `EditorState§ 与 `Vec<Effect>` → `App::complete` 收尾。widgets/components/core 中
  **没有任何 fake action 或 fake state**。
- 合并过程暴露的 core API 缺口见 §8（项目树不存在、没有项目 mutation、`Open` 直接带 content、
  TreeRow id 与 AssetId 不对齐等）。

---

## 1. 合并证明了什么（这是两个原型分开时无法证明的）

1. **依赖方向可编译期强制**：`editor-core` 零 egui 依赖；`ui-widgets` 只依赖 egui；
   `editor-components` 是唯一同时依赖两者、因而唯一知道领域类型的 crate。
   `ui-widgets/tests/purity.rs` 还扫描源码禁止领域类型。
2. **视图层能驱动真实 reducer**：组件产出的就是 `editor_core_prototype::Action`，
   直接喂给真 `App::dispatch`，不需要 adapter/mock（旧的 `fake_state.rs` + `actions.rs` 已删除）。
3. **跨 slice 规则在真实链路上生效**：点一个资产行 → `SelectionAction::SelectAsset` →
   reducer 同时改 `selection` + `layout.focus` + `project.loading_assets`，并发出
   `FocusPanel` 和 `LoadAsset{ticket}` effect；`App::complete` 再产生 `SpawnPreview`。
   full-stack 测试逐条断言（`project_row_click_round_trips_into_core_reducer`）。
4. **文档编辑/保存的异步语义没被 UI 层破坏**：拖 inspector 的 slider → `SetMaterial` →
   document dirty；点 Save → `Effect::SaveDocument{ticket, bytes}`；此时 UI 仍显示 dirty/saving；
   `App::complete(DocumentSaved)` 后才清 dirty（`inspector_save_click_round_trips_effect_then_completion`）。
5. **widget 层真的不知道领域**：component 把 `ProjectState.assets§（扁平 path 列表）转成
   `Vec<TreeRow>`、把 `TreeOutput{clicked}` 转成 `Action`；`ui-widgets` 只认
   `u64` row id / 字符串 label。
6. **headless 交互测试可以覆盖"点击 → Action → reducer → Effect"**，无需 GPU/窗口/egui_kittest。

---

## 2. 分层与数据流

```text
+---------------------------- editor-core-prototype ----------------------------+
| EditorState { project, selection, documents, layout, modal, inflight }        |
| pure reducers  (&mut EditorState, Action) -> Vec<Effect>                      |
| App { dispatch / dispatch_command / complete / take_effects }                 |
+------------------------------------------------------------------------------+
                ^ Action (plain data)                    | Effect (plain data)
                |                                        v
+----------------------- editor-components-prototype -------------------+
| Frame<'a> { state: &'a EditorState }   UiOut { actions: Vec<Action> } |
| ProjectPanel::show(ui, &Frame, &mut UiOut) -> hit rects              |
|   ProjectState.assets --derive-from-paths--> Vec<ProjectNode>         |
|       --tree_rows()--> Vec<ui_widgets::TreeRow>                       |
|       --TreeView->TreeOutput--> actions_from_tree() --> Action        |
| InspectorPanel::show(ui, &Frame, &mut UiOut)                         |
|   active Document --match EditData--> FormSection/PropertyRow/Slider/TextInput |
|       --widget Output--> DocumentAction::{SetMaterial,SetText,Save,Undo,...}  |
+----------------------------------------------------------------------+
                ^ props / outputs                        | UiOut
                |                                        v
+---------------------------- ui-widgets-prototype ----------------------------+
| Theme tokens -> egui Visuals | atoms (impl egui::Widget) | compounds        |
| TreeView/Toolbar/PanelChrome/PropertyRow/... show() -> plain-data Output      |
+------------------------------------------------------------------------------+
                ^ egui 0.35 (cached, offline)
```

帧循环（原型版，生产见 ADR §3.3）：

```text
1. App::new(); app.dispatch(Action::Project(AssetsDiscovered{ assets }))
2. let frame = Frame::new(&app.state); let mut out = UiOut::new();
   ProjectPanel::show(ui, &frame, &mut out);
3. actions = out.take(); for action in actions { app.dispatch(action); }
4. host: app.take_effects() -> 执行 IO/GPU；完成后 app.complete(ticket, outcome)
```

---

## 3. Crate 映射

```text
prototypes/editor-ui/
├── Cargo.toml            # [workspace] members = editor-core, ui-widgets, editor-components; resolver 3
├── Cargo.lock            # 提交，保证 --offline 可复现
├── .gitignore            # /target
├── README.md
└── crates/
    ├── editor-core/                  package editor-core-prototype（零外部依赖）
    │   ├── src/{lib,state,action,effect,document,command,modal}.rs
    │   ├── src/reduce.rs + src/reduce/{project,selection,document,layout,modal}.rs
    │   └── tests/core_flow.rs        # 19 个 core 集成测试
    ├── ui-widgets/                   package ui-widgets-prototype（只依赖 egui 0.35）
    │   ├── src/{lib,theme,icon,atoms,compounds,testing}.rs
    │   └── tests/{atoms,compounds,purity}.rs
    └── editor-components/            package editor-components-prototype
        ├── src/frame.rs              # Frame + UiOut（ADR 帧层的替身）
        ├── src/project_panel.rs      # 真实 ProjectState -> 路径推导树 -> 真正 Action
        ├── src/inspector_panel.rs    # 真实 Document -> 属性 widget -> DocumentAction
        └── tests/{component_actions,full_stack}.rs
```

| 文件 | 内容 |
|---|---|
| `editor-core/src/state.rs` | `EditorState` 5 slices + `AssetId`/`DocumentId`/`PanelId` + `ProjectState::demo()`（8 个嵌套路径资产） |
| `editor-core/src/action.rs` | 聚合 `Action` + 每域 Action；**合并新增** `CreateAsset/DeleteAsset/RenameAsset` |
| `editor-core/src/effect.rs` | `Effect`（`SaveDocument/LoadAsset/SpawnPreview/FocusPanel/ShowNativeDialog/Quit/PersistLayout` + **新增 `PersistProject`**）、`EffectTicket`、`EffectOutcome` |
| `editor-core/src/reduce/project.rs` | **合并扩展**：AssetDiscovered/load/preview + create/delete/rename（纯函数，返回 `PersistProject`） |
| `editor-core/src/lib.rs` | `App`：dispatch / dispatch_command / complete / take_effects |
| `ui-widgets/src/testing.rs` | headless harness：`frame/click/secondary_click/double_click/drag/type_text/press_key` |
| `editor-components/src/project_panel.rs` | `project_nodes`（path→folder 推导）/`tree_rows`/`actions_from_tree`/`actions_from_toolbar` |
| `editor-components/src/inspector_panel.rs` | Material/Text/Toml 编辑器 + `actions_from_toolbar` |

---

## 4. 状态归属

| 状态 | 归属 | 例子 |
|---|---|---|
| 领域状态（可序列化、要 undo/MCP） | `editor-core-prototype` | assets/selection/documents/dirty/history/layout/modal/inflight |
| UI 视图状态（重启不需恢复） | component 自己的 ViewState | `ProjectViewState { search, expanded: BTreeSet<String> }` |
| 瞬态控件状态 | egui Memory（stable Ids） | TextEdit 焦点/选区、CollapsingHeader 展开、Window 位置 |
| 主题渲染缓存 | `ctx.data`（从 StyleState 刷新） | `Theme` tokens |
| 用户意图 | `UiOut`（帧级缓冲） | `Vec<Action>`，host 帧末 drain → dispatch |

组件**从不**持有领域状态的副本、从不修改 `EditorState`、从不做 IO —— 只读 `&Frame`、只写 `&mut UiOut`。
唯一"每帧临时副本"是 egui 文本编辑所需的 buffer（component 里 clone → 编辑 → 发 `SetText`）。

---

## 5. Headless full-stack 测试配方

`ui-widgets/src/testing.rs` 的 `Harness` 提供 `egui::Context::run_ui` + 合成 `RawInput`。
合并后新增了两个手势（由 full-stack 测试逼出来的）：

- `double_click(pos)`：两套完整 click 序列；第二次 release 帧报告 `double_clicked`。
- `drag(from, to)`：press → 多次 `PointerMoved` → release。**egui Slider 是 `Sense::drag` 控件，
  纯 click 不会改值**（实测 `value` 不变），这是旧 widgets 原型没覆盖的交互。

测试模式：

```rust
let mut draw = |ui: &mut Ui| {
    let frame = Frame::new(&app.state);          // 只读
    let mut out = UiOut::new();
    let output = panel.show(ui, &frame, &mut out);
    *row_rects.borrow_mut() = output.row_rects;  // 测试需要 hit rect
    actions.borrow_mut().extend(out.take());     // 收集真实 Action
};
harness.frame(Vec::new(), &mut draw);
let rect = row_rects.borrow().iter().find(...).unwrap().1;
harness.drag(rect.center(), pos2(rect.right()-8.0, rect.center().y), &mut draw);
drop(draw);                                      // 结束对 app 的只读借用
for action in actions.borrow().clone() { app.dispatch(action); }   // 真实 reducer
```

注意：
- egui 一帧可能跑 2 个 pass（`max_passes=2`），**每帧输出必须覆盖写**；
  但 slider 这类 drag 控件的 `changed` 出现在中间帧，所以 full-stack 里改为**累积** `Action`
  （重复的 `SetMaterial` 对 reducer 幂等：内容相等会被 `apply_edit` 忽略）。
- 点击需要 3 个 pass（draw → PointerMoved → press+release）；Window/Area 需要"稳定帧"。

---

## 6. 验证命令与输出

```sh
cd prototypes/editor-ui
cargo build --offline    # 通过
cargo test --offline     # 通过
```

实测（2026-10-30，macOS aarch64，rustc/cargo 1.97.1；egui 0.35.0 来自本机 cache，零网络）：

```text
$ cargo build --offline
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s

$ cargo test --offline
     Running unittests src/lib.rs (editor_components_prototype)
running 0 tests
     Running tests/component_actions.rs (editor-components-prototype)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/full_stack.rs (editor-components-prototype)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running unittests src/lib.rs (editor_core_prototype)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/core_flow.rs (editor-core-prototype)
running 19 tests
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/lib.rs (ui_widgets_prototype)
running 0 tests
     Running tests/atoms.rs (ui-widgets-prototype)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/compounds.rs (ui-widgets-prototype)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/purity.rs (ui-widgets-prototype)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests editor_components_prototype / editor_core_prototype / ui_widgets_prototype
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

# 合计：49 passed / 0 failed；0 warnings；exit 0
```

---

## 7. 测试清单（49 个）

| 范围 | 数量 | 关键覆盖 |
|---|---|---|
| `editor-core/tests/core_flow.rs` + lib | 19 + 1 | 原有 14 个（选择/编辑/save+complete/close modal/undo/revert/command/异步/quit/layout/save 失败）+ **新增 6 个**：`ProjectState::demo()`、create/delete/rename（含 folder 子路径重写）、documents demo 助手 |
| `ui-widgets/tests` | 8 + 5 + 1 = 14 | 所有 atom/compound 一帧无 panic；Button/Checkbox/TextInput 合成点击；TreeView 行/expander 点击；Toolbar；ConfirmDialog；主题/图标可替换；purity 扫描 |
| `editor-components/tests/component_actions.rs` | 8 | 纯映射：path→folder 树推导/折叠/搜索、TreeRow props、click→`SelectAsset`、double-click→`Open`（按 kind 内容）、toolbar→`CreateAsset/DeleteAsset/FocusPanel`、inspector toolbar→`Save/Undo/Redo/Revert` + `SetMaterial` |
| `editor-components/tests/full_stack.rs` | 7 | 真实往返：资产行点击→selection/layout/loading/effects+complete；double-click→Material 文档；工具栏建文件夹→core assets+PersistProject；slider 拖拽→SetMaterial→dirty；Save→effect→complete 清 dirty；TextInput 打字→SetText；Undo/Redo→history |

> 数量说明：上一阶段汇报的 "widgets 25 tests" 是**两个 crate 合计**（ui-widgets 14 + 旧
> kairos-editor-components 11）。合并后 ui-widgets 保持原 14 个测试不动，components 的 11 个
> 用真实 core 类型重写并扩展到 15 个（8 纯映射 + 7 full-stack），core 由 14 增到 20。

---

## 8. 接入真实 core 类型时发现的问题（合并的收获）

1. **core 没有项目树**：`ProjectState` 只有扁平 `Vec<AssetEntry>`（id/path/kind）。
   component 按 `path.split('/')` **推导文件夹层级**，不新增 parent 指针；
   `AssetKind::Folder` 是为了 create-folder mutation 能落盘才加的最小扩展。
   **Gap**：`ProjectAction::CreateAsset { parent: Option<AssetId> }` 无法寻址"路径推导出来的
   文件夹"（它们没有 id），component 目前只能把 `parent` 传 `None`（项目根）或显式
   Folder entry 的 id。生产建议 parent 用 path / 稳定的 FolderId。
2. **core 原本没有任何项目 mutation**（只有 AssetsDiscovered/load/preview）：合并新增
   `CreateAsset/DeleteAsset/RenameAsset` + reducer + `Effect::PersistProject`，并补 6 个 core 测试
   （folder 重命名会重写所有子路径，否则 path 推导树会漂移）。asset id 由 reducer 用
   `max+1` 确定性分配（真实 scanner 拥有 id 分配权）。
3. **`DocumentAction::Open` 直接携带 content**（ADR 的设计是 `Effect::LoadDocument` →
   `DocumentLoaded` completion）。component 只能按 kind 伪造占位内容：
   Material→`MaterialData::default()`、Toml/Text→空串。**Gap**：生产必须走异步 load effect；
   否则双击打开的文档内容永远是占位符。
4. **widget 的 TreeRow id 是 `u64`，core 身份是 `AssetId`（资产）或"没有 id"（文件夹）**：
   用 FNV-1a(path) 生成稳定 row id，click 回来后按同一算法反查 node。
   **Gap**：widget kit 若要接真实多实例/稳定 id，应接受 opaque/String id 或由 view 预留
   `NodeId -> AssetId` 映射（原型用 hash 是权宜）。
5. **selection 是跨 slice 规则**：点资产行不只是选中，还会 focus inspector + 发起 LoadAsset
   effect —— full-stack 测试验证了这条规则在"组件点击"路径上仍然成立；而点击文件夹是
   **view state（expanded set）**，绝不发 Action（测试显式断言）。
6. **文档没有字段 id / patch 概念**：Material 编辑必须整块 `SetMaterial(MaterialData)`，
   文本编辑整块 `SetText`。生产（Phase 3）建议 field-level patch + `EditTransaction.label`，
   否则 undo 粒度/合并策略做不了。
7. **egui Slider 是 drag 控件**：合成 click 不改值（合并时实测），必须 `Harness::drag`；
   且 `changed()` 只在中间帧为 true。UI 层每帧 dispatch 不受影响，但 headless 测试要么逐帧
   dispatch，要么累积 Action（原型选后者，reducer 对等值 edit 幂等）。
8. **CommandRegistry 与 toolbar 尚未接通**：core 的 `CommandId/CommandRegistry` 已存在且可
   `dispatch_command`，但组件的 toolbar item 直接产出 Action（测试可观测性更好）。
   生产建议 toolbar item id == `CommandId`，统一走 registry（菜单/快捷键/命令面板/MCP 同源）。
9. **旧 `fake_state.rs` + `actions.rs` 删除**：省掉约 250 行伪造领域模型；代价是 core
   增加 ~120 行（demo + mutation + 测试）。这正是合并的价值：伪造层不再漂移。

---

## 9. Caveats（这个原型没有证明什么）

- **headless only**：没有 GPU/窗口/字体光栅化/截图；颜色/间距/emoji 观感未验证。
- 没有 dock/多实例/wgpu 视口/`EffectExecutor`/`World` 边界；`Effect::PersistProject/PersistLayout`
  只是数据，没有真正的磁盘读写。
- 没有强制 multi-pass（未触发 `request_discard`）；双缓冲/最后一遍提交规则靠测试约定，
  没有真跑过 2-pass 场景。
- 只有 2 个 panel（Project/Inspector）；搜索简化（搜索时忽略折叠、只按 name 匹配）；
  ContextMenu 只输出请求数据，未接真正弹层；项目打开内容是占位符（见 §8.3）。
- `core_flow.rs` 中"删资产"不关闭已打开的同路径文档（真实 reducer 应处理），原型未建模。

## 10. 规模

```text
editor-core:      src ~1.5k 行 + tests 596 行
ui-widgets:       src ~2.3k 行 + tests 446 行（沿用旧原型的 14 个测试）
editor-components src ~1.1k 行 + tests 695 行
合计 ~6.3k 行（含注释/测试，不含 target/）。
```

## 11. 与旧原型的关系

本目录由两个旧原型合并而来，**验证全绿后旧目录已删除**：

- `prototypes/editor-ui-core` → `crates/editor-core`（代码搬移，仅扩展项目 mutation/demo）
- `prototypes/editor-ui-widgets` 的 `crates/kairos-ui-widgets` → `crates/ui-widgets`（只改
  package 名/import 名；14 个测试原样保留）
- `prototypes/editor-ui-widgets` 的 `crates/kairos-editor-components` → `crates/editor-components`
  （删除 fake_state/actions，改用真实 core 类型重写，测试扩到 15 个）
