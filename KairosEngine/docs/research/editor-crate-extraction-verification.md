# `kairos_editor` 抽出 — 验收证据（[#272](https://github.com/WhitePetal/KairosEngine/issues/272)）

- **日期**：2026-09-14
- **对象**：[#270](https://github.com/WhitePetal/KairosEngine/issues/270) 把 `kairos_editor` 抽出为独立 crate（纯迁移）
- **基线**：[#254](https://github.com/WhitePetal/KairosEngine/issues/254) §7 的测试与验收基线
- **口径**：本票只验证，不改代码；发现的问题按「只迁移不重构」登记，不顺手修。**首轮验证发现合并闸门红（见 §1.1）后，经决定对唯一那处既有 doctest 做了单行修复（见 §1.2 与 §5），故本文同时记录修复前 / 修复后两次 `cargo test-full`。**
- **被测树**：`HEAD = 0a7dac6`（`main`），另加一处 doctest 导入修复（`kairos_transform/src/global_transform.rs:155`）。工作区仅 `Library/asset_registry.toml`（数据文件，不参与编译 / 单测）有改动，且该改动早于本次运行。工作区根为 `KairosEngine/`（仓库根 `KairosEngine/` 的子目录）。

## 结论摘要

| 项 | 结论 |
|---|---|
| 三件套（`test-crate kairos_editor` / `check --workspace --all-targets` / `test-crate kairos_engine`） | ✅ 全绿 |
| `cargo test-full`（含 doctest，合并闸门） | 首轮 ❌ **红**（1 处，`kairos_transform` doctest）→ 修复后 ✅ **全绿** |
| `cargo check -p kairos_editor`（正向独立性） | ✅ 通过 |
| `cargo tree --invert kairos_editor`（反向独立性） | ✅ 不含 `kairos_engine` |
| 编辑器人工冒烟（0–5 步） | ⏸ **未执行**（需要人在 GUI 前；本文只给出可自动化的前置证据） |
| P0 / P1 回归项 | 逐条核对，见下；**无一由 #270 引入** |

> **验收判定**：抽出本身在编译、单测、独立性三个维度成立。首轮唯一的红是 `kairos_transform` 一处**既有** doctest 编译失败，与 #270 无因果关系（证据见 §1.2）；已按单行修复处理，修复后 `cargo test-full` **全绿**。除「编辑器人工冒烟 0–5 步」（需人在 GUI 前）外，自动化验收项**全部达成**。

---

## 1. 命令基线（三件套 + 收尾全量）

工作目录：`KairosEngine/`（工作区根）。原始日志留在**仓库根**的 `.scratch/272/`（未跟踪；绝对路径 `/Users/baiaoxiang/KairosEngine/.scratch/272/`）。

| # | 命令 | 结果 | 日志 |
|---|---|---|---|
| 1 | `cargo test-crate kairos_editor` | ✅ **74 passed; 0 failed**（`src/lib.rs`）；`src/main.rs` 0 tests | `01-test-crate-editor.log` |
| 2 | `cargo check --workspace --all-targets` | ✅ `Finished dev profile ... in 0.38s`，0 error | `02-check-workspace.log` |
| 3 | `cargo test-crate kairos_engine` | ✅ **29 passed; 0 failed**（`src/lib.rs`）；`bake_assets` 0 tests | `03-test-crate-engine.log` |
| 4 | `cargo test-full`（修复前） | ❌ **1 failed**（详见 §1.1）；其余全绿 | `04-test-full.log` |
| 4′ | `cargo test-full`（修复后） | ✅ **全绿**：38 个 `test result`，全部 `0 failed` | `07-test-full-after-fix.log` |

### 1.1 `cargo test-full` 实际记录（修复前 → 修复后）

`test-full`（= `test --workspace --no-fail-fast`）跑完了每个成员的单测与 doctest。`04-test-full.log` 里每个目标的 `test result` 均为 `ok`，唯一的 `FAILED` 出现在末位的 `-p kairos_transform --doc`：

```
failures:
    kairos_transform/src/global_transform.rs - global_transform::GlobalTransform::reparented_to (line 153)

error[E0432]: unresolved imports `kairos_ecs::Entity`, `kairos_ecs::Query`, `kairos_ecs::Component`, `kairos_ecs::Commands`, `kairos_ecs::ChildOf`
   --> kairos_transform/src/global_transform.rs:156:18
    |
156 | use kairos_ecs::{Entity, Query, Component, Commands, ChildOf};
    |                  ^^^^^^  ^^^^^  ^^^^^^^^^  ^^^^^^^^  ^^^^^^ no `ChildOf` in the root
...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

error: doctest failed, to rerun pass `-p kairos_transform --doc`
error: 1 target failed: `-p kairos_transform --doc`
```

**修复后**（`07-test-full-after-fix.log`）：

```
   Doc-tests kairos_transform

running 1 test
test kairos_transform/src/global_transform.rs - global_transform::GlobalTransform::reparented_to (line 153) ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

修复后整份日志的 38 个 `test result` 行全部 `0 failed`，无 `FAILED` / `error`。

同一全量运行里（修复前那次），**编辑器抽出的核心目标是通过的**，特别值得注意的是 #254 §4.3 点名的那 13 处 doctest：

```
   Doc-tests kairos_editor
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests kairos_ecs
test result: ok. 453 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 182.24s
```

即：**13 处路径改写（`kairos_engine::kairos_editor::…` → `kairos_editor::…`）全部改对、全部通过**；红的是另一处。

### 1.2 红的根因与修复：一处**既有** doctest，与 #270 无关

`kairos_transform/src/global_transform.rs:156` 的示例从 `kairos_ecs` **根**导入 `Entity / Query / Component / Commands / ChildOf`，而这些名字只存在于 `kairos_ecs::prelude`（`kairos_ecs/src/lib.rs:45-98` 的 `pub mod prelude`；根部只有 `pub use kairos_ptr as ptr;`）。因此这处 doctest **从来没有编译过**。

因果证据（机械、可复核）：

1. **引入点早于迁移**：该 `use kairos_ecs::{…}` 行由 [`3e91e91`](https://github.com/WhitePetal/KairosEngine/commit/3e91e91)「fix: kairos_transform」（2026-09-11 18:59）加入。
   `git log -S "use kairos_ecs::{Entity, Query, Component, Commands, ChildOf}" -- KairosEngine/kairos_transform/src/global_transform.rs` → 只有 `3e91e91`。
2. **迁移窗口内两处相关文件均未改动**：
   - `git log --oneline 2119b2c..HEAD -- KairosEngine/kairos_ecs/src/lib.rs` → 空（ECS 根导出在迁移前后逐字节相同）。
   - `global_transform.rs` 最后一次改动是 `2119b2c`，而 `2119b2c` 在 `78098bc/26770e4/78bab5e` 之前。
3. **四个迁移提交都不碰 `kairos_ecs` / `kairos_transform`**：
   `78098bc`、`26770e4`、`78bab5e`、`0a7dac6` 的 `--stat` 里没有 `kairos_ecs/` 或 `kairos_transform/` 下任何文件。
4. 与 §9 已登记的文档层既有缺陷同类（`node.rs` 残缺 fence、`create::` 拼写、egui_dock fork 死链），只是这一处是 **error**（编译失败），故会挡住 `test-full`。

**修复（单行）**：把导入改到 `kairos_ecs::prelude::{…}`：

```diff
- /// # use kairos_ecs::{Entity, Query, Component, Commands, ChildOf};
+ /// # use kairos_ecs::prelude::{Entity, Query, Component, Commands, ChildOf};
```

`prelude` 同时导出 `Component` 的 trait 与 derive 宏（`kairos_ecs/src/component.rs:19`），故 `#[derive(Component)]` 仍成立。修复后 `cargo test -p kairos_transform --doc` 与 `cargo test-full` 均通过；改动仅此一行，不触碰引擎 / 编辑器的迁移产物。

> `test-crate` 别名是 `test --lib --bins --tests`，**不含 `--doc`**，所以三件套全绿确实掩盖了它——正是 #254 §4.3 预告的口径。#255 标记的关键输入得到实测确认，只是位置在 `kairos_transform` 而非编辑器。

---

## 2. 独立性硬证据

| 方向 | 命令 | 结果 |
|---|---|---|
| 正向 | `cargo check -p kairos_editor` | ✅ `Checking kairos_editor v0.1.0` → `Finished dev profile ... in 2.66s`（编辑器只靠自身声明依赖即可编译） |
| 反向 | `cargo tree --invert kairos_editor` | ✅ 输出仅一行：`kairos_editor v0.1.0 (/…/kairos_editor)`，**不含 `kairos_engine`** |

（日志：`05-check-p-editor.log`、`06-tree-invert-editor.log`。）

---

## 3. P0 / P1 回归项逐条核对（#254 §7）

| 级别 | 回归项 | 结论 | 证据 |
|---|---|---|---|
| P0 | 起错 bin / 起不来（`default-run` 迁移） | ✅ 迁移正确（GUI 首跑仍需人） | `kairos_editor/Cargo.toml:10` `default-run = "kairos_editor"`；`cargo metadata --no-deps` 显示 17 个成员中**只有** `kairos_editor` 带 `default_run`。因工作区是 virtual manifest，另用**隔离探针**实测（`cargo 1.97.1`，virtual workspace + 3 个 bin 成员、唯一一个带 `default-run`）：裸 `cargo run` 选中该成员（打印 `ran a`）；探针工作区在仓库根 `.scratch/272/cargo-run-probe/`。故 workspace 根裸 `cargo run` 起的是编辑器。 |
| P0 | `install` 漏调 / 时机错 | ✅ | `kairos_editor/src/lib.rs:26-45`：`Engine::new()?` → `install(&mut engine.world)` → `KairosGame::new(&mut engine)`，顺序与 #256 前置条件一致；`kairos_editor::test::install_mounts_every_editor_subsystem` 通过（74 之一）。 |
| P0 | 项目树不随文件变化刷新 | ✅ 代码随迁且接线在位（GUI 观察仍需人） | `project_watcher.rs`（+`project_watcher/test.rs`）整体迁入；`ui.rs:414-416` 每帧 `project_window.poll_external_changes()`；`project_window.rs:159-200` 的 `refresh_from_disk` 整表重建并按 GUID 恢复选中。相关测试通过：watcher 2 项、`project_path_tree` 32 项（含 `refresh_picks_up_files_created_outside_the_editor_and_keeps_guids`）。 |
| P1 | 某类检查器打不开（各 loader 的 `install` 随迁 + `AssetKind` 路由） | ✅ 路由与安装在位（逐类实开仍需人） | `lib.rs:107-113` 的 `install` 覆盖 `editor_assets::text` / `editor_assets::toml` / `syntax` / `ui::inspector::texture` / `camera`；`ui/inspector/creater.rs:19-82` 的 `create_from_asseet_kind` 对 `AssetKind` **11 个变体**全部有分支。texture/material/audio/text/toml 相关测试通过。 |
| P1 | 场景无画面 / 相机不动 | ✅ 安装在位（画面/操作仍需人） | `camera.rs:194-204` `camera::install` 注册 `editor_camera_controller_system` 到 `PostUpdate`；`camera/test.rs` 通过；场景窗口代码随迁。 |
| P1 | 保存不落盘（bin 迁移后裸 `cargo run` 的 cwd 仍是 workspace root） | ✅ 路径相对 cwd，落点正确（实存仍需人） | 注册表用相对路径 `Library/asset_registry.toml`（`asset_registry.rs:208`）。`git status` 显示 `KairosEngine/Library/asset_registry.toml` 有改动——即从 workspace 根运行时文件确实写在 workspace 根下，与迁移前一致。 |
| P1 | doctest 漏改（三件套绿而 `test-full` 红） | ✅ 13 处编辑器 doctest 全绿；唯一红的是 `kairos_transform` **既有** doctest，已修复 | 见 §1.1 / §1.2；修复后 `cargo test-full` 全绿。 |
| P2 | syntax 高亮失效 | ✅ 安装在位（渲染效果仍需人） | `syntax.rs:359-362` `syntax::install` 注册 `SyntaxHighlightSettingsLoader`；`syntax::test::syntax_loads_through_the_core` 通过。 |

**未执行（需人在 GUI 前）**：上表标「仍需人」的观察项，以及 §4。

---

## 4. 编辑器人工冒烟（0–5 步）——未执行

本环境无法驱动 GUI，故 0–5 步**均未执行**。请在 GUI 前按 #272 原文逐条跑并回填本表：

| 步 | 动作 | 期望 | 结果 |
|---|---|---|---|
| 0 | workspace 根裸 `cargo run` | 起的是**编辑器**（同时验证 `default-run` 迁移与新 bin 首跑） | ☐ |
| 1 | 项目树：展开 / 刷新；改动项目内文件 | watcher 更新树 | ☐ |
| 2 | 检查器：逐个选中 texture / material / audio / text / toml | 面板打开不 panic | ☐ |
| 3 | 场景窗口 | 有画面、相机可控 | ☐ |
| 4 | 代码与 TOML 的编辑与保存 | `.rs` / `.toml` 改动落盘 | ☐ |
| 5 | 退出 | 干净退出 | ☐ |

`bake_assets`：按 #272 只随 `--all-targets` 编译，**不单跑**（单跑会重写 `imported_assets/Default`，与验收编辑器抽取无因果）。

---

## 5. 发现的问题登记

| # | 问题 | 级别 | 证据 | 归属 |
|---|---|---|---|---|
| 1 | `kairos_transform/src/global_transform.rs:153-156` doctest 从 `kairos_ecs` 根导入 `Entity/Query/Component/Commands/ChildOf`，实际只在 `kairos_ecs::prelude`；该 doctest 从未编译，令 `cargo test-full` 红 | 原阻塞合并闸门，**已修复** | §1.1 / §1.2 | **既有缺陷**（`3e91e91`），非 #270；单行把导入改到 `kairos_ecs::prelude::{…}` |

既有「明确不计回归」项（层级/控制台/关于窗口 `TODO`、右键新建五类资源 `todo!()`、`Scene → New Scene` `todo!()`、docking 拖拽 collection `todo!()`、文档层既有缺陷、`math/color/converts.rs` 反赖 egui/syntect、无 `[workspace.dependencies]`、`KairosGame` 只服务编辑器、`Engine`/`Editor`/crate 同名易混）本票复核期间**未观察到新增或恶化**；按 #272 口径不报。

---

## 6. 复现命令

```sh
cd KairosEngine            # 工作区根
cargo test-crate kairos_editor
cargo check --workspace --all-targets
cargo test-crate kairos_engine
cargo test-full            # 修复后全绿
cargo check -p kairos_editor
cargo tree --invert kairos_editor
```
