# 编辑器抽出为 `kairos_editor` crate — 事实清点（inventory）

> **来源票**：wayfinder [#255](https://github.com/WhitePetal/KairosEngine/issues/255)（map [#252](https://github.com/WhitePetal/KairosEngine/issues/252)）
> **基准**：`HEAD = 05f340f`（2026-09-14，`refactor(kairos_audio): drive audio from the Update schedule`）
> **一次性分支**：`research/editor-crate-extraction-inventory`（照 [#179](https://github.com/WhitePetal/KairosEngine/issues/179) 先例）
> **口径**：只查事实、不落实现。本清单由 grep + 读码得出，**未编译验证**；凡标注「需编译确认」处即为尚未证实的推断。
> **语言**：中文。

## 0. 范围、方法与统计

### 0.1 清点范围

| 组 | 路径 | 归属 |
|---|---|---|
| 模块根 | `kairos_engine/src/kairos_editor.rs` | 拆：`Engine` + `build_world` 留引擎，`KairosEngine` + 模块声明随走 |
| 编辑器子树 | `kairos_engine/src/kairos_editor/**` | 搬（**例外**：`schedule*` 三个文件留引擎） |
| 对话框 | `kairos_engine/src/kairos_dialog.rs` | 搬 → `dialog` |
| 入口 | `kairos_engine/src/main.rs` | 搬 → 新 crate 的 bin |

**计数**（`find` 实测）：

- `kairos_engine/src/kairos_editor/` 下共 **74 个 `.rs`**；其中 `schedule.rs`、`schedule/test.rs`、`schedule/asset_test.rs` **3 个留引擎**，**71 个搬走**。
- 加上模块根 + `kairos_dialog.rs` + `main.rs` 三个根文件，**实际搬走 74 个 `.rs`**。
- 新的 crate 布局（镜像，不重组）：`kairos_editor.rs` → `kairos_editor/src/lib.rs`；`kairos_editor/X` → `kairos_editor/src/X`；`kairos_dialog.rs` → `kairos_editor/src/dialog.rs`；`main.rs` → `kairos_editor/src/main.rs`。

### 0.2 提取方法

对每个搬走的文件：

1. **去掉文档注释行**（`///`、`//!`）与行尾 `//` 注释 —— 文档注释里的路径引用单独在 §4 处理。
2. 抓取所有 `crate::…` 代码引用，以及 `use crate::{ … }` 花括号组内**每一顶层的首段标识符**（深度感知匹配，覆盖多层嵌套的 `use crate::{ a, graphics::{…}, … }`）。
3. 按首段判定**归属**：`kairos_editor` / `kairos_dialog` → 编辑器内部；其余 → 引擎侧。
4. 生成改写后的写法。

`super::` / `self::` 引用**不需要改写** —— 新 crate 的模块层级与原 `kairos_editor` 子树逐级对应（镜像布局）。

---

## 1. 逐文件归属与 import 改写

### 1.1 改写规则总表

**A. 引擎侧**（原 `crate::X` → `kairos_engine::X`）

| 原写法 | 新写法 | 备注 |
|---|---|---|
| `crate::asset` | `kairos_engine::asset` | `lib.rs:18` `pub use kairos_asset as asset` |
| `crate::audio` | `kairos_engine::audio` | `lib.rs:23` `pub use kairos_audio as audio` |
| `crate::graphics` | `kairos_engine::graphics` | `lib.rs:42` `pub use kairos_graphics as graphics` |
| `crate::physics` | `kairos_engine::physics` | `lib.rs:37` `pub use kairos_physics as physics` |
| `crate::time` | `kairos_engine::time` | `lib.rs:46` `pub use kairos_time as time` |
| `crate::math` | `kairos_engine::math` | `math.rs:3` `pub use kairos_math::*` |
| `crate::inputs` | `kairos_engine::inputs` | |
| `crate::log` | `kairos_engine::log` | 指引擎自带 `Log` 模块，**不是**外部 crate `log` |
| `crate::spatial` | `kairos_engine::spatial` | |
| `crate::kairos_paths` | `kairos_engine::kairos_paths` | |
| `crate::kairos_settings` | `kairos_engine::kairos_settings` | |
| `crate::kairos_ui` | `kairos_engine::kairos_ui` | |
| `crate::kairos_game` | `kairos_engine::kairos_game` | |
| `crate::schedule` | `kairos_engine::schedule` | `schedule` 从 `kairos_editor::schedule` 归位到 `kairos_engine::schedule` |

**B. 编辑器侧**（原 `crate::kairos_editor::X` → `crate::X`）

| 原写法 | 新写法 | 备注 |
|---|---|---|
| `crate::kairos_editor`（单独） | `crate` | crate 根 |
| `crate::kairos_editor::X` | `crate::X` | `X` ∈ `ui` / `runtime` / `camera` / `consts` / `syntax` / `editor_assets` / `asset_registry` / `project_path_tree` / `project_watcher` / `dialog` |
| `crate::kairos_dialog` | `crate::dialog` | `kairos_dialog.rs` → `dialog.rs` |
| `super::` / `self::` | 不变 | 布局镜像 |

**C. 例外（必须拆组 / 特殊处理）**

| 原写法 | 新写法 | 为什么特殊 |
|---|---|---|
| `crate::kairos_editor::Engine` | `kairos_engine::Engine` | `Engine` 归位引擎，不在编辑器 crate 里 |
| `crate::kairos_editor::build_world` | `kairos_engine::build_world`（可见性见 §7.7） | 同上 |
| `crate::kairos_editor::schedule` | `kairos_engine::schedule` | 同上 |

### 1.2 关键特例：`crate::kairos_editor::{…}` 组必须**逐项拆开**

这是本清单最容易机械改错的地方：编辑器文件大量使用

```rust
use crate::{
    graphics::…,
    kairos_editor::{
        Engine,
        asset_registry::AssetKind,
        ui::{…},
    },
    log::Log,
};
```

这个组里 **`Engine` 属引擎侧，其余属编辑器侧**，不能整组平移。改写后应是：

```rust
use kairos_engine::{
    graphics::…,
    Engine,
    log::Log,
};
use crate::{
    asset_registry::AssetKind,
    ui::{…},
};
```

`Engine` 出现在以下 **14 个搬走的文件**里（均为 `use` 或函数签名）：

| 文件 | `Engine` 的引用行 |
|---|---|
| `ui.rs` | `:14`（组内）、`:224`、`:267`、`:273`、`:372`、`:414`、`:778`、`:833` |
| `ui/docking_tab.rs` | `:4`（组内）、`:192`、`:213`、`:462`、`:482`、`:775`、`:812`、`:1096`、`:2280` |
| `ui/docking_tab/tab_drawer.rs` | `:2`（组内）、`:33` |
| `ui/project_window.rs` | `:9`（组内）、`:532`、`:622` |
| `ui/layout.rs` | `:6`（组内）、`:46`、`:53`、`:97`、`:104`、`:143`、`:150`、`:189`、`:196` |
| `ui/inspector/audio.rs` | `:15`（组内）、`:324`、`:334`、`:402` |
| `ui/inspector_window.rs` | `:10`（组内）、`:158`、`:211` |
| `ui/scene_window.rs` | `:21`（组内）、`:144`、`:363` |
| `ui/game_window.rs` | `:20`（组内）、`:93`、`:180` |
| `ui/tool_bar.rs` | `:11`（组内）、`:148`、`:342` |
| `ui/hierarchy_window.rs` | `:5`（组内）、`:81`、`:107` |
| `ui/about_window.rs` | `:15`（`use crate::kairos_editor::{Engine, consts};` → `use crate::consts;` + `use kairos_engine::Engine;`）、`:93`、`:177` |
| `ui/console_window.rs` | `:4`（组内）、`:79`、`:109` |
| `ui/preferences_window.rs` | `:5`（组内）、`:108`、`:179` |

另：`kairos_engine/src/kairos_editor.rs` 自身是 `Engine` 的定义处（`:25`、`:30`），该文件拆成 `kairos_engine/src/engine.rs`（`Engine` + `build_world`）与 `kairos_editor/src/lib.rs`（`KairosEngine`→`Editor` + 模块声明）。

**其余需要「拆组」的成员**：`consts`（编辑器内部，`about_window.rs:15`）、`runtime`（编辑器内部）、`ui::paths`（编辑器内部，`runtime.rs:36`）。

### 1.3 逐文件清单

> 键：`engine` = 改写为 `kairos_engine::…`；`editor` = 改写为 `crate::…`（新 crate 内）。每行是该文件去重后的引用目标。

### kairos_engine/src/kairos_dialog.rs  ->  kairos_editor/src/dialog.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor.rs  ->  kairos_editor/src/lib.rs
- engine: `kairos_engine::asset::AssetOptions::new`, `kairos_engine::asset::install`, `kairos_engine::audio`, `kairos_engine::audio::audio::install`, `kairos_engine::audio::audio_ext::install`, `kairos_engine::audio::audio_ext::pcm::install`, `kairos_engine::audio::install`, `kairos_engine::graphics`, `kairos_engine::inputs`, `kairos_engine::kairos_game`, `kairos_engine::kairos_ui::font::install`, `kairos_engine::log`, `kairos_engine::physics`, `kairos_engine::time`
- editor: `crate::editor_assets::text::install`, `crate::editor_assets::toml::install`, `crate::syntax::install`, `crate::ui::inspector::texture::install`

### kairos_engine/src/kairos_editor/asset_registry.rs  ->  kairos_editor/src/asset_registry.rs
- engine: `kairos_engine::asset::AssetOptions::DEFAULT_PROCESSED_FILE_PATH`, `kairos_engine::asset::io::get_meta_path`

### kairos_engine/src/kairos_editor/camera.rs  ->  kairos_editor/src/camera.rs
- engine: `kairos_engine::math`, `kairos_engine::time`
- editor: `crate`

### kairos_engine/src/kairos_editor/camera/test.rs  ->  kairos_editor/src/camera/test.rs
- engine: `kairos_engine::graphics`, `kairos_engine::math`, `kairos_engine::time`
- editor: `crate`

### kairos_engine/src/kairos_editor/consts.rs  ->  kairos_editor/src/consts.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/editor_assets.rs  ->  kairos_editor/src/editor_assets.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/editor_assets/text.rs  ->  kairos_editor/src/editor_assets/text.rs
- engine: `kairos_engine::asset`

### kairos_engine/src/kairos_editor/editor_assets/toml.rs  ->  kairos_editor/src/editor_assets/toml.rs
- engine: `kairos_engine::asset`

### kairos_engine/src/kairos_editor/project_path_tree.rs  ->  kairos_editor/src/project_path_tree.rs
- editor: `crate`, `crate::dialog`

### kairos_engine/src/kairos_editor/project_path_tree/create_request.rs  ->  kairos_editor/src/project_path_tree/create_request.rs
- editor: `crate::asset_registry::AssetKind`

### kairos_engine/src/kairos_editor/project_path_tree/test.rs  ->  kairos_editor/src/project_path_tree/test.rs
- editor: `crate`

### kairos_engine/src/kairos_editor/project_path_tree/texture_path.rs  ->  kairos_editor/src/project_path_tree/texture_path.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/project_path_tree/tree_node.rs  ->  kairos_editor/src/project_path_tree/tree_node.rs
- editor: `crate::asset_registry`

### kairos_engine/src/kairos_editor/project_watcher.rs  ->  kairos_editor/src/project_watcher.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/project_watcher/test.rs  ->  kairos_editor/src/project_watcher/test.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/runtime.rs  ->  kairos_editor/src/runtime.rs
- engine: `kairos_engine::graphics`, `kairos_engine::kairos_paths`, `kairos_engine::kairos_settings`, `kairos_engine::math::float4x4`
- editor: `crate`, `crate::dialog`

### kairos_engine/src/kairos_editor/syntax.rs  ->  kairos_editor/src/syntax.rs
- engine: `kairos_engine::asset`, `kairos_engine::math`

### kairos_engine/src/kairos_editor/ui.rs  ->  kairos_editor/src/ui.rs
- engine: `kairos_engine::asset::Handle`, `kairos_engine::graphics`, `kairos_engine::log`, `kairos_engine::math`
- editor: `crate`, `crate::dialog`, `crate::ui::docking_tab::dock_state::tree::node::Node`

### kairos_engine/src/kairos_editor/ui/about_window.rs  ->  kairos_editor/src/ui/about_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log::Log`
- editor: `crate`, `crate::dialog`, `crate::ui`, `crate::ui::docking_tab::window_state::WindowState`

### kairos_engine/src/kairos_editor/ui/console_window.rs  ->  kairos_editor/src/ui/console_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/dialog.rs  ->  kairos_editor/src/ui/dialog.rs
- editor: `crate::ui`

### kairos_engine/src/kairos_editor/ui/docking_tab.rs  ->  kairos_editor/src/ui/docking_tab.rs
- engine: `kairos_engine::log`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state.rs  ->  kairos_editor/src/ui/docking_tab/dock_state.rs
- editor: `crate::ui::docking_tab`

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree.rs  ->  kairos_editor/src/ui/docking_tab/dock_state/tree.rs
- editor: `crate::ui::docking_tab`

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node.rs  ->  kairos_editor/src/ui/docking_tab/dock_state/tree/node.rs
- editor: `crate::ui::docking_tab::dock_state::tree`

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node/leaf_node.rs  ->  kairos_editor/src/ui/docking_tab/dock_state/tree/node/leaf_node.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node/split_node.rs  ->  kairos_editor/src/ui/docking_tab/dock_state/tree/node/split_node.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/test.rs  ->  kairos_editor/src/ui/docking_tab/dock_state/tree/test.rs
- editor: `crate::ui::docking_tab::dock_state::tree::Tree`

### kairos_engine/src/kairos_editor/ui/docking_tab/drag_and_drop.rs  ->  kairos_editor/src/ui/docking_tab/drag_and_drop.rs
- editor: `crate::ui::docking_tab`

### kairos_engine/src/kairos_editor/ui/docking_tab/state.rs  ->  kairos_editor/src/ui/docking_tab/state.rs
- editor: `crate::ui::docking_tab`

### kairos_engine/src/kairos_editor/ui/docking_tab/styles.rs  ->  kairos_editor/src/ui/docking_tab/styles.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/docking_tab/surfaces.rs  ->  kairos_editor/src/ui/docking_tab/surfaces.rs
- editor: `crate::ui::docking_tab`

### kairos_engine/src/kairos_editor/ui/docking_tab/tab_drawer.rs  ->  kairos_editor/src/ui/docking_tab/tab_drawer.rs
- engine: `kairos_engine::log`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/docking_tab/translations.rs  ->  kairos_editor/src/ui/docking_tab/translations.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/docking_tab/window_state.rs  ->  kairos_editor/src/ui/docking_tab/window_state.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/drag.rs  ->  kairos_editor/src/ui/drag.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/egui_ext.rs  ->  kairos_editor/src/ui/egui_ext.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/game_window.rs  ->  kairos_editor/src/ui/game_window.rs
- engine: `kairos_engine::graphics`, `kairos_engine::graphics::attachment::AttachmentFormat::D24S8`, `kairos_engine::graphics::attachment::AttachmentFormat::RGBA8UNorm`, `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log::Log`, `kairos_engine::math`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/global_styles.rs  ->  kairos_editor/src/ui/global_styles.rs
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/hierarchy_window.rs  ->  kairos_editor/src/ui/hierarchy_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/ide_detection.rs  ->  kairos_editor/src/ui/ide_detection.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/inspector.rs  ->  kairos_editor/src/ui/inspector.rs
- engine: `kairos_engine::graphics`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/inspector/audio.rs  ->  kairos_editor/src/ui/inspector/audio.rs
- engine: `kairos_engine::asset`, `kairos_engine::audio`, `kairos_engine::math`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`, `crate::ui::Message::AudioInspectorSeekPreview`, `crate::ui::Message::AudioInspectorTogglePreview`, `crate::ui::Messager`

### kairos_engine/src/kairos_editor/ui/inspector/audio/test.rs  ->  kairos_editor/src/ui/inspector/audio/test.rs
- engine: `kairos_engine::audio`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/inspector/code.rs  ->  kairos_editor/src/ui/inspector/code.rs
- engine: `kairos_engine::asset`, `kairos_engine::math`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/creater.rs  ->  kairos_editor/src/ui/inspector/creater.rs
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/inspector/directory.rs  ->  kairos_editor/src/ui/inspector/directory.rs
- editor: `crate::project_path_tree::ProjectPathGraph`, `crate::ui`, `crate::ui::Messager`

### kairos_engine/src/kairos_editor/ui/inspector/document.rs  ->  kairos_editor/src/ui/inspector/document.rs
- engine: `kairos_engine::asset`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/font.rs  ->  kairos_editor/src/ui/inspector/font.rs
- engine: `kairos_engine::asset`, `kairos_engine::kairos_ui`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/material.rs  ->  kairos_editor/src/ui/inspector/material.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics`, `kairos_engine::graphics::texture::Texture`, `kairos_engine::graphics::texture::format::decode`, `kairos_engine::math`, `kairos_engine::spatial`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/inspector/material/test.rs  ->  kairos_editor/src/ui/inspector/material/test.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics::compare_function::CompareFunction`, `kairos_engine::graphics::material::Material`, `kairos_engine::graphics::material::SerializedMaterial`, `kairos_engine::graphics::render_state`
- editor: `crate::ui::inspector::material::MaterialInspector`

### kairos_engine/src/kairos_editor/ui/inspector/mesh.rs  ->  kairos_editor/src/ui/inspector/mesh.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics`, `kairos_engine::graphics::vertex::Vertex`, `kairos_engine::math`, `kairos_engine::spatial`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/shader.rs  ->  kairos_editor/src/ui/inspector/shader.rs
- engine: `kairos_engine::asset`, `kairos_engine::math`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/texture/edit.rs  ->  kairos_editor/src/ui/inspector/texture/edit.rs
- engine: `kairos_engine::asset`, `kairos_engine::asset::AssetMetaDyn::serialize`, `kairos_engine::asset::io::get_meta_path`, `kairos_engine::asset::meta::processor_name`, `kairos_engine::graphics::texture`, `kairos_engine::graphics::texture::TextureProcessor`, `kairos_engine::graphics::texture::TextureSettings`

### kairos_engine/src/kairos_editor/ui/inspector/texture/mod.rs  ->  kairos_editor/src/ui/inspector/texture/mod.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics`, `kairos_engine::graphics::texture`, `kairos_engine::graphics::texture::PixelDatas`, `kairos_engine::graphics::texture::PixelDatas::U8`, `kairos_engine::graphics::texture::format::decode`, `kairos_engine::graphics::texture::format::encode`, `kairos_engine::graphics::texture::sampler::MipmapConfig`, `kairos_engine::kairos_paths`, `kairos_engine::kairos_settings`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/toml.rs  ->  kairos_editor/src/ui/inspector/toml.rs
- engine: `kairos_engine::asset`, `kairos_engine::math`
- editor: `crate`, `crate::project_path_tree::ProjectPathGraph`

### kairos_engine/src/kairos_editor/ui/inspector/unknown.rs  ->  kairos_editor/src/ui/inspector/unknown.rs
- editor: `crate::project_path_tree::ProjectPathGraph`, `crate::ui`, `crate::ui::Messager`

### kairos_engine/src/kairos_editor/ui/inspector_window.rs  ->  kairos_editor/src/ui/inspector_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/layout.rs  ->  kairos_editor/src/ui/layout.rs
- engine: `kairos_engine::graphics`, `kairos_engine::log`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/native_dialog.rs  ->  kairos_editor/src/ui/native_dialog.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/paths.rs  ->  kairos_editor/src/ui/paths.rs
- 无 `crate::` 引用

### kairos_engine/src/kairos_editor/ui/preferences_window.rs  ->  kairos_editor/src/ui/preferences_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`, `kairos_engine::math`, `kairos_engine::math::float2::from_array`, `kairos_engine::math::float3::from_array_4`, `kairos_engine::math::float4::new`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/project_window.rs  ->  kairos_editor/src/ui/project_window.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`
- editor: `crate`, `crate::ui`

### kairos_engine/src/kairos_editor/ui/project_window/content_panel.rs  ->  kairos_editor/src/ui/project_window/content_panel.rs
- engine: `kairos_engine::math`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/project_window/context_menu.rs  ->  kairos_editor/src/ui/project_window/context_menu.rs
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/project_window/hierarchy_panel.rs  ->  kairos_editor/src/ui/project_window/hierarchy_panel.rs
- engine: `kairos_engine::math`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/scene_window.rs  ->  kairos_editor/src/ui/scene_window.rs
- engine: `kairos_engine::graphics`, `kairos_engine::graphics::attachment::AttachmentFormat::D24S8`, `kairos_engine::graphics::attachment::AttachmentFormat::RGBA8UNorm`, `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log::Log`, `kairos_engine::math`
- editor: `crate`, `crate::dialog`

### kairos_engine/src/kairos_editor/ui/scene_window/gizmos.rs  ->  kairos_editor/src/ui/scene_window/gizmos.rs
- engine: `kairos_engine::graphics`
- editor: `crate`

### kairos_engine/src/kairos_editor/ui/scene_window/gizmos/axes_indicator.rs  ->  kairos_editor/src/ui/scene_window/gizmos/axes_indicator.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics`, `kairos_engine::math`

### kairos_engine/src/kairos_editor/ui/scene_window/gizmos/grid_plane.rs  ->  kairos_editor/src/ui/scene_window/gizmos/grid_plane.rs
- engine: `kairos_engine::asset`, `kairos_engine::graphics`, `kairos_engine::math`

### kairos_engine/src/kairos_editor/ui/tool_bar.rs  ->  kairos_editor/src/ui/tool_bar.rs
- engine: `kairos_engine::graphics::graphics_graph::GraphicsCommand`, `kairos_engine::log`, `kairos_engine::math`
- editor: `crate`, `crate::dialog`

### kairos_engine/src/kairos_editor/ui/ui_style_fields.rs  ->  kairos_editor/src/ui/ui_style_fields.rs
- engine: `kairos_engine::math`

### kairos_engine/src/main.rs  ->  kairos_editor/src/main.rs
- 无 `crate::` 引用

### 1.4 全量 distinct 映射表

| 原写法 | 归属 | 新写法 | 出现次数 |
|---|---|---|---|
| `audio` | engine | `kairos_engine::audio` | 3 |
| `crate::asset` | engine | `kairos_engine::asset` | 16 |
| `crate::asset::AssetMetaDyn::serialize` | engine | `kairos_engine::asset::AssetMetaDyn::serialize` | 1 |
| `crate::asset::AssetOptions::DEFAULT_PROCESSED_FILE_PATH` | engine | `kairos_engine::asset::AssetOptions::DEFAULT_PROCESSED_FILE_PATH` | 1 |
| `crate::asset::AssetOptions::new` | engine | `kairos_engine::asset::AssetOptions::new` | 1 |
| `crate::asset::Handle` | engine | `kairos_engine::asset::Handle` | 1 |
| `crate::asset::install` | engine | `kairos_engine::asset::install` | 1 |
| `crate::asset::io::get_meta_path` | engine | `kairos_engine::asset::io::get_meta_path` | 2 |
| `crate::asset::meta::processor_name` | engine | `kairos_engine::asset::meta::processor_name` | 1 |
| `crate::audio::audio::install` | engine | `kairos_engine::audio::audio::install` | 1 |
| `crate::audio::audio_ext::install` | engine | `kairos_engine::audio::audio_ext::install` | 1 |
| `crate::audio::audio_ext::pcm::install` | engine | `kairos_engine::audio::audio_ext::pcm::install` | 1 |
| `crate::audio::install` | engine | `kairos_engine::audio::install` | 1 |
| `crate::graphics` | engine | `kairos_engine::graphics` | 1 |
| `crate::graphics::attachment::AttachmentFormat::D24S8` | engine | `kairos_engine::graphics::attachment::AttachmentFormat::D24S8` | 2 |
| `crate::graphics::attachment::AttachmentFormat::RGBA8UNorm` | engine | `kairos_engine::graphics::attachment::AttachmentFormat::RGBA8UNorm` | 2 |
| `crate::graphics::compare_function::CompareFunction` | engine | `kairos_engine::graphics::compare_function::CompareFunction` | 1 |
| `crate::graphics::graphics_graph::GraphicsCommand` | engine | `kairos_engine::graphics::graphics_graph::GraphicsCommand` | 9 |
| `crate::graphics::material::Material` | engine | `kairos_engine::graphics::material::Material` | 1 |
| `crate::graphics::material::SerializedMaterial` | engine | `kairos_engine::graphics::material::SerializedMaterial` | 1 |
| `crate::graphics::render_state` | engine | `kairos_engine::graphics::render_state` | 1 |
| `crate::graphics::texture` | engine | `kairos_engine::graphics::texture` | 2 |
| `crate::graphics::texture::PixelDatas` | engine | `kairos_engine::graphics::texture::PixelDatas` | 1 |
| `crate::graphics::texture::PixelDatas::U8` | engine | `kairos_engine::graphics::texture::PixelDatas::U8` | 1 |
| `crate::graphics::texture::Texture` | engine | `kairos_engine::graphics::texture::Texture` | 1 |
| `crate::graphics::texture::TextureProcessor` | engine | `kairos_engine::graphics::texture::TextureProcessor` | 1 |
| `crate::graphics::texture::TextureSettings` | engine | `kairos_engine::graphics::texture::TextureSettings` | 1 |
| `crate::graphics::texture::format::decode` | engine | `kairos_engine::graphics::texture::format::decode` | 2 |
| `crate::graphics::texture::format::encode` | engine | `kairos_engine::graphics::texture::format::encode` | 1 |
| `crate::graphics::texture::sampler::MipmapConfig` | engine | `kairos_engine::graphics::texture::sampler::MipmapConfig` | 1 |
| `crate::graphics::vertex::Vertex` | engine | `kairos_engine::graphics::vertex::Vertex` | 1 |
| `crate::kairos_ui::font::install` | engine | `kairos_engine::kairos_ui::font::install` | 1 |
| `crate::log::Log` | engine | `kairos_engine::log::Log` | 3 |
| `crate::math` | engine | `kairos_engine::math` | 2 |
| `crate::math::float2::from_array` | engine | `kairos_engine::math::float2::from_array` | 1 |
| `crate::math::float3::from_array_4` | engine | `kairos_engine::math::float3::from_array_4` | 1 |
| `crate::math::float4::new` | engine | `kairos_engine::math::float4::new` | 1 |
| `crate::math::float4x4` | engine | `kairos_engine::math::float4x4` | 1 |
| `graphics` | engine | `kairos_engine::graphics` | 14 |
| `inputs` | engine | `kairos_engine::inputs` | 1 |
| `kairos_game` | engine | `kairos_engine::kairos_game` | 1 |
| `kairos_paths` | engine | `kairos_engine::kairos_paths` | 2 |
| `kairos_settings` | engine | `kairos_engine::kairos_settings` | 2 |
| `kairos_ui` | engine | `kairos_engine::kairos_ui` | 1 |
| `log` | engine | `kairos_engine::log` | 11 |
| `math` | engine | `kairos_engine::math` | 17 |
| `physics` | engine | `kairos_engine::physics` | 1 |
| `spatial` | engine | `kairos_engine::spatial` | 2 |
| `time` | engine | `kairos_engine::time` | 3 |
| `crate::kairos_dialog` | editor | `crate::dialog` | 1 |
| `crate::kairos_editor` | editor | `crate` | 5 |
| `crate::kairos_editor::asset_registry` | editor | `crate::asset_registry` | 1 |
| `crate::kairos_editor::asset_registry::AssetKind` | editor | `crate::asset_registry::AssetKind` | 1 |
| `crate::kairos_editor::editor_assets::text::install` | editor | `crate::editor_assets::text::install` | 1 |
| `crate::kairos_editor::editor_assets::toml::install` | editor | `crate::editor_assets::toml::install` | 1 |
| `crate::kairos_editor::project_path_tree::ProjectPathGraph` | editor | `crate::project_path_tree::ProjectPathGraph` | 10 |
| `crate::kairos_editor::syntax::install` | editor | `crate::syntax::install` | 1 |
| `crate::kairos_editor::ui` | editor | `crate::ui` | 10 |
| `crate::kairos_editor::ui::Message::AudioInspectorSeekPreview` | editor | `crate::ui::Message::AudioInspectorSeekPreview` | 1 |
| `crate::kairos_editor::ui::Message::AudioInspectorTogglePreview` | editor | `crate::ui::Message::AudioInspectorTogglePreview` | 1 |
| `crate::kairos_editor::ui::Messager` | editor | `crate::ui::Messager` | 3 |
| `crate::kairos_editor::ui::docking_tab` | editor | `crate::ui::docking_tab` | 5 |
| `crate::kairos_editor::ui::docking_tab::dock_state::tree` | editor | `crate::ui::docking_tab::dock_state::tree` | 1 |
| `crate::kairos_editor::ui::docking_tab::dock_state::tree::Tree` | editor | `crate::ui::docking_tab::dock_state::tree::Tree` | 1 |
| `crate::kairos_editor::ui::docking_tab::dock_state::tree::node::Node` | editor | `crate::ui::docking_tab::dock_state::tree::node::Node` | 1 |
| `crate::kairos_editor::ui::docking_tab::window_state::WindowState` | editor | `crate::ui::docking_tab::window_state::WindowState` | 1 |
| `crate::kairos_editor::ui::inspector::material::MaterialInspector` | editor | `crate::ui::inspector::material::MaterialInspector` | 1 |
| `crate::kairos_editor::ui::inspector::texture::install` | editor | `crate::ui::inspector::texture::install` | 1 |
| `kairos_dialog` | editor | `crate::dialog` | 5 |
| `kairos_editor` | editor | `crate` | 31 |

### 1.5 留在引擎的 `schedule` 子树

这三个文件不回编辑器 crate，但要换 crate 内位置（`kairos_editor::schedule` → `kairos_engine::schedule`）：

| 原路径 | 新路径 |
|---|---|
| `kairos_engine/src/kairos_editor/schedule.rs` | `kairos_engine/src/schedule.rs` |
| `kairos_engine/src/kairos_editor/schedule/test.rs` | `kairos_engine/src/schedule/test.rs` |
| `kairos_engine/src/kairos_editor/schedule/asset_test.rs` | `kairos_engine/src/schedule/asset_test.rs` |

该子树的 `crate::` 引用清单（实测）：

| 现写法 | 归属 | 新写法 |
|---|---|---|
| `crate::asset`、`crate::audio`、`crate::graphics`、`crate::time`、`crate::kairos_ui::font::Font` | 引擎内部 | **不变**（仍在 `kairos_engine` 内解析） |
| `crate::kairos_editor::build_world` | 引擎内部（`build_world` 留引擎） | `crate::engine::build_world`（或同文件重导出后的路径） |
| `crate::kairos_editor::editor_assets::{Text, Toml}` | **编辑器侧** | `kairos_editor::editor_assets::{Text, Toml}` —— 这会让引擎反赖编辑器，**必须改为别的方案**（见 §9 #2） |
| `crate::kairos_editor::syntax::SyntaxHighlightSettings` | **编辑器侧** | 同上 |

另：`schedule.rs:298` 的 `install` 是 `pub(crate)`（见 §9 #3）。

---

## 2. 依赖集

计数与检索口径：git 根为仓库根，workspace 根为其下的 `KairosEngine/`；`kairos_engine/src/kairos_editor/` 树下实测 **74 个 `.rs`**，其中 `schedule.rs` + `schedule/asset_test.rs` + `schedule/test.rs` 这 **3 个留在引擎**（成为 `kairos_engine::schedule`），其余 **71 个**随编辑器搬走；再加 `src/kairos_editor.rs`、`src/kairos_dialog.rs`、`src/main.rs`，实际搬走 **74 个 `.rs`**（与原估计 76 有 2 的出入，按实测口径）。「编辑器文件集」= `src/kairos_editor/**`（不含 `schedule*` 子树）+ 上述 3 个根文件；「engine 非编辑器代码」= `src/**` 排除 `kairos_editor/**`、`kairos_editor.rs`、`kairos_dialog.rs`、`main.rs`（`schedule.rs`/`schedule/**` 需保留，单独补测）。判定「直接引用」用 `\b<crate>::`、`use <crate>…` 与 `<crate>` 词匹配；`crate::graphics`/`crate::asset`/`crate::audio`/`crate::physics`/`crate::time` 等经 `kairos_engine` 再导出的路径不计为对底层 `kairos_*` crate 的直接依赖。

### 2.0 总表

> 「engine 保留?」指把编辑器搬走后，`kairos_engine` 是否仍需该依赖。

**A. 外部 crate**

| crate | 编辑器直接引用（file:line 证据） | 新 crate 依赖? | engine 保留? | engine 侧非编辑器引用点 |
|---|---|---|---|---|
| `toml` | `ui/project_window.rs:34` `use toml::from_str`；`editor_assets.rs:5` `pub use toml::Toml`；`ui/inspector/toml.rs:9` | ✅ 必需 | ❌ 可摘除 | 无（`toml::` 在 engine 非编辑器 0 命中；`kairos_settings.rs:9`/`kairos_paths.rs:1` 只是注释与路径字符串） |
| `anyhow` | 无 | ❌ | ❌ | 无（两侧 `anyhow::` 均 0 命中） |
| `async-fs` | `ui/inspector/texture/edit.rs:52,130`；`syntax.rs:312` | ✅ 必需 | ❌ 可摘除 | 无 |
| `rand` | 无 | ❌ | ✅ 保留 | `kairos_game.rs:392` `rand::random_range` |
| `smallvec` | 无 | ❌ | ✅ 保留 | `kairos_game.rs:390` `smallvec::smallvec![…]` |
| `crossbeam-channel` | `project_watcher.rs:22,54` | ✅ 必需 | ❌ 可摘除 | 无 |
| `bytemuck` | 无 | ❌ | ❌ | 无（`\bbytemuck\b` 0 命中，含 derive 形式） |
| `tokio` | `ui/inspector/mesh.rs:83,460`；`material.rs:229,1228`；`scene_window.rs:58,477,549`；`game_window.rs:34,264,276`；`ui.rs:111,117`；`main.rs:7` `#[tokio::main]` | ✅ 必需 | ❌ 可摘除 | 无（engine 非编辑器 `tokio::` 0 命中；`asset_pipeline` 走 `pollster`+`kairos_tasks`） |
| `rayon` | 无 | ❌ | ❌ | 无 |
| `rkyv` | 无 | ❌ | ✅ 保留 | `asset_pipeline/test.rs:95` `rkyv::from_bytes`（测试；可降级 dev-dep，需编译确认） |
| `petgraph` | `project_path_tree.rs:10,342`；`ui/project_window.rs:32`；`ui/*/context_menu.rs:2`；`content_panel.rs:5`；`hierarchy_panel.rs:2`；`ui.rs:128,130,132,136,140,142,144` | ✅ 必需 | ❌ 可摘除 | 无 |
| `winit` | `runtime.rs:9,171,461,491,492,493`；`kairos_editor.rs:12`；`main.rs:5` | ✅ 必需 | ✅ 保留 | `inputs.rs:3,102,105,111,130,132`；`kairos_game.rs:173,177,181,185` |
| `env_logger` | `main.rs:9,10` | ✅ 必需（bin） | ✅ 保留 | `bin/bake_assets.rs:14,15` |
| `log` | `ui/project_window.rs:122` 等大量 `log::warn!/error!/info!/debug!` | ✅ 必需 | ❌ 可摘除 | 无（engine 非编辑器 `log::` **0 命中**；`log.rs` 只用 `eprintln!`，见 `log.rs:68-70`） |
| `open` | `ui/ide_detection.rs:213,254` | ✅ 必需 | ❌ 可摘除 | 无 |
| `native-dialog` | `ui/native_dialog.rs:3`；`kairos_dialog.rs:3` | ✅ 必需 | ❌ 可摘除 | 无 |
| `serde` | `ui/inspector/toml.rs:8` 等约 23 处 `use serde::{Deserialize, Serialize}` | ✅ 必需 | ✅ 保留 | `kairos_settings.rs:5`；`math/color/serialies.rs:1,8,17,19` |
| `sonic-rs` | 无 | ❌ | ❌ | 无 |
| `strum` | `asset_registry.rs:8` `EnumIter, IntoEnumIterator`；`inspector/mesh.rs:4,209`；`material.rs:11`；`texture/mod.rs:16,876` | ✅ 必需 | ❌ 可摘除 | 无 |
| `duplicate` | `ui/docking_tab.rs:42` `use duplicate::duplicate` | ✅ 必需 | ❌ 可摘除 | 无 |
| `paste` | `ui/docking_tab.rs:43` `use paste::paste` | ✅ 必需 | ❌ 可摘除 | 无 |
| `image` | `runtime.rs:37`；`ui/tool_bar.rs:96,105`；`ui/inspector/texture/mod.rs:210,212,216,257,258,262` | ✅ 必需 | ❌ 可摘除 | 无 |
| `base64` | 无 | ❌ | ❌ | 无 |
| `wgpu` | `runtime.rs:362-373`（`wgpu::CurrentSurfaceTexture::*`） | ✅ 必需 | ❌ 可摘除 | 无（engine 非编辑器 0 命中；`kairos_graphics` 自带其 wgpu） |
| `pollster` | `runtime.rs:190`（非测试）；`texture/edit.rs:185`（测试） | ✅ 必需 | ✅ 保留 | `asset_pipeline.rs:95` |
| `egui` | `kairos_editor.rs:10` `use egui::Visuals`；`ui.rs:35`；`runtime.rs:7`；全局约 417 处 | ✅ 必需 | ✅ 保留 | `math/color/converts.rs:3,5,9,10` `impl From<Color32> for egui::Color32` |
| `egui_extras` | `runtime.rs:104` `install_image_loaders`；`inspector/code.rs:125,132`；`mesh.rs:7`；`material.rs:7,338,373`；`toml.rs:4`；`texture/mod.rs:12,869,980`；`syntax.rs:23,341` | ✅ 必需 | ❌ 可摘除 | 无 |
| `emath` | `ui/docking_tab.rs:15` `use emath::TSTransform`；`ui/docking_tab/drag_and_drop.rs:6` `use emath::{GuiRounding, inverse_lerp}` | ✅ 必需（显式） | ❌ 可摘除 | 无 |
| `epaint` | `ui/docking_tab.rs:16` `use epaint::TextShape` | ✅ 必需（显式） | ❌ 可摘除 | 无 |
| `egui-winit` | `runtime.rs:93,180,292` | ✅ 必需 | ❌ 可摘除 | 无 |
| `egui-wgpu` | `runtime.rs:311` `egui_wgpu::ScreenDescriptor` | ✅ 必需 | ❌ 可摘除 | 无 |
| `parking_lot` | `ui/project_window.rs:31`；`inspector/{code,mesh,shader,material,toml,document}.rs`；`texture/mod.rs:14,98,351`；`content_panel.rs:4`；`tool_bar.rs:7`；`runtime.rs:8`；`ui.rs:153…` | ✅ 必需 | ❌ 可摘除 | 无 |
| `gltf` | 无 | ❌ | ❌ | 无（两侧均 0 命中；疑似遗留依赖，摘除前需编译确认） |
| `uuid` | `asset_registry.rs:9`；`project_path_tree.rs:626` | ✅ 必需 | ❌ 可摘除 | 无 |
| `kira` | `ui/inspector/audio.rs:20,426,465` | ✅ 必需 | ❌ 可摘除 | 无（`kairos_game.rs` 里只是注释提到 kira） |
| `rustfft` | `ui/inspector/audio.rs:79`（非测试） | ✅ 必需 | ❌ 可摘除 | 无 |
| `half` | 无 | ❌ | ✅ 保留 | `benches/texture_encode_decode.rs:7` `use half::f16`（bench 用） |
| `glam` | 无 | ❌ | ❌ | 无 |
| `egui_commonmark` | `ui/inspector/document.rs:4` `CommonMarkCache, CommonMarkViewer` | ✅ 必需 | ❌ 可摘除 | 无 |
| `syntect` | `syntax.rs:210,211,213,222,225,257,259,309,313,326` | ✅ 必需 | ✅ 保留 | `math/color/converts.rs:26` `impl From<Color32> for syntect::highlighting::Color` |
| `tokio-tungstenite` | 无 | ❌ | ❌ | 无（optional，未被任何 feature 启用） |
| `futures-util` | 无 | ❌ | ❌ | 无（optional，未被任何 feature 启用） |
| `serde_json` | 无（`ui/docking_tab/window_state.rs:65` 仅注释） | ❌ | ❌ | 无 |
| `indexmap` | 无 | ❌ | ❌ | 无 |
| `notify-debouncer-full` | `project_watcher.rs:23` | ✅ 必需 | ❌ 可摘除 | 无 |
| `criterion` | 无 | ❌ | ✅ 保留（dev） | `benches/texture_encode_decode.rs:6` |
| `tempfile` | 测试专用：`editor_assets/toml.rs:109`、`text.rs:111`、`syntax.rs:407`、`texture/edit.rs:175`、`texture/mod.rs:825`、`material/test.rs:13`、`project_path_tree/test.rs:1`、`project_watcher/test.rs:30,52`（均位于 `#[cfg(test)] mod test` 之后） | ✅ dev-dep | ✅ 保留（dev） | `asset_pipeline/test.rs:22,23`；`kairos_ui/font.rs:107` |
| `objc2`（macOS） | `runtime.rs:131` | ✅ target-dep | ❌ 可摘除 | 无（engine 非编辑器 `\bobjc2\b` 0 命中） |
| `objc2-app-kit`（macOS） | `runtime.rs:133` | ✅ target-dep | ❌ 可摘除 | 无 |
| `objc2-foundation`（macOS） | `runtime.rs:134` | ✅ target-dep | ❌ 可摘除 | 无 |

**B. `kairos_*` path 依赖**

| path crate | 编辑器直接引用（证据） | 新 crate 依赖? | engine 保留? | engine 侧引用点 |
|---|---|---|---|---|
| `kairos_engine` | `use kairos_engine` 仅 `main.rs:1`；其余经 `crate::{graphics,asset,audio,math,log,time,kairos_ui,…}`（`crate::graphics` 38 处、`crate::asset` 38 处、`crate::math` 6 处、`crate::audio` 5 处、`crate::log` 3 处、`crate::time` 2 处、`crate::kairos_ui` 2 处） | ✅ 必需（核心） | — | — |
| `kairos_ecs` | `camera.rs:17`；`syntax.rs:10,11,369,370`；`ui/inspector/audio.rs:5`；`editor_assets/text.rs:13,73,74`；约 75 处 | ✅ 必需 | ✅ 保留 | `asset_pipeline.rs:33`；`kairos_ui/font.rs:10,69,70`；`kairos_game.rs:24` |
| `kairos_collections` | `ui.rs:2` `use kairos_collections::TypeIdMap` | ✅ 必需 | ❌ **可摘除**（全 engine src 仅 `ui.rs:2` 一处） | 无 |
| `kairos_tasks` | `editor_assets/text.rs:14`、`toml.rs:13`、`syntax.rs:12` | ✅ 必需 | ✅ 保留 | `kairos_ui/font.rs:11` |
| `kairos_transform` | `camera.rs:24`（非测试）、`camera/test.rs:23` | ✅ 必需 | ✅ 保留 | `kairos_game.rs:29` |
| `kairos_graphics` | `ui/inspector/material/test.rs:47`（仅测试） | ✅ dev-dep（测试用） | ✅ 保留 | `lib.rs:42` 再导出；`asset_pipeline/test.rs:95,112` |
| `kairos_asset` | 无直接（经 `crate::asset`） | ❌（经 engine 再导出） | ✅ 保留 | `lib.rs:18` 再导出；`asset_pipeline.rs:31,32` |
| `kairos_audio` | 无直接（经 `crate::audio`） | ❌（经 engine 再导出） | ✅ 保留 | `lib.rs:23` 再导出 |
| `kairos_physics` | 无直接 | ❌（经 engine 再导出） | ✅ 保留 | `lib.rs:37` 再导出 |
| `kairos_time` | 无直接（经 `crate::time`） | ❌（经 engine 再导出） | ✅ 保留 | `lib.rs:46` 再导出；`schedule.rs:42` `crate::time` |
| `kairos_math` | 无直接（经 `crate::math`） | ❌ | ✅ 保留 | `math.rs:3` `pub use kairos_math::*`；`spatial.rs:5` |

**C. bench / tests**

- `benches/` 只有 `texture_encode_decode.rs`，用 `criterion`（第 6 行）+ `half::f16`（第 7 行）+ `kairos_engine::graphics::texture::format`（第 8、9 行），**无任何编辑器代码**。
- `tests/` 目录**不存在**（`find KairosEngine/kairos_engine/tests` 退出码 2）。
- `[[bench]] name="texture_encode_decode"` 属引擎侧，`criterion` 与 bench 都不随编辑器搬走。

---

### 2.1 `kairos_editor` 的 `[dependencies]` 建议

> 前提：workspace 根 `KairosEngine/Cargo.toml` **没有 `[workspace.dependencies]`**（只有 `[workspace.package]`，见其 22-25 行）。因此 `workspace = true` 只能用于 `version`/`edition`/`license` 等 package 字段，**依赖项必须写具体版本/路径**（除非先去加 `[workspace.dependencies]`）。新包还要加进 `members`（否则不参与构建）。

```toml
[package]
name = "kairos_editor"
version.workspace = true          # 继承 [workspace.package]
edition.workspace = true
license.workspace = true
default-run = "kairos_editor"     # bin 名待定

[features]                        # 见第 3 章
default = ["debug", "trace"]
track_location = ["kairos_engine/track_location"]
debug          = ["kairos_engine/debug"]
trace          = ["kairos_engine/trace"]
kairos_reflect = ["kairos_engine/kairos_reflect"]
hotpatching    = ["kairos_engine/hotpatching"]
debug_stepping = ["kairos_engine/debug_stepping"]

[dependencies]
# —— 引擎本体与再导出（Engine / graphics / asset / audio / physics / time / math / log / kairos_ui）——
kairos_engine     = { path = "../kairos_engine" }   # crate::graphics 38 / crate::asset 38 / crate::math 6 / crate::audio 5 / crate::log 3 / crate::time 2 / crate::kairos_ui 2
# —— 编辑器直接使用的 path crate ——
kairos_ecs        = { path = "../kairos_ecs" }        # camera.rs:17, syntax.rs:10, audio.rs:5 …（~75 处）
kairos_collections= { path = "../kairos_collections" }# ui.rs:2 TypeIdMap
kairos_tasks      = { path = "../kairos_tasks" }      # text.rs:14 / toml.rs:13 / syntax.rs:12
kairos_transform  = { path = "../kairos_transform" }  # camera.rs:24 LocalTransform（非测试）

# —— 外部 crate：逐项对应上文 A 表 ——
toml = "1.1.2"                                        # project_window.rs:34 / editor_assets.rs:5
async-fs = "2.1"                                      # texture/edit.rs:52,130 / syntax.rs:312
crossbeam-channel = "0.5.15"                          # project_watcher.rs:22,54
tokio = { version = "1.52.3", features = ["full"] }   # oneshot + main.rs #[tokio::main]
petgraph = "0.8.3"                                    # project_path_tree / ui.rs 变体
winit = { version = "0.30", features = ["android-native-activity"] }  # runtime.rs:9；与 engine 保持一致
env_logger = "0.10"                                   # main.rs:9（bin）
log = "0.4"                                           # log::warn! 等
open = "5.4.0"                                        # ide_detection.rs:213,254
native-dialog = "0.9.6"                               # ui/native_dialog.rs:3 + dialog.rs
serde = "1.0.228"                                     # 大量 derive
strum = { version = "0.26", features = ["derive"] }   # asset_registry.rs:8 / mesh.rs:4
duplicate = "2.0"                                     # docking_tab.rs:42
paste = "1.0"                                         # docking_tab.rs:43
image = { version = "0.25.10", features = ["rayon", "serde", "png"] }  # runtime.rs:37 / texture
wgpu = { version = "29.0.3", features = ["serde"] }   # runtime.rs:362-373（须与 egui-wgpu 0.35 匹配）
pollster = "0.3"                                      # runtime.rs:190
egui = "0.35.0"
egui_extras = { version = "0.35.0", features = ["all_loaders", "syntect"] }  # runtime.rs:104 / syntax.rs:341；datepicker 未见使用
emath = "0.35.0"                                      # docking_tab.rs:15 / drag_and_drop.rs:6（**显式**）
epaint = "0.35.0"                                     # docking_tab.rs:16（**显式**）
egui-winit = "0.35.0"                                 # runtime.rs:93,180,292
egui-wgpu = "0.35.0"                                  # runtime.rs:311
parking_lot = "0.12.5"                                # 大量 Mutex
uuid = { version = "1", features = ["v4", "serde"] }  # asset_registry.rs:9 / project_path_tree.rs:626
kira = { version = "0.12.0", features = ["serde"] }   # audio.rs:20,426,465
rustfft = "6"                                         # audio.rs:79
egui_commonmark = "0.24.0"                            # document.rs:4
syntect = { version = "5.3", default-features = false, features = ["default-fancy"] }  # syntax.rs:210+
notify-debouncer-full = { version = "0.7.0", default-features = false }  # project_watcher.rs:23

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6.4"                                       # runtime.rs:131
objc2-app-kit = { version = "0.3.2", default-features = false, features = ["std", "NSApplication", "NSImage"] }
objc2-foundation = { version = "0.3.2", default-features = false, features = ["std", "NSString"] }

[dev-dependencies]
tempfile = "3"                                        # 各 `#[cfg(test)] mod test`
kairos_graphics = { path = "../kairos_graphics" }     # material/test.rs:47（仅测试）
```

理由要点：

- **不引入** `rand` / `smallvec` / `rkyv` / `half` / `bytemuck` / `glam` / `anyhow` / `base64` / `gltf` / `sonic-rs` / `serde_json` / `indexmap` / `rayon` / `futures-util` / `tokio-tungstenite` / `criterion`：编辑器文件里对这些名字的命中要么为 0，要么只出现在注释（`serde_json`、`half`、`kairos_asset` 各一处注释）。
- **显式列 `emath`/`epaint`**：编辑器代码直接写 `emath::`/`epaint::`，虽然 `egui` 会再导出它们，但**直接路径引用必须显式声明依赖**。
- **`kairos_asset` / `kairos_audio` / `kairos_physics` / `kairos_time` / `kairos_math` 不进新 crate**：编辑器全部经 `crate::asset` 等再导出使用，搬出后改为 `kairos_engine::asset::…` 即可；`kairos_engine` 保留再导出是这层的契约。
- **`kairos_graphics` 放 dev-dep**：唯一引用在 `ui/inspector/material/test.rs:47`，而 `material.rs:49` 是 `#[cfg(test)]`，属测试代码，可用 dev-dependency。
- **`tempfile` 放 dev-dep**：上表 A 中列出的编辑器命中点全部位于 `#[cfg(test)] mod test` 之内。
- **`env_logger` 虽只被 bin 用**，bin 与 lib 同属一个 package，仍写进 `[dependencies]`。
- `wgpu`/`egui-wgpu` 版本必须成对（29 / 0.35），否则 `ScreenDescriptor` 类型不匹配。

### 2.2 `kairos_engine` 可摘除的依赖

以下每项都对「engine 非编辑器代码」做过全量检索，命中数为 0：

| 依赖 | 检索口径 | 结果 |
|---|---|---|
| `toml` | `grep -rnE "\btoml::" src --exclude-dir=kairos_editor --exclude=…` | 0 |
| `log` | `grep -rnE "\blog::" …` | 0（`log.rs` 自实现，仅 `eprintln!`） |
| `tokio` | `\btokio::` | 0 |
| `async-fs` | `\basync_fs::` | 0 |
| `petgraph` `strum` `parking_lot` `crossbeam-channel` `notify-debouncer-full` `native-dialog` `open` `uuid` `kira` `rustfft` `image` `wgpu` `egui-extras` `egui-winit` `egui-wgpu` `emath` `epaint` `egui-commonmark` `duplicate` `paste` `anyhow` `base64` `gltf` `sonic-rs` `serde_json` `indexmap` `futures-util` `tokio-tungstenite` `bytemuck` `rayon` `glam` | 逐个 `\b<name>\b`（覆盖 derive/macro 形态） | 全部 0 |
| `objc2` / `objc2-app-kit` / `objc2-foundation` | `\bobjc2\b` | 0（`[target.'cfg(target_os="macos")'.dependencies]` 整段可删） |
| `kairos_collections` | `\bkairos_collections\b`（全 src） | 仅编辑器 `ui.rs:2` 一处，engine 可摘 |

> 提示：`anyhow`/`base64`/`gltf`/`sonic-rs`/`serde_json`/`indexmap`/`futures-util`/`tokio-tungstenite`/`bytemuck`/`rayon`/`glam` 在**两侧都无直接引用**，属于可整体删除的遗留项（`futures-util`/`tokio-tungstenite` 还是 optional 且无 feature 启用）。删除前建议编译确认。

### 2.3 `kairos_engine` 必须保留的依赖

| 依赖 | engine 侧非编辑器引用点（file:line） |
|---|---|
| `winit` | `inputs.rs:3,102,105,111,130,132`；`kairos_game.rs:173,177,181,185` |
| `egui` | `math/color/converts.rs:3,5,9,10`（`impl From<Color32> for egui::Color32`） |
| `syntect` | `math/color/converts.rs:26`（`impl From<Color32> for syntect::highlighting::Color`） |
| `pollster` | `asset_pipeline.rs:95` |
| `serde` | `kairos_settings.rs:5`；`math/color/serialies.rs:1,8,17,19` |
| `env_logger` | `bin/bake_assets.rs:14,15` |
| `smallvec` | `kairos_game.rs:390` |
| `rand` | `kairos_game.rs:392` |
| `rkyv` | `asset_pipeline/test.rs:95`（测试；可降 dev-dep） |
| `half` | `benches/texture_encode_decode.rs:7`（bench；可降 dev-dep） |
| `criterion`（dev） | `benches/texture_encode_decode.rs:6` |
| `tempfile`（dev） | `asset_pipeline/test.rs:22,23`；`kairos_ui/font.rs:107` |
| `kairos_ecs` | `asset_pipeline.rs:33`；`kairos_ui/font.rs:10,69,70`；`kairos_game.rs:24` |
| `kairos_asset` | `lib.rs:18` 再导出；`asset_pipeline.rs:31,32` |
| `kairos_graphics` | `lib.rs:42` 再导出；`asset_pipeline/test.rs:95,112` |
| `kairos_math` | `math.rs:3`；`spatial.rs:5` |
| `kairos_audio` | `lib.rs:23` 再导出 |
| `kairos_physics` | `lib.rs:37` 再导出 |
| `kairos_time` | `lib.rs:46` 再导出；`schedule.rs:42` |
| `kairos_tasks` | `kairos_ui/font.rs:11` |
| `kairos_transform` | `kairos_game.rs:29` |

### 2.4 易错点

1. **`schedule` 的测试依赖被搬走的编辑器资产**（高风险，会直接编译失败）：留在引擎的 `kairos_editor/schedule/asset_test.rs` 引用 `crate::kairos_editor::editor_assets::{Text, Toml}`（:17）、`crate::kairos_editor::syntax::SyntaxHighlightSettings`（:18）、`crate::kairos_editor::build_world`（:26）；`schedule.rs:360` 又 `mod asset_test;`。搬走后这些路径全部失效——需要把该测试一并搬走，或让引擎侧保持对编辑器资产的可见性。
2. **`build_world` 与 `Engine` 的归属自相矛盾**：`kairos_editor.rs:105-107,127` 的 `build_world` 调用编辑器资产 `install`，而设计要求 `Engine` 留在引擎、文件又整个搬走。实际需要把「引擎启动（Engine/build_world，去除编辑器资产 install）」与「编辑器资产注册」切开，这属于设计决策，不是纯依赖搬迁。
3. **`crate::` 前缀整体要改**：编辑器代码里 `crate::graphics`(38)/`crate::asset`(38)/`crate::math`(6)/`crate::audio`(5)/`crate::log`(3)/`crate::time`(2)/`crate::kairos_ui`(2) 都要改成 `kairos_engine::…`；`crate::kairos_editor::…` 自引用（如 `inspector/audio.rs:14`）改为 `crate::…`。这依赖 `kairos_engine` 的模块保持 `pub`（`log`、`math`、`inputs`、`kairos_game`、`kairos_ui` 均为 `pub mod`）。
4. **`kairos_dialog` → `dialog`**：编辑器内有 16 处 `crate::kairos_dialog::error_message_window`（`about_window.rs:3,146,156`、`tool_bar.rs:22,318,325`、`scene_window.rs:19,340,350`、`project_path_tree.rs:17,385,398,415,433,441`、`ui.rs:44,505`、`runtime.rs:29,467`），`main.rs:2,18` 也有引用。新 crate 里要统一改为 `crate::dialog::…`。
5. **两个 `log` 不要混**：`log::warn!` 是外部 crate `log`（编辑器必需，引擎 0 引用可摘）；`crate::log::Log` 是引擎自带的日志模块（保留在引擎，编辑器经 `kairos_engine::log::Log` 用）。摘引擎的 `log` 依赖时别误删 `log.rs`。
6. **`workspace = true` 的适用范围**：workspace 根没有 `[workspace.dependencies]`，依赖项写 `workspace = true` 会报错；只能对 `version`/`edition`/`license` 用。
7. **别忘了注册 workspace 成员**：`KairosEngine/Cargo.toml` 的 `members` 需加 `"kairos_editor"`。
8. **同名依赖版本必须一致**：`winit`、`egui*`、`syntect`、`pollster`、`serde`、`image` 会同时被两个 crate 依赖，版本/feature 不一致会导致类型不互通或重复编译。
9. **feature 传递的隐藏耦合**：`winit` 的 `android-native-activity`、`egui_extras` 的 `all_loaders`/`syntect`、`image` 的 `png`、`notify-debouncer-full` 的 `default-features = false`、`syntect` 的 `default-fancy` 都必须照搬，否则运行时行为变化（例如图片/主题加载失败）。

## 3. features

结论先行：编辑器代码**没有任何** `#[cfg(feature = "…")]`、`cfg!(feature = …)`，也不引用 `track_location`/`kairos_reflect`/`hotpatching`/`debug_stepping` 这些名字（在 `src/kairos_editor/**` + `kairos_editor.rs` 上 grep 全部 0 命中；引擎其余代码同样 0）。引擎的 features 是**纯透传**给 `kairos_ecs`，本 crate 自身不据此编译任何分支。

### 3.1 features 一览

| feature | 编辑器是否引用 | 证据 | 新 crate 是否需同名透传 | 备注 |
|---|---|---|---|---|
| `default = ["debug","trace"]` | —（定义本身） | `Cargo.toml:14` | ✅ 建议同上 | 见下「default 怎么写」 |
| `track_location` | 否 | `\btrack_location\b` 在编辑器 0 命中 | ✅ 建议透传 `kairos_engine/track_location` | 决定 ECS 的 `#[track_location]`；编辑器未直接写该 attribute |
| `debug` | 否 | 无 `#[cfg(feature="debug")]`/`cfg!` 命中 | ✅ 建议透传 | ECS 的 debug 行为 |
| `trace` | 否 | 无任何 `trace_span`/`cfg!` 命中 | ✅ 建议透传 | 引擎的 `trace` 会开启 `kairos_ecs` 的 `dep:tracing`（`kairos_ecs/Cargo.toml`） |
| `kairos_reflect` | 否 | `\bkairos_reflect\b` 0 命中 | ✅ 建议透传 | |
| `hotpatching` | 否 | `\bhotpatching\b` 0 命中 | ✅ 建议透传 | |
| `debug_stepping` | 否 | `\bdebug_stepping\b` 0 命中 | ✅ 建议透传 | |

`default` 怎么写：

- 新 crate 建议 `default = ["debug", "trace"]`，与 `kairos_engine` 对齐，保证「单独构建/单独发版 `kairos_editor`」时行为与在 workspace 里一致。
- 若要默认特性可被真正关闭，需要把对引擎的依赖写成 `kairos_engine = { path = "../kairos_engine", default-features = false }` 并逐项透传。因为引擎 src 内**没有任何 feature 门控**，关掉引擎默认只会影响 `kairos_ecs` 的 `debug`/`trace`，不会破坏引擎编译。
- 反之，若保留 `kairos_engine` 默认特性（不写 `default-features = false`），即使新 crate 写 `--no-default-features`，引擎的 `debug`+`trace` 仍会被打开——想精确控制就必须显式关掉。

`kairos_engine` 保留这些 feature 是否必要：

- **必要**。引擎 src 已无任何 `cfg(feature)`，这些 feature 的全部意义就是作为**对外选择 `kairos_ecs` 编译配置的唯一接缝**；一旦新 crate 采用 `kairos_engine/<name>` 透传，引擎就必须继续声明它们，否则新 crate 的 feature 会指向不存在的 feature（`cargo` 报 feature 不存在）。
- 也可以让新 crate 直接透传到 `kairos_ecs/<name>`（`kairos_editor` 已直接依赖 `kairos_ecs`），这样即使引擎删掉这些 feature 也能工作；但会形成两条并行的 feature 接缝，且若引擎与编辑器同时被构建，`cargo` 的 feature 统一仍会合并到同一份 `kairos_ecs`。**推荐仍走 `kairos_engine/<name>`，保持单一接缝**。
- 另一个可选动作：把 `default` 从 `kairos_engine` 移除，改由叶子 crate 决定；但这会改变现有下游默认行为，不建议在本次抽取中顺手做。

需编译确认项：`track_location` 是否应默认开启（当前引擎默认不含它，编辑器也未见 `#[track_location]` 用法，故保持默认关闭）；`kira` 的 `serde`、`image` 的 `rayon` 是否为编辑器实际所需（此处按「与引擎完全一致」建议，属保守选择）。

---

## 4. doctest / 文档注释清单

### 4.1 会因搬迁而**编译失败**的 doctest（`use kairos_engine::kairos_editor::…`）

共 **13 处**，全部在 `ui/docking_tab/**`。搬迁后新 crate 名就是 `kairos_editor`，故只改 crate 名即可。

| # | file:line | 所在代码块（fence 行号） | 现文本 | 新写法 |
|---|---|---|---|---|
| 1 | `ui/docking_tab/dock_state/tree/node.rs:191` | 190–204 | `use kairos_engine::kairos_editor::ui::docking_tab::dock_state::DockState;` | `use kairos_editor::ui::docking_tab::dock_state::DockState;` |
| 2 | `ui/docking_tab/dock_state/tree/node.rs:246` | 245–252 | 同上 | 同上 |
| 3 | `ui/docking_tab/dock_state/tree/node.rs:171` | **169–174（残缺块）** | 同上 | 同上（见下方注） |
| 4 | `ui/docking_tab/dock_state/tree.rs:330` | 329–339 | `use kairos_engine::kairos_editor::ui::docking_tab::{dock_state::DockState, dock_state::tree::NodeIndex};` | `use kairos_editor::ui::docking_tab::{dock_state::DockState, dock_state::tree::NodeIndex};` |
| 5 | `ui/docking_tab/dock_state/tree.rs:357` | 356–362 | `…dock_state::DockState;` | `use kairos_editor::…dock_state::DockState;` |
| 6 | `ui/docking_tab/dock_state/tree.rs:373` | 372–380 | 同上 | 同上 |
| 7 | `ui/docking_tab/dock_state/tree.rs:402` | 401–415 | `…{dock_state::DockState, dock_state::tree::{NodeIndex, Split}, surfaces::SurfaceIndex};` | 同结构，crate 名换 `kairos_editor` |
| 8 | `ui/docking_tab/dock_state/tree.rs:550` | 549–563 | `…{dock_state::DockState, dock_state::tree::{NodeIndex, Split, node::Node}, surfaces::SurfaceIndex};` | 同上 |
| 9 | `ui/docking_tab/dock_state.rs:125` | 124–133 | `use kairos_engine::kairos_editor::ui::docking_tab::dock_state::DockState;` | `use kairos_editor::ui::docking_tab::dock_state::DockState;` |
| 10 | `ui/docking_tab/dock_state.rs:532` | 531–538（隐藏行 `# use`） | 同上 | 同上 |
| 11 | `ui/docking_tab/dock_state.rs:565` | 564–571 | 同上 | 同上 |
| 12 | `ui/docking_tab/dock_state.rs:583` | 582–589（隐藏行） | 同上 | 同上 |
| 13 | `ui/docking_tab/dock_state.rs:602` | 601–608（隐藏行） | 同上 | 同上 |

> **第 3 条的残缺块**：`node.rs:169` 是 `/// ```rust`，`:170` 却是字面 `/// ```rust`（多写了一层），`:174` 才真正闭合。于是该 doctest 的**块体只是一行注释**，既不含那条 `use`、也不会因为路径失效而失败。这是**已存在的文档缺陷**，建议顺手修但不是搬迁的必需项（见 §9）。

### 4.2 intra-doc link 里的 `crate::…`

这些是 **rustdoc warning**（非 error，仓库无 `deny`）。分两类：

**(a) 今天有效、搬迁后必须加 `kairos_engine::` 前缀**

| file:line | 现文本 | 新写法 |
|---|---|---|
| `camera.rs:7` | `[`Camera`](crate::graphics::camera::Camera)` | `kairos_engine::graphics::camera::Camera` |
| `editor_assets/text.rs:23` | `[`Assets<Text>`](crate::asset::Assets)` | `kairos_engine::asset::Assets` |
| `editor_assets/text.rs:61` | `[`crate::asset::install`]` | `kairos_engine::asset::install` |
| `editor_assets/toml.rs:22` | `[`Assets<Toml>`](crate::asset::Assets)` | `kairos_engine::asset::Assets` |
| `editor_assets/toml.rs:59` | `[`crate::asset::install`]` | `kairos_engine::asset::install` |
| `syntax.rs:275` | `[`Assets<SyntaxHighlightSettings>`](crate::asset::Assets)` | `kairos_engine::asset::Assets` |
| `syntax.rs:347` | `[`Toml`](crate::kairos_editor::editor_assets::Toml)` | `crate::editor_assets::Toml` |
| `syntax.rs:357` | `[`crate::asset::install`]` | `kairos_engine::asset::install` |
| `ui/inspector/texture/edit.rs:11` | `[`AssetServer::add_async`](crate::asset::AssetServer::add_async)` | `kairos_engine::asset::AssetServer::add_async` |
| `ui/inspector/texture/edit.rs:46` | 同上 | 同上 |
| `ui/inspector/texture/mod.rs:748` | `[`crate::asset::install`]` | `kairos_engine::asset::install` |

**(b) 今天（推断）已断裂，搬迁后依旧断裂 —— upstream `egui_dock` fork 的遗留**

| file:line | 现文本 | 说明 |
|---|---|---|
| `ui/docking_tab/surfaces.rs:31` | `[`DockState`](crate::DockState)` | `kairos_engine` 根部并无 `DockState` 再导出；搬迁后应为 `crate::ui::docking_tab::dock_state::DockState` |
| `ui/docking_tab/dock_state/tree/node.rs:11` | `[`Tree`](crate::Tree)` | 同上（`crate::ui::docking_tab::dock_state::tree::Tree`） |
| `ui/docking_tab/tab_drawer.rs:112` | `[`DockArea::show_add_popup`](crate::DockArea::show_add_popup)` | 同上（`crate::ui::docking_tab::DockArea`） |
| `ui/docking_tab/dock_state.rs:100` | ``[`DockArea`](create::kairos_engine::kairos_editor::ui::docking_tab)`` | **拼写错误**：`create::` 应为 `crate::`（即使改对也指向已不存在的路径） |

> (b) 类判为「今天已断裂」是**读码推断**（`kairos_engine/src/lib.rs` 无对应 `pub use`），未跑 `cargo doc` 验证。

### 4.3 lint 配置与后果

- `kairos_engine/Cargo.toml:22-23` 只有 `[lints.rust] unexpected_cfgs = { level = "warn", … }`；workspace 根 `Cargo.toml` **没有** `[workspace.lints]`。
- `kairos_engine/src/lib.rs`、`main.rs`、`kairos_editor.rs` 均无 `#![deny(…)]` / `#![forbid(…)]`。
- **后果**：4.2 的断裂是 **warning**，不会挡住编译；4.1 的 doctest 是 **编译失败（error）**。
- **但要小心验证口径**：`.cargo/config.toml` 的 `test-crate` 别名是 `test --lib --bins --tests --no-fail-fast -p` —— **不含 doctest**。只有 `cargo test-full`（= `test --workspace --no-fail-fast`）才跑 doctest。因此 §4.1 的 13 处若不改，`cargo test-crate kairos_editor` 可能全绿而 `cargo test-full` 红。**这是给验收票（[#254](https://github.com/WhitePetal/KairosEngine/issues/254)）的关键输入。**

---

## 5. `Library/asset_registry.toml` 条目

**口径**：基线 HEAD；工作区里该文件有 +116 行未提交改动，用 `git show HEAD:KairosEngine/Library/asset_registry.toml` 分析。

### 5.1 形态事实

- 指向编辑器的条目共 **94 条**：**18 条目录条目 + 76 条 `.rs` 文件条目**（含 `kairos_editor.rs` 自身与 `kairos_dialog.rs`）。
- 条目结构只有 `[[entries]]` + `guid` + `path`，**没有 kind 字段**；资产类型由扩展名推断 —— `AssetKind::from_extension` 把 `.rs` 映射为 `Script`（`kairos_editor/asset_registry.rs:42,81`）。故这 76 个文件条目全是 `Script` 资产。
- **无 `.meta`**：`find kairos_engine/src/kairos_editor* -name '*.meta'` → **0 个**。编辑器 `.rs` 是纯源码资产，不带 `.meta`。
- **不涉及 `imported_assets/`**：`imported_assets` 下只有 `kairos_asset/imported_assets*` 两条，与编辑器无关。
- `kairos_engine/src/main.rs` **没有**注册表条目。
- **3 条已是陈旧条目**（文件在磁盘上不存在，是 audio 抽取 [#178](https://github.com/WhitePetal/KairosEngine/issues/178)/[#179](https://github.com/WhitePetal/KairosEngine/issues/179) 的残留）：
  - `kairos_engine/src/kairos_editor/serialize_asset`
  - `kairos_engine/src/kairos_editor/serialize_asset/audio.rs`
  - `kairos_engine/src/kairos_editor/serialize_asset.rs`

### 5.2 未提交改动的影响

工作区新增的 116 行条目（`docs/adr/0007-…`、`docs/research/render-cache-defect-inventory.md`、`kairos_audio/**` 整棵树）中，**没有一条** `kairos_editor`。因此 HEAD 与工作区在「编辑器条目」上集合相同，只是行号整体后移（例如 `kairos_dialog.rs` 从 `:2251` 后移到 `:2355`）。

### 5.3 完整条目表（GUID 不变，只改 path）

| # | GUID | HEAD 路径 | 新路径 | 磁盘存在 |
|---|---|---|---|---|
| 1 | `bba240e2-8328-4a42-8547-b118ec2abf45` | `kairos_engine/src/kairos_dialog.rs` | `kairos_editor/src/dialog.rs` | 是 |
| 2 | `80526d85-3303-413c-a4ec-e375d0a2e1aa` | `kairos_engine/src/kairos_editor` | `kairos_editor/src` | 是 |
| 3 | `9cee2180-a3d0-407c-88f1-1bd5c3caafd0` | `kairos_engine/src/kairos_editor/asset_registry.rs` | `kairos_editor/src/asset_registry.rs` | 是 |
| 4 | `0b50e6ca-80fa-4ee7-ac4f-84aa1de61a49` | `kairos_engine/src/kairos_editor/camera` | `kairos_editor/src/camera` | 是 |
| 5 | `e53c3fc3-d709-4bf9-94ae-7db2b9f4f2a1` | `kairos_engine/src/kairos_editor/camera/test.rs` | `kairos_editor/src/camera/test.rs` | 是 |
| 6 | `d43ebab6-4321-4e3b-a570-228a3ddf566c` | `kairos_engine/src/kairos_editor/camera.rs` | `kairos_editor/src/camera.rs` | 是 |
| 7 | `860bbaba-ccb5-4233-a78a-1c815f1c46b4` | `kairos_engine/src/kairos_editor/consts.rs` | `kairos_editor/src/consts.rs` | 是 |
| 8 | `263c7a14-2e0b-48af-b225-a93d3cfded92` | `kairos_engine/src/kairos_editor/editor_assets` | `kairos_editor/src/editor_assets` | 是 |
| 9 | `784e23a9-5dd2-4b5b-b265-9b449b7fa894` | `kairos_engine/src/kairos_editor/editor_assets/text.rs` | `kairos_editor/src/editor_assets/text.rs` | 是 |
| 10 | `3586cbb1-7d5a-4a9c-84e4-619de023b7be` | `kairos_engine/src/kairos_editor/editor_assets/toml.rs` | `kairos_editor/src/editor_assets/toml.rs` | 是 |
| 11 | `0bd21288-a5e5-4e2e-9a2d-e41a147d2f1f` | `kairos_engine/src/kairos_editor/editor_assets.rs` | `kairos_editor/src/editor_assets.rs` | 是 |
| 12 | `3e37c81f-6f5e-4113-9e64-559c85d85361` | `kairos_engine/src/kairos_editor/project_path_tree` | `kairos_editor/src/project_path_tree` | 是 |
| 13 | `6d710b01-b3fd-4f16-847d-42367b18c507` | `kairos_engine/src/kairos_editor/project_path_tree/create_request.rs` | `kairos_editor/src/project_path_tree/create_request.rs` | 是 |
| 14 | `2bcc5429-1f9d-4b78-9aea-000a56c39bd7` | `kairos_engine/src/kairos_editor/project_path_tree/test.rs` | `kairos_editor/src/project_path_tree/test.rs` | 是 |
| 15 | `3cbb2388-1fca-4f25-9d39-5b7823938a73` | `kairos_engine/src/kairos_editor/project_path_tree/texture_path.rs` | `kairos_editor/src/project_path_tree/texture_path.rs` | 是 |
| 16 | `e1a06f32-618c-49a9-8df9-5377dc3c722b` | `kairos_engine/src/kairos_editor/project_path_tree/tree_node.rs` | `kairos_editor/src/project_path_tree/tree_node.rs` | 是 |
| 17 | `827c8276-4662-4108-8561-a71d571590d2` | `kairos_engine/src/kairos_editor/project_path_tree.rs` | `kairos_editor/src/project_path_tree.rs` | 是 |
| 18 | `5780db05-e773-453d-a063-6a8c80fc313b` | `kairos_engine/src/kairos_editor/runtime.rs` | `kairos_editor/src/runtime.rs` | 是 |
| 19 | `6b1cc6a7-0280-4960-ad79-735b6d3a7256` | `kairos_engine/src/kairos_editor/schedule` | `kairos_editor/src/schedule` | 是 |
| 20 | `c2a3cc6e-77ec-4afe-8ef3-83cd86ecb9de` | `kairos_engine/src/kairos_editor/schedule/asset_test.rs` | `kairos_editor/src/schedule/asset_test.rs` | 是 |
| 21 | `2776ec4d-f4bd-4e4b-b7b3-b7d4afaab859` | `kairos_engine/src/kairos_editor/schedule/test.rs` | `kairos_editor/src/schedule/test.rs` | 是 |
| 22 | `4674d97b-3c1e-4591-a281-f730555525c5` | `kairos_engine/src/kairos_editor/schedule.rs` | `kairos_editor/src/schedule.rs` | 是 |
| 23 | `c5ddeb92-9f81-4e23-84cc-8ded89730a88` | `kairos_engine/src/kairos_editor/serialize_asset` | `kairos_editor/src/serialize_asset` | **否（陈旧）** |
| 24 | `f0442261-408e-4dc3-8cf7-1b143d62af2f` | `kairos_engine/src/kairos_editor/serialize_asset/audio.rs` | `kairos_editor/src/serialize_asset/audio.rs` | **否（陈旧）** |
| 25 | `5826719b-d832-4aaa-9fe1-3c4ada959eed` | `kairos_engine/src/kairos_editor/serialize_asset.rs` | `kairos_editor/src/serialize_asset.rs` | **否（陈旧）** |
| 26 | `01a07f31-5990-41b7-824a-228d3efcd588` | `kairos_engine/src/kairos_editor/syntax.rs` | `kairos_editor/src/syntax.rs` | 是 |
| 27 | `60d80d6d-bab9-4e13-9106-1b7fcd4cb3df` | `kairos_engine/src/kairos_editor/ui` | `kairos_editor/src/ui` | 是 |
| 28 | `3a562dbf-21a2-42e4-95be-23957e1fb0d4` | `kairos_engine/src/kairos_editor/ui/about_window.rs` | `kairos_editor/src/ui/about_window.rs` | 是 |
| 29 | `3e1b7cfd-97ff-4ebc-be50-c4cca907d780` | `kairos_engine/src/kairos_editor/ui/console_window.rs` | `kairos_editor/src/ui/console_window.rs` | 是 |
| 30 | `503658de-6820-4eb0-aa00-96e4668ebd7f` | `kairos_engine/src/kairos_editor/ui/dialog.rs` | `kairos_editor/src/ui/dialog.rs` | 是 |
| 31 | `cacc2747-de22-492b-9b64-2256e0e28a26` | `kairos_engine/src/kairos_editor/ui/docking_tab` | `kairos_editor/src/ui/docking_tab` | 是 |
| 32 | `df20e4ac-0612-493c-b3a5-7ba3745195e6` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state` | `kairos_editor/src/ui/docking_tab/dock_state` | 是 |
| 33 | `560e3a61-7784-4ac0-a163-db6f9d9d7ce1` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree` | `kairos_editor/src/ui/docking_tab/dock_state/tree` | 是 |
| 34 | `7db0b2ed-7b32-4df8-83fd-3390a7ec5521` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node` | `kairos_editor/src/ui/docking_tab/dock_state/tree/node` | 是 |
| 35 | `13290246-c93c-4581-abff-034834ea0b9b` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node/leaf_node.rs` | `kairos_editor/src/ui/docking_tab/dock_state/tree/node/leaf_node.rs` | 是 |
| 36 | `d04f9bd0-6054-4d54-91f8-4ba3100c78c7` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node/split_node.rs` | `kairos_editor/src/ui/docking_tab/dock_state/tree/node/split_node.rs` | 是 |
| 37 | `ba7a0fab-f265-489a-b2c5-d37a8b3e0266` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/node.rs` | `kairos_editor/src/ui/docking_tab/dock_state/tree/node.rs` | 是 |
| 38 | `6048fa8c-dbdb-48a2-84fd-36226612c05b` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree/test.rs` | `kairos_editor/src/ui/docking_tab/dock_state/tree/test.rs` | 是 |
| 39 | `f9b38514-16e1-4d10-8491-22996c37ebc4` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state/tree.rs` | `kairos_editor/src/ui/docking_tab/dock_state/tree.rs` | 是 |
| 40 | `082b88d2-e981-47ec-bf1b-75344717142c` | `kairos_engine/src/kairos_editor/ui/docking_tab/dock_state.rs` | `kairos_editor/src/ui/docking_tab/dock_state.rs` | 是 |
| 41 | `1390b31e-83c4-4160-a776-1452e219d0ce` | `kairos_engine/src/kairos_editor/ui/docking_tab/drag_and_drop.rs` | `kairos_editor/src/ui/docking_tab/drag_and_drop.rs` | 是 |
| 42 | `bfbf5847-d298-4127-bd4e-bae7bab00b18` | `kairos_engine/src/kairos_editor/ui/docking_tab/state.rs` | `kairos_editor/src/ui/docking_tab/state.rs` | 是 |
| 43 | `a2dd2d5c-9f6d-48a7-98ca-e39cde3e6019` | `kairos_engine/src/kairos_editor/ui/docking_tab/styles.rs` | `kairos_editor/src/ui/docking_tab/styles.rs` | 是 |
| 44 | `bb13a61f-57d5-4753-a1ff-5beb7ccc6a2f` | `kairos_engine/src/kairos_editor/ui/docking_tab/surfaces.rs` | `kairos_editor/src/ui/docking_tab/surfaces.rs` | 是 |
| 45 | `557828c5-2163-4c49-94b1-863e2a576837` | `kairos_engine/src/kairos_editor/ui/docking_tab/tab_drawer.rs` | `kairos_editor/src/ui/docking_tab/tab_drawer.rs` | 是 |
| 46 | `9f9ad23f-78b6-4257-98ca-ff5fd5711897` | `kairos_engine/src/kairos_editor/ui/docking_tab/translations.rs` | `kairos_editor/src/ui/docking_tab/translations.rs` | 是 |
| 47 | `ac083153-0a9b-4584-9a0c-cb9299365ba4` | `kairos_engine/src/kairos_editor/ui/docking_tab/window_state.rs` | `kairos_editor/src/ui/docking_tab/window_state.rs` | 是 |
| 48 | `173db216-0351-44d3-9812-692cd1ebd22d` | `kairos_engine/src/kairos_editor/ui/docking_tab.rs` | `kairos_editor/src/ui/docking_tab.rs` | 是 |
| 49 | `c940a2bf-6b6d-4851-8591-9bb8f49f559a` | `kairos_engine/src/kairos_editor/ui/drag.rs` | `kairos_editor/src/ui/drag.rs` | 是 |
| 50 | `c4ab48e7-2fce-4ce1-bfa9-31765d05da4b` | `kairos_engine/src/kairos_editor/ui/egui_ext.rs` | `kairos_editor/src/ui/egui_ext.rs` | 是 |
| 51 | `728b850c-4b2b-46fc-87ff-c24bc073101d` | `kairos_engine/src/kairos_editor/ui/game_window.rs` | `kairos_editor/src/ui/game_window.rs` | 是 |
| 52 | `4c19b190-a2e0-4da0-94c7-d45fe7b6f044` | `kairos_engine/src/kairos_editor/ui/global_styles.rs` | `kairos_editor/src/ui/global_styles.rs` | 是 |
| 53 | `f91faa36-dc0c-4042-9003-04e8dd55a2fe` | `kairos_engine/src/kairos_editor/ui/hierarchy_window.rs` | `kairos_editor/src/ui/hierarchy_window.rs` | 是 |
| 54 | `ad6f29a2-33b6-49ae-a9fa-4e493932ef56` | `kairos_engine/src/kairos_editor/ui/ide_detection.rs` | `kairos_editor/src/ui/ide_detection.rs` | 是 |
| 55 | `6c3fbe2d-d337-4d72-9619-3f41becf94c7` | `kairos_engine/src/kairos_editor/ui/inspector` | `kairos_editor/src/ui/inspector` | 是 |
| 56 | `db4b878c-60cb-4fe2-b38b-74cd33828f46` | `kairos_engine/src/kairos_editor/ui/inspector/audio` | `kairos_editor/src/ui/inspector/audio` | 是 |
| 57 | `bdd2bed0-d258-457f-a24b-760e577daaf9` | `kairos_engine/src/kairos_editor/ui/inspector/audio/test.rs` | `kairos_editor/src/ui/inspector/audio/test.rs` | 是 |
| 58 | `b57ecacc-c6e4-4365-8377-274e62ed467a` | `kairos_engine/src/kairos_editor/ui/inspector/audio.rs` | `kairos_editor/src/ui/inspector/audio.rs` | 是 |
| 59 | `1accf02e-8e19-488d-8361-3fd28944db61` | `kairos_engine/src/kairos_editor/ui/inspector/code.rs` | `kairos_editor/src/ui/inspector/code.rs` | 是 |
| 60 | `f69c2adb-8134-456f-86a6-990baf2f0977` | `kairos_engine/src/kairos_editor/ui/inspector/creater.rs` | `kairos_editor/src/ui/inspector/creater.rs` | 是 |
| 61 | `e247098a-94fe-43aa-8e3a-274cfc9a7deb` | `kairos_engine/src/kairos_editor/ui/inspector/directory.rs` | `kairos_editor/src/ui/inspector/directory.rs` | 是 |
| 62 | `2a42d7bf-76f5-4bcc-8e6e-aec73dcb40b8` | `kairos_engine/src/kairos_editor/ui/inspector/document.rs` | `kairos_editor/src/ui/inspector/document.rs` | 是 |
| 63 | `b0acdec2-53b4-4be0-a900-3b998ef147bc` | `kairos_engine/src/kairos_editor/ui/inspector/font.rs` | `kairos_editor/src/ui/inspector/font.rs` | 是 |
| 64 | `0a24f618-aba7-4c45-8b75-f854fc0dbe29` | `kairos_engine/src/kairos_editor/ui/inspector/material` | `kairos_editor/src/ui/inspector/material` | 是 |
| 65 | `09841e1c-624a-4ed5-9e8e-9037d5a53277` | `kairos_engine/src/kairos_editor/ui/inspector/material/test.rs` | `kairos_editor/src/ui/inspector/material/test.rs` | 是 |
| 66 | `709a12da-ef08-4cdd-9009-cda8fec616c3` | `kairos_engine/src/kairos_editor/ui/inspector/material.rs` | `kairos_editor/src/ui/inspector/material.rs` | 是 |
| 67 | `316dd5bd-d8d4-4a8e-b34b-6d4e54de7971` | `kairos_engine/src/kairos_editor/ui/inspector/mesh.rs` | `kairos_editor/src/ui/inspector/mesh.rs` | 是 |
| 68 | `ddeba15d-d340-483b-9a15-04895618f199` | `kairos_engine/src/kairos_editor/ui/inspector/shader.rs` | `kairos_editor/src/ui/inspector/shader.rs` | 是 |
| 69 | `0f00647f-1b41-45ee-9b08-a7e21d7075bb` | `kairos_engine/src/kairos_editor/ui/inspector/texture` | `kairos_editor/src/ui/inspector/texture` | 是 |
| 70 | `a46a64e4-144d-4af1-be17-b8f7548cc86a` | `kairos_engine/src/kairos_editor/ui/inspector/texture/edit.rs` | `kairos_editor/src/ui/inspector/texture/edit.rs` | 是 |
| 71 | `83d82a13-aba9-460b-a069-88c4138a8cb4` | `kairos_engine/src/kairos_editor/ui/inspector/texture/mod.rs` | `kairos_editor/src/ui/inspector/texture/mod.rs` | 是 |
| 72 | `a9491c5d-8fce-4b41-930c-8a22f8338ef5` | `kairos_engine/src/kairos_editor/ui/inspector/toml.rs` | `kairos_editor/src/ui/inspector/toml.rs` | 是 |
| 73 | `a347cf1e-9343-41bc-87af-8e43f38eb3c8` | `kairos_engine/src/kairos_editor/ui/inspector/unknown.rs` | `kairos_editor/src/ui/inspector/unknown.rs` | 是 |
| 74 | `ee28e9ec-f6f9-4e46-9d67-3e8a58b48622` | `kairos_engine/src/kairos_editor/ui/inspector.rs` | `kairos_editor/src/ui/inspector.rs` | 是 |
| 75 | `3bd54b5a-122b-4558-a7c3-697e38a71b0d` | `kairos_engine/src/kairos_editor/ui/inspector_window.rs` | `kairos_editor/src/ui/inspector_window.rs` | 是 |
| 76 | `8b1090de-f544-492b-a94f-fbf8fb4be61a` | `kairos_engine/src/kairos_editor/ui/layout.rs` | `kairos_editor/src/ui/layout.rs` | 是 |
| 77 | `2b3d6ab7-1318-42e5-af83-392fed92b17a` | `kairos_engine/src/kairos_editor/ui/native_dialog.rs` | `kairos_editor/src/ui/native_dialog.rs` | 是 |
| 78 | `e184b20d-2977-4757-9b52-7ab1a423333e` | `kairos_engine/src/kairos_editor/ui/paths.rs` | `kairos_editor/src/ui/paths.rs` | 是 |
| 79 | `513a00b4-1a12-4b82-a3e2-5c1cd46e9412` | `kairos_engine/src/kairos_editor/ui/preferences_window.rs` | `kairos_editor/src/ui/preferences_window.rs` | 是 |
| 80 | `ce8270b2-6e57-4079-9b36-640c8466b92f` | `kairos_engine/src/kairos_editor/ui/project_window` | `kairos_editor/src/ui/project_window` | 是 |
| 81 | `bd40f34d-52b1-46b9-bd00-4df826f3ee15` | `kairos_engine/src/kairos_editor/ui/project_window/content_panel.rs` | `kairos_editor/src/ui/project_window/content_panel.rs` | 是 |
| 82 | `bac8c823-2e19-4ea6-ae0e-df96d9a50226` | `kairos_engine/src/kairos_editor/ui/project_window/context_menu.rs` | `kairos_editor/src/ui/project_window/context_menu.rs` | 是 |
| 83 | `6a0bb60d-031f-47c2-b8af-458f3ad66e27` | `kairos_engine/src/kairos_editor/ui/project_window/hierarchy_panel.rs` | `kairos_editor/src/ui/project_window/hierarchy_panel.rs` | 是 |
| 84 | `34265ceb-343d-4366-bb0c-7fb25b9e6184` | `kairos_engine/src/kairos_editor/ui/project_window.rs` | `kairos_editor/src/ui/project_window.rs` | 是 |
| 85 | `4a3bbae7-fe47-42ef-aac8-e0690d57d086` | `kairos_engine/src/kairos_editor/ui/scene_window` | `kairos_editor/src/ui/scene_window` | 是 |
| 86 | `64c6e6cd-36bc-4fde-99ee-f70e4dcef014` | `kairos_engine/src/kairos_editor/ui/scene_window/gizmos` | `kairos_editor/src/ui/scene_window/gizmos` | 是 |
| 87 | `caf9f1a5-050c-411c-a389-3ffdacd96a18` | `kairos_engine/src/kairos_editor/ui/scene_window/gizmos/axes_indicator.rs` | `kairos_editor/src/ui/scene_window/gizmos/axes_indicator.rs` | 是 |
| 88 | `32a99c4c-b403-4770-a7b3-a99daf4a8b2f` | `kairos_engine/src/kairos_editor/ui/scene_window/gizmos/grid_plane.rs` | `kairos_editor/src/ui/scene_window/gizmos/grid_plane.rs` | 是 |
| 89 | `24defe9e-ff12-4934-9eef-11a468b57e00` | `kairos_engine/src/kairos_editor/ui/scene_window/gizmos.rs` | `kairos_editor/src/ui/scene_window/gizmos.rs` | 是 |
| 90 | `7d5d1708-be38-4d3c-a654-22d12aff3837` | `kairos_engine/src/kairos_editor/ui/scene_window.rs` | `kairos_editor/src/ui/scene_window.rs` | 是 |
| 91 | `aacfd175-6dc1-4198-a12e-a995730e5f7f` | `kairos_engine/src/kairos_editor/ui/tool_bar.rs` | `kairos_editor/src/ui/tool_bar.rs` | 是 |
| 92 | `a9d5a882-ca2f-40d5-b0de-67377b6266b9` | `kairos_engine/src/kairos_editor/ui/ui_style_fields.rs` | `kairos_editor/src/ui/ui_style_fields.rs` | 是 |
| 93 | `224b85b6-82af-40bf-b752-884795a26195` | `kairos_engine/src/kairos_editor/ui.rs` | `kairos_editor/src/ui.rs` | 是 |
| 94 | `62d8eb93-e3ce-4094-883e-7d0d8037c4ed` | `kairos_engine/src/kairos_editor.rs` | `kairos_editor/src/lib.rs` | 是 |

共 94 条（其中目录条目 18 条，`.rs` 文件条目 76 条）。

### 5.4 改写规则

| HEAD path 前缀 | 新 path 前缀 |
|---|---|
| `kairos_engine/src/kairos_editor.rs` | `kairos_editor/src/lib.rs` |
| `kairos_engine/src/kairos_editor` | `kairos_editor/src` |
| `kairos_engine/src/kairos_editor/<X>` | `kairos_editor/src/<X>` |
| `kairos_engine/src/kairos_dialog.rs` | `kairos_editor/src/dialog.rs` |

GUID 与 path 是**绑定**的（GUID 是身份、path 是位置），所以搬迁只需重写 path；GUID 全部保留即等于「位置迁移、身份不变」。**但注意**：`kairos_editor/schedule*` 三个文件是**留引擎**的，它们的条目应改到引擎侧路径（`kairos_engine/src/schedule…`）而不是 `kairos_editor/src/schedule…`；表里 #21–#23（`schedule*`）需特别处理，不能照 §5.4 的机械规则走。

---

## 6. 非编辑器侧的编辑器引用

全 workspace 检索 `kairos_editor` / `kairos_dialog` / `KairosEditorRuntime`（排除 `target/`、`.git/`、编辑器子树自身、`Library/asset_registry.toml`）后，实际需要改写的引用点如下。

| # | 引用点 | 现状 | 改写方案 |
|---|---|---|---|
| 1 | `kairos_engine/src/asset_pipeline.rs:35` | `use crate::kairos_editor::schedule;` | `use crate::schedule;`（`schedule` 归位引擎；文件其余部分不动） |
| 2 | `kairos_engine/src/lib.rs:5` | `pub mod kairos_dialog;` | **删除**（随编辑器走） |
| 3 | `kairos_engine/src/lib.rs:9` | `pub mod kairos_editor;` | **删除**；新增 `pub mod engine;` + `pub use engine::Engine;` + `pub mod schedule;` |
| 4 | `kairos_engine/src/kairos_game.rs:18` | 组内 `kairos_editor::{Engine, schedule},` | 拆为 `engine::Engine`（或 `crate::Engine`）与 `crate::schedule` —— **不能整组平移** |
| 5 | `kairos_engine/src/main.rs:1-4` | `use kairos_engine::{kairos_dialog, kairos_editor::runtime::{KairosEditorRuntime, KairosEditorRuntimeEvent}};` | 文件整体搬到 `kairos_editor/src/main.rs`，改为 `use kairos_editor::{dialog, runtime::{EditorRuntime, EditorRuntimeEvent}};`（命名按 map 决策：「crate 内去掉 `Kairos` 前缀」） |
| 6 | `kairos_engine/Cargo.toml:5` | `default-run = "kairos_engine"` | **删除**；新 crate 写 `default-run = "kairos_editor"`（裸 `cargo run` 继续指向编辑器） |
| 7 | `kairos_engine/src/bin/bake_assets.rs` | 只用 `env_logger` + `kairos_engine::asset_pipeline::bake_assets()`，**无编辑器引用** | 不动（继续留 `kairos_engine`；`asset_pipeline` 留引擎，见 #1） |
| 8 | `kairos_engine/benches/texture_encode_decode.rs` | `criterion` + `half::f16` + `kairos_engine::graphics::texture::format`，**无编辑器引用** | 不动 |
| 9 | `kairos_engine/tests/` | **目录不存在** | — |
| 10 | `Library/asset_registry.toml` | 94 条指向编辑器 | 见 §5 |
| 11 | `kairos_graphics/src/extract/test.rs:37` | 文档注释：`The real engine uses its \`kairos_editor::schedule::Extract\`` | `schedule` 归位引擎后该路径不再存在 → 改为 `kairos_engine::schedule::Extract` |
| 12 | `kairos_physics/src/tests.rs:10` | 文档注释：`its \`kairos_editor::schedule::FixedUpdate\`` | 改为 `kairos_engine::schedule::FixedUpdate` |
| 13 | `README.md:119`、`README-zh.md:123` | 「引擎代码结构」树把 `kairos_editor/` 画成 `kairos_engine/src/` 的下级 | 树形图与说明句需重述（属 [#258](https://github.com/WhitePetal/KairosEngine/issues/258) 的范围，本清单只登记） |
| 14 | `.agents/skills/implement-editor-inspector/SKILL.md:27,39,40` | 指示「`kairos_editor/editor_assets/<name>.rs`」「`kairos_editor/consts.rs`」「在 `kairos_editor.rs` 里加 `pub mod editor_assets;`」 | 路径与「模块根文件」的说法都需重述（[#258](https://github.com/WhitePetal/KairosEngine/issues/258) 范围） |
| 15 | `prototypes/kairos-editor-mcp-middleware/` | 仅在 `Cargo.toml:7` description 与 `src/lib.rs:1` 注释里出现 `kairos_editor_mcp`；**不依赖** `kairos_engine`，也无源码引用 | 本次无需改动（map「Not yet specified」已登记该 prototype 是否改为依赖新 crate） |
| 16 | `.mcp.json` / `.zed/settings.json` / `.cargo/config.toml` / `deny.toml` / `rust-toolchain.toml` | 无编辑器引用 | 不动 |
| 17 | `.codegraph/codegraph.db` | 二进制索引，含编辑器符号的位置 | 抽取后需 `codegraph` 重新索引（非代码引用，但会影响 agent 的 explore 结果） |

**未发现编辑器引用的地方**（明确排除）：`kairos_graphics/**`（除 #11 注释）、`kairos_physics/**`（除 #12 注释）、`kairos_asset/**`、`kairos_audio/**`、`kairos_ecs/**`、`kairos_math/**`、`kairos_time/**`、`kairos_transform/**`、`kairos_tasks/**`、`kairos_collections/**`、`kairos_supervisor/**`、`kairos_ptr/**`、`benches/`、`res/`、`Preferences/`（除 `Styles/AboutWindowStyle.toml` 里的窗口标题字符串）。

> 路径均相对 workspace 根 `KairosEngine/KairosEngine/`。「跨 crate」指编辑器搬去 `kairos_editor` crate 之后。
> 结论来自只读代码与 grep，未运行 cargo 构建/测试。

---

## 7. 易漏点

### 7.1 `KairosGame::new(&mut Engine)` 的调用点与归属

- **定义点**：`kairos_engine/src/kairos_game.rs:168` `pub struct KairosGame;`（无状态单元结构体）；`impl` 于 `:170`，签名 `pub fn new(engine: &mut Engine) -> Self`（`:171`）。
- **全 workspace 唯一代码调用点**：`kairos_engine/src/kairos_editor.rs:152` —— `let _ = KairosGame::new(&mut engine);`，位于 `KairosEngine::new`（编辑器宿主结构）内（`kairos_editor.rs:146-160`），返回值被丢弃。其余匹配仅在 `docs/**` 文档中。
- **归属与依赖**：`kairos_game.rs:18` `use crate::kairos_editor::{Engine, schedule};`，即当前依赖 `Engine` 所在模块。它只用 `&mut Engine` 的 `pub` 面：
  - `engine.input_engine.registe_input(...)` ×4：`:172-187`
  - `engine.world.resource::<AssetServer>()`：`:190,195,199,267,274`
  - `engine.world.spawn/resource_mut/entity_mut`：`:220-224,226,255-261,286-312`
  - `engine.world.get_resource_or_init::<Schedules>()` + `add_systems(schedule::Update, …)`：`:318-322`
- **含义（事实）**：`KairosGame::new` 接收 `&mut Engine` 意味着它须在 `Engine` 构造完成后运行，且同时改 `input_engine` 与 `world`。搬迁后只要 `Engine` 稳定在 `kairos_engine` 并 `pub` 再导出，该调用无论留在引擎还是被编辑器调用都能编译；但 `kairos_game.rs:18` 的 import 必须改为引擎内路径（`crate::engine::Engine` / `crate::schedule`）。`KairosGame` 不依赖编辑器 UI，留在引擎无循环依赖。

### 7.2 `Engine.input_engine`（pub 字段）的全部使用面

- **定义点**：`kairos_editor.rs:27` `pub input_engine: InputEngine,`（`Engine` 于 `:25-28`）。`InputEngine` 为 `pub struct`（`inputs.rs:39`），`new`/`registe_input`/`update_keyboard_input` 均 `pub`（`inputs.rs:46,71,94`）。
- **构造点**：`kairos_editor.rs:38`（`InputEngine::new()`），`:40-43` 装入 `Engine`。
- **引擎侧使用**：`kairos_game.rs:172,176,180,184`（`registe_input` ×4）。
- **编辑器侧使用**：`kairos_editor.rs:163` —— `KairosEngine::update_keyboard_input` 内 `self.engine.input_engine.update_keyboard_input(event)`；这是编辑器侧唯一直接触碰点。`KairosEditorRuntime` 经 `runtime.rs:237` 间接使用。编辑器 UI（`ui/**`）无直接读 `input_engine`。
- **跨 crate 合法性**：字段在 `pub struct` 上为 `pub`，`Engine` 从新 crate 根部再导出后，`engine.input_engine.…` 仍合法。无 `pub(crate)` 阻碍。

### 7.3 `Engine::time()` / `Engine::update()` 可见性，及编辑器侧 `&Engine` / `&mut Engine` 清单

可见性（全部够用）：

| 成员 | 定义 | 可见性 |
|---|---|---|
| `Engine.world` | `kairos_editor.rs:26` | `pub` |
| `Engine.input_engine` | `kairos_editor.rs:27` | `pub` |
| `Engine::new()` | `kairos_editor.rs:31` | `pub` |
| `Engine::time(&self) -> &Time` | `kairos_editor.rs:53` | `pub` |
| `Engine::update(&mut self)` | `kairos_editor.rs:64` | `pub` |

- `Engine::time()` 编辑器使用点仅一处：`kairos_editor/ui/inspector_window.rs:183`（`engine.time().delta_time_secs()`）。`Time::delta_time_secs` 为 `pub`（`kairos_time/src/lib.rs:118`）。
- `Engine::update()` 经 `KairosEngine::update`（`kairos_editor.rs:166-171`，`:170` 调 `self.engine.update()`）间接调用；`KairosEngine::update` 为**私有** `fn`，由 `runtime.rs:245` 调用。

编辑器侧接收 `&Engine` / `&mut Engine` 的签名与调用点：

**Trait 定义**

| 签名 | 位置 |
|---|---|
| `Drawer::ui(&self, ui, reader: &UIReader, messager: &mut Messager, engine: &Engine, log: &mut Log)` | `ui.rs:262-269` |
| `Drawer::render(&self, engine: &mut Engine, messager: &mut Messager) -> Option<GraphicsCommand>` | `ui.rs:271-275` |
| `TabDrawer::ui(&mut self, ui, reader, tab, messager, engine: &Engine, log)` | `ui/docking_tab/tab_drawer.rs:27-35` |
| `KairosTabDrawer::ui(... engine: &Engine ...)`（`:228` 转发 `tab.ui(...)`） | `ui.rs:218-228` |

**Context 层**

| 签名 | 位置 |
|---|---|
| `Context::darw(&mut self, ui, engine: &Engine, log)` | `ui.rs:372` |
| `Context::handle(&mut self, engine: &mut Engine, ui)` | `ui.rs:414` |
| `Context::render(&mut self, engine: &mut Engine) -> Vec<GraphicsCommand>` | `ui.rs:778` |
| `Context::show_tab<T>(&mut self, engine: &mut Engine, ui, zone)` | `ui.rs:831` |

**`DockArea` 贯穿参数（均 `engine: &Engine`）**：`ui/docking_tab.rs:187,208,456,477,770,807,1091,2275`；末端 `docking_tab.rs:2375` `tab_viewer.ui(..., engine, log)`。

**各 `Drawer` 实现（`&Engine` / `&mut Engine`）**：`about_window.rs:88/175`、`console_window.rs:74/107`、`game_window.rs:88/178`、`hierarchy_window.rs:76/105`、`inspector_window.rs:153/209`、`layout.rs:41/51,92/102,138/148,184/194`、`preferences_window.rs:103/177`、`project_window.rs:527/620`、`scene_window.rs:139/361`、`tool_bar.rs:143/340`。

**非 Drawer、直接收 `&mut Engine` 的方法**

| 签名 | 位置 | 调用点 |
|---|---|---|
| `AudioInspector::toggle_playback(&mut self, engine: &mut Engine)` | `ui/inspector/audio.rs:323` | `ui.rs:614` |
| `AudioInspector::seek_and_play(&mut self, engine: &mut Engine, position: f32)` | `ui/inspector/audio.rs:334` | `ui.rs:621` |
| `AudioInspector::play(&mut self, engine: &mut Engine)` | `ui/inspector/audio.rs:403` | `:326,:349` |

**顶层调用点**：`kairos_editor.rs:174`(`handle`)、`:181`(`darw`)、`:185`(`render`)、`:170`(`Engine::update`)；`Context` 内 `ui.rs:389,394-401,518,524,536,542,625,655,781`；`runtime.rs:268,270,272`。

**结论**：编辑器所需面全部 `pub`，`&mut Engine` 跨 crate 可用。

### 7.4 `engine.world` 的使用面

`engine.world` 字段 `pub`（`kairos_editor.rs:26`），`World` 来自外部 crate `kairos_ecs`（全 `pub`），搬迁后仍可访问。

| 类别 | 使用点 |
|---|---|
| `world.resource::<T>()` | `ui/game_window.rs:141`(GameView)、`ui/scene_window.rs:191`(SceneView)、`ui/inspector_window.rs:181`(以 `&World` 转发)、`ui.rs:551`、`ui/inspector/audio.rs:398`、`ui/scene_window.rs:419`/`ui/game_window.rs:235`(`world.entity(camera).get::<CameraView>()`) |
| `world.resource_mut::<T>()` | `ui.rs:514`(`apply_camera_style(&mut engine.world)`)、`ui.rs:639,642`(SceneView.size)、`:661,664`(GameView.size)、`ui/inspector/audio.rs:437`(`AudioEngine` 播放) |
| `world.get_resource_or_init::<T>()` | `ui.rs:677,681,685`(`SceneViewInput`) |
| 传 `&engine.world` / `&mut engine.world` 给 helper | `ui.rs:631`(spawn_editor_camera)、`ui.rs:689,692,695,698,706,712,719,727,735,743,751,762,769`；`runtime.rs:193,355` |
| 生产路径 `run_schedule` | 无直接调用；由 `Engine::update` 内 `self.world.run_schedule(schedule::Main)`（`kairos_editor.rs:65`）+ `clear_trackers()`（`:66`）驱动 |
| `insert_resource` / `run_schedule`（测试） | `camera/test.rs:63`、`editor_assets/text.rs:131`、`editor_assets/toml.rs:129`（均作用于裸 `World`，非 `engine.world`） |

### 7.5 `Engine::new` 内部引用 与 `KairosGame::new` 现状

`Engine::new`（`kairos_editor.rs:31-44`）：

- `:32` `let mut world = build_world();`
- `:37` `crate::audio::install(&mut world, AudioEngine::new()?, schedule::Update);`
- `:38` `let input_engine = InputEngine::new();`

路径可行性：`crate::audio` 为 `pub use kairos_audio as audio;`（`lib.rs:23`），`Engine` 留在引擎后 `crate::audio::install` 仍解析；`schedule::Update` 现靠同文件 `pub mod schedule;`（`kairos_editor.rs:21`）解析，`Engine` 移入 `engine.rs` 后须写 `crate::schedule::Update`（或 `use crate::schedule;`）。`crate::audio::install` 签名 `pub fn install(world, engine: AudioEngine, update_stage: impl ScheduleLabel)`（`kairos_audio/src/lib.rs:224`），`AudioEngine::new` 为 `pub`（同文件 `:59`）。

`KairosGame::new(&mut engine)` 现状：当前由编辑器宿主 `KairosEngine::new` 调用（`kairos_editor.rs:152`），而非 `Engine::new`。`KairosGame` 定义在引擎侧 `kairos_game.rs`，返回值不被保存。**仅陈述现状，不下决策。**

### 7.6 `#[cfg(test)]` 模块与 `Engine` 相关的测试分布

| 位置 | 类型 | 与 `Engine`/引擎 bootstrap 的关系 |
|---|---|---|
| `kairos_editor/schedule.rs:360`（`mod asset_test`） | 文件 `schedule/asset_test.rs` | **强耦合**：`use crate::kairos_editor::build_world;`（`:26`）、`crate::kairos_editor::editor_assets::{Text,Toml}`（`:17`）、`crate::kairos_editor::syntax::SyntaxHighlightSettings`（`:18`）；调 `build_world()`（`:40,92,103,125,148,160`）。注释称「the exact World `Engine::new` builds」 |
| `kairos_editor/schedule.rs:363`（`mod test`） | 文件 `schedule/test.rs` | 仅用 `super::{…}` 与 `crate::time`；注释提到刻意避开 `Engine::new()`（`test.rs:8`），无实际耦合 |
| `kairos_editor/camera.rs:238`（`mod test`） | 文件 `camera/test.rs` | **耦合**：`use crate::kairos_editor::schedule::{self, First, Main}`（`test.rs:32`），调 `schedule::install(&mut world)`（`:60`）；注释称避开 `Engine::new()`（`:6`） |
| `editor_assets/text.rs:67`、`editor_assets/toml.rs:65`、`syntax.rs:363` | 内联 | 裸 `World` + `crate::asset`，不用 `Engine` |
| `ui/inspector/material.rs:49`、`ui/inspector/audio.rs:22`、`ui/inspector/texture/edit.rs:150`、`ui/inspector/texture/mod.rs:760` | 文件/内联 | 裸 `World`；`material/test.rs:47` 直接用 `kairos_graphics::material::install` |
| `project_path_tree.rs:26`、`project_watcher.rs:29` | 文件 | 不用 `Engine` |
| `ui/docking_tab/dock_state/tree.rs:16`、`ui/ide_detection.rs:287` | 文件/内联 | 不用 `Engine` |

- 引擎侧（非编辑器）无 `Engine` 测试。
- 无任何测试直接构造 `Engine`；`Engine::new()` 在所有测试中被刻意回避（`camera/test.rs:6`、`schedule/test.rs:8`）。

### 7.7 硬阻碍清单（pub(crate)/私有）

1. **`schedule::install` 为 `pub(crate)`，编辑器测试要跨 crate 调它。** 定义 `kairos_editor/schedule.rs:298` `pub(crate) fn install(world: &mut World)`；使用 `camera/test.rs:60` `schedule::install(&mut world)`（`schedule` 见 `test.rs:32`）。`camera` 搬入 `kairos_editor` 后该调用解析为 `kairos_engine::schedule::install` → **跨 crate 编译失败**。修法：改 `pub`，或让该测试自搭 schedule。
2. **`build_world` 私有且与 `schedule` 分属不同模块。** 定义 `kairos_editor.rs:78` `fn build_world() -> World`（无 `pub`）。当前 `schedule/asset_test.rs:26` 能访问它，是因为 asset_test 是 `kairos_editor` 的后代模块。若 `build_world` 移入 `engine.rs` 保持私有、`schedule/` 移到 `crate::schedule`，则 `crate::schedule::asset_test` 与 `engine` 是兄弟分支，无法访问私有项 → 引擎 crate 内编译失败。修法：`pub(crate) fn build_world`。
3. **`build_world` 反向调用编辑器 install（engine → editor 循环依赖）。** `kairos_editor.rs:105,106,107`（`editor_assets::text/toml::install`、`syntax::install`）、`:127`（`ui::inspector::texture::install`）、`:135`（`camera::install`）。编辑器独立成 crate 后引擎不能再引用，须迁到编辑器的 `install(world)`。这些 install 函数本身均 `pub`：`editor_assets/text.rs:63`、`editor_assets/toml.rs:61`、`syntax.rs:359`、`ui/inspector/texture/mod.rs:750`、`camera.rs:189`。
4. **`KairosEngine` 私有字段/方法被 `runtime.rs` 直取。** 字段 `engine` 私有（`kairos_editor.rs:140`）；`update_keyboard_input`(`:162`)、`update`(`:166`)、`handle_ui`(`:173`)、`draw_ui`(`:177`)、`render_ui`(`:184`)、`on_exit`(`:188`) 均私有。使用点 `runtime.rs:118,193,237,245,268,270,272,355,451`。二者必须同 crate。
5. **`asset_pipeline.rs:35`**（引擎，留下）`use crate::kairos_editor::schedule` → 需改 `crate::schedule`。
6. **`kairos_game.rs:18`** `use crate::kairos_editor::{Engine, schedule}` → 需改引擎内路径。
7. **doctest 路径**：`ui/docking_tab/dock_state/tree.rs:328,355,371,400,548`、`.../tree/node.rs:169,189,244`、`.../dock_state.rs:123,530,563,581,600` 写死 `use kairos_engine::kairos_editor::ui::…`，模块搬迁后 doctest 编译失败，需改 `use kairos_editor::ui::…`。

### 7.8 需确认项

**确定结论（已核实，不再是开放问题）**

- `apply_camera_style` 收 `&mut World`，非 `&mut Engine`：`ui.rs:514` 传 `&mut engine.world`；`Context::handle` 自身收 `&mut Engine`，内部按需再借 `.world`。
- `Inspector` trait 不收 `Engine`：`ui/inspector.rs:26-36` 收 `&World` + `delta_time: f32`，由 `InspectorWindow` 桥接（`inspector_window.rs:178-184`）。
- `impl KairosGame` 当前仅有 `new` 与 `spawn_audio_scene`（`kairos_game.rs:170-402`），无 `update`/`render`；文档中的相关引用为历史事实。

**真正待定**

- `KairosGame::new` 的最终调用归属（引擎内 vs 编辑器），属设计决策。
- `KairosEngine` 与 `Engine` 现同处 `kairos_editor.rs`，搬迁需拆分；`KairosEngine` 是否改名（避免与 crate/`KairosGame` 混淆）未定。

## 8. KairosEditorRuntime 的耦合面

### 8.1 runtime.rs 耦合条目

`使用点` 均指 `kairos_engine/src/kairos_editor/runtime.rs`。

| 使用点 file:line | 定义点 file:line | 定义可见性 | 搬迁后可达? | 若不可达需做什么 |
|---|---|---|---|---|
| `:28`(use)、`:90`(字段类型)、`:117,190`(`RenderPipeline::new`)、`:198,205`(`.device`)、`:210`(`.max_texture_side()`)、`:252`、`:259`(`.get_window_surface()`)、`:313-314`(`.surface_config`)、`:354`(`.present()`)、`:515,520`(`.set_window_resize()`) | `kairos_graphics/src/lib.rs:26` `pub mod render_pipeline`；`render_pipeline.rs:176` `pub struct RenderPipeline`（`pub device` :178、`pub surface_config` :180），`pub async fn new` :207、`:318`、`:342`、`:976`、`:992` | **pub** | ✅ | — |
| `:23`(`GraphicsCommand`/`GraphicsGraph`)、`:319`(`new`)、`:324`、`:331`、`:332`、`:340`、`:345`、`:346`、`:353`(`build`) | `graphics_graph.rs:1-2` `mod graph; mod graphics_command;`（**私有模块**）+ `:5-6` `pub use graph::*; pub use graphics_command::*;`；`graphics_command.rs:22` `pub struct GraphicsCommand` + `pub fn` :31,44,56,62,98,131,213；`graph.rs:20` `pub struct GraphicsGraph`（字段 :21-25 全 `pub`）+ `pub fn build` :29 | **pub（经 `pub use` 再导出）** | ✅ | 走 `kairos_graphics::graphics_graph::{…}`；勿直连私有的 `graph`/`graphics_command` |
| `:19`(`ColorAttachmentBind`)、`:325`(`ColorAttachmentBind::new`) | `graphics_graph.rs:3` `pub mod graphics_node`；`graphics_node.rs:16` `pub struct`，`pub fn new` :21 | **pub** | ✅ | — |
| `:18`(`Attachment`/`AttachmentLoadAction`/`AttachmentStoreAction`/`InternalAttachmentId`)、`:320`、`:325` | `kairos_graphics/src/lib.rs:32` `pub mod attachment`；`attachment.rs:6,12,21,38`（枚举/结构），`pub fn new` :47、`pub fn from_internal_id` :62 | **pub** | ✅ | — |
| `:21`(`use crate::math::float4x4`)、`:331`(`float4x4::IDENTITY`) | `kairos_engine/src/lib.rs:3` `pub mod math`；`math.rs:3` `pub use kairos_math::*;`；`kairos_math/src/matrix.rs:13` `pub struct float4x4(pub(crate) glam::Mat4)`、`:16` `pub const IDENTITY` | 类型/常量 **pub**；内部 tuple 字段 `pub(crate)`（底层 `glam::Mat4`） | ✅（仅用 `IDENTITY`） | 若需从原始 `glam::Mat4` 构造/取内层则不可（当前未使用）；可直接依赖 `kairos_math` |
| `:24`(`use crate::kairos_paths`)、`:44`(`PATH_KAIROS_SETTINGS`) | `kairos_engine/src/lib.rs:11` `pub mod kairos_paths`；`kairos_paths.rs:1` `pub const` | **pub** | ✅（`kairos_engine::kairos_paths`） | — |
| `:25`(`EngineSettings`)、`:45`(`toml::from_slice::<…>`)、`:189-192`(`settings.texture_compression`) | `kairos_engine/src/lib.rs:12` `pub mod kairos_settings`；`kairos_settings.rs:11` `pub struct EngineSettings`、`:12` `pub texture_compression`；字段类型 `TextureCompressionConfig`：`kairos_graphics/src/texture/format.rs:1156` `pub struct` | **pub** | ✅ | — |
| `:29`(`use … kairos_dialog`)、`:467`(`error_message_window`) | `kairos_engine/src/lib.rs:5` `pub mod kairos_dialog`；`kairos_dialog.rs:6` `pub fn` | **pub** | 若 `kairos_dialog.rs` 随编辑器走 → 本地 `crate::kairos_dialog`；若留下 → `kairos_engine::kairos_dialog` 可达 | 两种均可，但调用处路径须相应改写；不可并存同名模块 |
| `:30`(`use … kairos_editor::{KairosEngine, consts, ui::paths}`)、`:118`、`:193,355`(`.engine.world`)、`:237,245,268,270,272,451`(私有方法)、`:36,157`(`consts`)、`:36,177`(`paths::PATH_ENGINE_ICON`) | `kairos_editor.rs:139` `pub struct KairosEngine`（字段 `engine` **私有** :140）；`consts.rs:1-2` `pub const`；`ui/paths.rs:1` `pub const` | struct/const **pub**；字段与方法 **私有** | 随编辑器搬入同 crate → 本地 `crate::{…}` 可达 | `KairosEngine` 必须与 `runtime` 同 crate（见 7.7-4） |

**结论（已核实）**：`runtime.rs` 全文**没有**任何 `schedule` 引用，也不存在 `crate::kairos_editor::schedule` 的用法。编辑器侧的 schedule 耦合实际在 `kairos_editor/camera.rs:27`（引入 `schedule::PostUpdate`）与 `camera/test.rs:32,60`（引入并调用 `schedule::install`）。

### 8.2 整个 `kairos_editor/**` 的外部 crate 引用扫描

针对 `crate::graphics::` / `crate::asset::` / `crate::audio::` / `crate::physics::` / `crate::math::` / `crate::kairos_paths` / `crate::kairos_settings` / `crate::kairos_game` / `crate::kairos_ui` / `crate::inputs` / `crate::log` / `crate::time` 的命中，逐条核对目标可见性：

| 前缀 | 目标可见性 | 编辑器是否引用到 `pub(crate)`/私有 |
|---|---|---|
| `crate::graphics::*` | 全部 `pub`（`texture`/`material`/`mesh`/`shader`/`vertex`/`render_state`/`compare_function`/`camera`/`view_port`/`graphics_graph`/`attachment`/`egui_texture_handle`/`lod_mesh_component`/`material_component`） | 否。`kairos_graphics` 的 `pub(crate)` 仅 `asset_events.rs:38,49`、`test_support.rs:7`，编辑器未用。`texture::SamplerConfig` 在 `texture.rs:23` 是私有 `use`，但编辑器全走 `texture::sampler::SamplerConfig`（`ui/inspector/texture/edit.rs:153`、`ui/inspector/texture/mod.rs:768`），可达 |
| `crate::asset::*` | 全部 `pub`（`Asset`/`AssetLoader`/`AssetWorldExt`/`LoadContext`/`Reader`/`VisitAssetDependencies`/`AssetServer`/`Assets`/`Handle`/`install`/`AssetOptions`/`AssetStages`/`AssetEventSystems`/`AssetTrackingSystems`/`AssetAction`/`AssetMeta`/`AssetMetaDyn`/`AssetId`、`io::get_meta_path` `io.rs:600`、`meta::processor_name` `meta.rs:143`、`AssetServer::add_async` `server.rs:951`、`Handle::id` `handle.rs:200`） | 否。`kairos_asset` 的 `pub(crate)`（`asset_changed.rs:55`、`assets.rs:325/337/449/500/525/584`、`handle.rs:36/74/141/154/173/591`、`id.rs:20`、`index.rs:20/117/125`）编辑器未引用 |
| `crate::audio::*` | 全部 `pub`（`lib.rs:47,84,195,224`、`audio.rs:225`、`audio_ext.rs:91`、`pcm.rs:237`） | 否。`kairos_audio` 的 `pub(crate)`（`pcm.rs:246`、`driver.rs:22`、`lib.rs:102,175`）编辑器未引用；`ui/inspector/audio/test.rs:103` 的 `write_wav_bytes` 是自建同名函数 |
| `crate::physics::*` | 编辑器**无命中**；`kairos_physics` 的 `pub(crate)`（`collider.rs:25`、`rigid_body.rs:15`、`lib.rs:171`）编辑器未引用 | 否 |
| `crate::math::*` | `math.rs:3` `pub use kairos_math::*`，成员均 `pub` | 否 |
| `crate::kairos_paths` / `crate::kairos_settings` / `crate::kairos_ui::font::Font` | 均 `pub mod` + `pub` 项（`kairos_ui.rs:1`、`font.rs:15`） | 否 |
| `crate::inputs` | 仅 `kairos_editor.rs:4`（随编辑器搬）；`InputEngine`/`Input` 均 `pub` | 否 |
| `crate::log` | `pub mod log`；`Log`/`Log::new` 均 `pub`（`log.rs:25,30`） | 否 |
| `crate::time` | `pub use kairos_time as time`（`lib.rs:46`）；`Time`/`delta_time`/`delta_time_secs` 均 `pub` | 否 |
| `crate::kairos_game` | 编辑器**无命中**（仅 `kairos_editor.rs:5` 的 `use`） | 否 |

### 8.3 硬阻碍清单（pub(crate)/私有）

1. **`kairos_engine::schedule::install` 为 `pub(crate)`**（`kairos_editor/schedule.rs:298`），被编辑器测试 `camera/test.rs:60` 引用；`camera` 搬入 `kairos_editor` 后解析为 `kairos_engine::schedule::install` → **跨 crate 编译失败**。必须改 `pub` 或重构该测试。这是 `kairos_editor/**` 中唯一指向「留下模块」的 `pub(crate)` 条目。
2. **`build_world` 私有 + `schedule/asset_test.rs:26` 引用**（若该测试随 `schedule/` 留在引擎）→ 引擎 crate 内跨模块私有访问失败，且 `asset_test.rs:17,18` 还引用两个会搬走的编辑器类型。需 `pub(crate) build_world` 并重排该测试归属。
3. **`KairosEngine` 私有字段/方法与 `runtime.rs` 同 crate 约束**（`kairos_editor.rs:140,162,166,173,177,184,188` ↔ `runtime.rs:118,193,237,245,268,270,272,355,451`）。`KairosEngine` 必须进 `kairos_editor`。
4. **引擎 → 编辑器反向 install 调用**（`kairos_editor.rs:105,106,107,127,135`）→ 必须改为编辑器的 `install(world)`。
5. **`crate::kairos_editor::schedule` 的外部引用**：`asset_pipeline.rs:35`（留下 → `crate::schedule`）、`kairos_game.rs:18`（留下 → 引擎内路径）、`camera.rs:27`（搬走 → `kairos_engine::schedule::PostUpdate`，`pub` 可）。
6. **doctest 路径** `use kairos_engine::kairos_editor::…`（`ui/docking_tab/**`，详见 7.7-7）。
7. **`runtime.rs` 的 `kairos_dialog` / `kairos_editor::{KairosEngine, consts, ui::paths}`**：随编辑器搬迁后应改为 crate 本地路径；若 `kairos_dialog.rs` 留在引擎则保持 `kairos_engine::kairos_dialog`。二者不可并存同名。

### 8.4 需确认项

**确定结论（已核实）**

- `runtime.rs` 全文无 `schedule` 引用（也不存在 `crate::kairos_editor::schedule` 用法）；`schedule` 耦合在 `camera.rs:27` 与 `camera/test.rs:32,60`。
- `kairos_editor/schedule.rs:298` 为 `pub(crate) fn install`。
- `schedule/asset_test.rs` 引用 `crate::kairos_editor::build_world`（`:26`）、`editor_assets::{Text,Toml}`（`:17`）、`syntax::SyntaxHighlightSettings`（`:18`）。

**真正待定**

- `kairos_dialog.rs` 的最终归属（搬去编辑器 vs 留在引擎），决定 `runtime.rs:29,467`、`ui.rs:505`、`about_window.rs:3` 的 import 路径。
- `KairosEngine` 的命名与拆分（`Engine`→`engine.rs`，`KairosEngine`→编辑器 crate）；是否改名未定。
- `KairosEngine` 私有方法（`update`/`handle_ui`/`draw_ui`/`render_ui`/`on_exit`/`update_keyboard_input`）是否改 `pub(crate)` 或改为受控 API；当前与 `runtime.rs` 的耦合依赖「同 crate 私有可见」这一隐式契约。
- `main.rs`（编辑器 bin）、`kairos_engine/Cargo.toml:5` 的 `default-run = "kairos_engine"`、`src/bin/bake_assets.rs`（只用 `asset_pipeline`，`bake_assets.rs:19`）在拆分后的 bin 归属；`bake_assets` 需继续留在 `kairos_engine`（依赖 `asset_pipeline`，而后者引用 `schedule`）。
---

## 9. 潜伏问题登记（本 effort 只迁移、不修复，仅登记）

| # | 问题 | 证据 |
|---|---|---|
| 1 | **注册表 3 条陈旧条目**指向已被 audio 抽取删掉的文件 | `serialize_asset` 三条（§5.1）磁盘不存在 |
| 2 | **`schedule/asset_test.rs` 归属自相矛盾**：它在 `schedule`（留引擎）下，却 `use crate::kairos_editor::{build_world, editor_assets::{Text, Toml}, syntax::SyntaxHighlightSettings}`（编辑器侧）。留引擎 = 引擎反赖编辑器（禁止）；随编辑器走 = 又违背「`schedule` 留引擎」 | `schedule/asset_test.rs:17,18,26,40` |
| 3 | **`schedule::install` 是 `pub(crate)`** —— 编辑器测试 `camera/test.rs:60` 搬迁后跨 crate 调用会**编译失败** | `schedule.rs:298`；`camera/test.rs:32,60` |
| 4 | **`build_world` 是私有 `fn`** —— 若 `Engine`+`build_world` 进 `engine.rs`、`schedule` 进 `schedule/`，则 `crate::schedule::asset_test` 无法访问私有 `build_world`（引擎 crate 内编译失败），需改 `pub(crate)` | `kairos_editor.rs:78` |
| 5 | **引擎→编辑器反向 install**：`build_world` 调用编辑器资产/相机的 `install`，拆分后必须整体迁到编辑器侧 `install(world)` | `kairos_editor.rs:105,106,107,127,135` |
| 6 | **`KairosEngine` 私有字段/方法与 `runtime.rs` 的隐式同 crate 契约**：`engine` 字段与 `update`/`handle_ui`/`draw_ui`/`render_ui`/`on_exit`/`update_keyboard_input` 全私有，`runtime.rs` 直接调用 —— `KairosEngine` 必须与 `runtime` 同 crate | `kairos_editor.rs:140,162-190` ↔ `runtime.rs:118,193,237,245,268,270,272,355,451` |
| 7 | **残缺 doctest fence**：`node.rs:169-174` 里嵌了一层 `` `/// ```rust` ``，使该示例块体只剩一行注释 | `node.rs:169,170,174` |
| 8 | **intra-doc link 拼写错误**：`(create::kairos_engine::…)`，`create` 应为 `crate` | `ui/docking_tab/dock_state.rs:100` |
| 9 | **上游 `egui_dock` fork 遗留的死链**：`crate::DockState` / `crate::Tree` / `crate::DockArea::show_add_popup`（推断今日已断） | §4.2 (b) |
| 10 | **`main.rs` 用 `#[tokio::main]`** —— 这让 `tokio` 成为编辑器 crate 的**直接**依赖（不是仅测试用） | `kairos_editor/src/main.rs:7`（原 `kairos_engine/src/main.rs:7`） |
| 11 | **引擎的 `math` 模块反向依赖 UI 生态**：`math/color/converts.rs` 实现 `From<Color32> for egui::Color32` 与 `for syntect::highlighting::Color`。`math` 留引擎，故 `egui`/`syntect` 必须留引擎 —— 但这条「引擎依赖 UI 库」的耦合值得登记 | `math/color/converts.rs:3,5,9,10,26` |
| 12 | **workspace 无 `[workspace.dependencies]`** —— `winit`/`egui*`/`wgpu`/`syntect`/`pollster`/`serde`/`image` 等会被两个 crate 各写一遍版本，存在漂移风险（`wgpu` 29 与 `egui-wgpu` 0.35 必须成对） | `Cargo.toml`（workspace）只有 `[workspace.package]` |
| 13 | **两侧都无直接引用的疑似遗留依赖**：`anyhow`、`base64`、`gltf`、`sonic-rs`、`serde_json`、`indexmap`、`bytemuck`、`rayon`、`glam`；`futures-util` / `tokio-tungstenite` 为 optional 且**无任何 feature 启用**。摘除前需编译确认 | §2.2 |
| 14 | **命名碰撞**：map 已定 `KairosEngine` → `Editor`，但「`Editor` 结构体 / `kairos_editor` crate / `kairos_editor` 模块」三者同名，机械改写时易混 | map Notes |
| 15 | **`KairosGame` 留引擎但只被编辑器路径调用**（`let _ = KairosGame::new(&mut engine);`）—— 引擎里因此存在一个「只服务编辑器」的模块 | `kairos_game.rs:171`；`kairos_editor.rs:152` |
| 16 | **`build_world` 的 install 顺序是隐式前置条件链**（schedule → asset → font → text/toml/syntax → physics → graphics(+assets) → 编辑器 texture inspector → audio → editor camera）。拆成「引擎 bootstrap + 编辑器 install」时，顺序语义必须保持 | `kairos_editor.rs:83-136` |

---

## 10. 交给下游票的输入

| 下游票 | 本清单提供的输入 |
|---|---|
| [#253](https://github.com/WhitePetal/KairosEngine/issues/253) 注册表迁移形态（GUID 稳定 vs 重分配） | §5：**94 条**（18 目录 + 76 `.rs`），GUID 与 path 分离，path 全部机械可推；另有 3 条**已陈旧**（`serialize_asset*`）需一并处理；`schedule*` 三条例外（留引擎） |
| [#254](https://github.com/WhitePetal/KairosEngine/issues/254) 测试与验收基线 | §4.3：`cargo test-crate` 别名**不含 doctest**，13 处 `kairos_engine::kairos_editor::…` doctest 只在 `cargo test-full` 暴露；§4.2 的 intra-doc 死链是 warning；`--bins` 会编译新 bin |
| [#256](https://github.com/WhitePetal/KairosEngine/issues/256) 安装顺序前置条件 | §9 #16 的顺序链；§7 的 `build_world` 拆分事实；各 `install` 签名与前置条件（`world.insert_resource` / `get_resource_or_init::<Schedules>` 语义） |
| [#257](https://github.com/WhitePetal/KairosEngine/issues/257) crate 接线形态 | §2（依赖集逐项证据 + 建议 `[dependencies]`/`[dev-dependencies]`/`[target.'cfg(macos)']`）、§3（features 全部纯透传 `kairos_ecs`）、§6 #6（`default-run`）、§9 #12（无 `[workspace.dependencies]`；成员表需加 `kairos_editor`） |
| [#258](https://github.com/WhitePetal/KairosEngine/issues/258) 文档与 agent 资产同步 | §6 #11–#14：README 双语树形图、`implement-editor-inspector` SKILL.md、`kairos_graphics`/`kairos_physics` 测试注释里的 `kairos_editor::schedule::…` |
| [#259](https://github.com/WhitePetal/KairosEngine/issues/259) 安装入口最终形态 | §9 #2/#5（`build_world` 拆分 + `asset_test.rs` 归属）、§7 全部、§8 的跨 crate 可达性结论 |

### 需编译确认的推断（本清单未验证）

1. §4.2 (b) 的三处 `crate::DockState` / `crate::Tree` / `crate::DockArea` **今天**是否已报 broken intra-doc link（需 `cargo doc` / rustdoc 输出）。
2. §2.2 中「engine 侧已无引用」的依赖，逐个摘除后是否仍能编译（尤以 `anyhow` / `gltf` / `sonic-rs` / `bytemuck` / `rayon` / `glam` 这类疑似遗留项）。
3. §2.1 建议的 `emath` / `epaint` 显式依赖是否真的必需（`egui` 会再导出它们；**直接写 `emath::` / `epaint::` 路径时通常必须显式声明**，但需编译确认）。
4. `kairos_collections` 是否只被编辑器使用（`ui.rs:2` 一处；引擎侧 0 命中）—— 若是，它应成为 `kairos_editor` 的直接依赖，引擎可摘。
5. §8 表格中「搬迁后可达」的结论逐条编译确认（尤其是 `graphics_graph` 的 `pub use` 再导出路径）。

