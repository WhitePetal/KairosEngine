# 编辑器 install 拆分后的前置条件核实（`build_world` → `Engine::new()` 之后）

> **来源票**：wayfinder [#256](https://github.com/WhitePetal/KairosEngine/issues/256)（map [#252](https://github.com/WhitePetal/KairosEngine/issues/252)）
> **基准**：`HEAD = 4d827b0`（2026-09-14，`docs(research): inventory the editor crate extraction touchpoints`）
> **一次性分支**：`research/editor-install-order-preconditions`
> **口径**：只查事实、不落实现。本文件由读码 + 现有测试得出，**未新增任何代码**；凡未证实处标「不确定」。
> **语言**：中文。

本文回答一个具体问题：把编辑器侧的 5 个 install（`text` / `toml` / `syntax` / `ui::inspector::texture` / `camera`）从 `build_world()` 内部挪到 `Engine::new()?` 之后执行，会不会打断真正的安装期前置条件。

---

## 0. 结论速览

| # | 子问题 | 判定 |
|---|---|---|
| 1 | `text`/`toml`/`syntax` 必须晚于 `asset::install`？ | **真前置**（`AssetServer` + `AssetStages` 两个资源） |
| 2 | `text`/`toml`/`syntax` 必须晚于 `font::install`？ | **非前置**（分组习惯） |
| 3 | `text`/`toml`/`syntax` 三者之间有序？ | **非前置**（扩展名/类型互不重叠） |
| 4 | `TomlLoader` 认领 `.toml`、`SyntaxHighlightSettingsLoader` 不认领——`syntax.rs` 注释成立？ | **成立**（`extensions() = &[]`；未类型化 `.toml` → `TomlLoader`） |
| 5 | `ui::inspector::texture::install` 必须早于 `pcm`/`audio`/`audio_ext`？ | **非前置** |
| 6 | `camera::install` 必须晚于 `graphics::install` / `install_assets`？ | **非前置**；只依赖 `schedule::install` |
| 7 | `text`/`toml`/`syntax`/`texture` 是否注册系统、是否依赖 `physics`/`graphics` 建 stage？ | 注册（经 `init_asset`），但走 `Schedules::entry` 自动建 stage，**非前置** |
| 8 | 同扩展名被两个 loader 认领 | **真顺序敏感**：`.wgsl` 同时被 `TextLoader` 与 `ShaderLoader` 认领（潜伏，见 §4） |
| 9 | 5 个 install 收敛到 `Engine::new()` 之后一次补装是否安全 | **除 `.wgsl` 外安全**；保留 `text` 原位即可完全安全（见 §5） |

---

## 1. 现状

`kairos_engine/src/kairos_editor.rs:78-137` 的 `build_world()`（**加粗** = 编辑器项）：

```
schedule::install
asset::install(AssetOptions)
kairos_ui::font::install
editor_assets::text::install          ← 编辑器
editor_assets::toml::install          ← 编辑器
syntax::install                       ← 编辑器
physics::install(FixedUpdate)
graphics::install(Extract)
graphics::install_assets(Extract)
ui::inspector::texture::install       ← 编辑器
audio::audio_ext::pcm::install
audio::audio::install
audio::audio_ext::install
camera::install                       ← 编辑器
```

`Engine::new()`（`kairos_editor.rs:31-44`）在 `build_world()` 之后唯一追加的是
`crate::audio::install(&mut world, AudioEngine::new()?, schedule::Update)`（`L37`）。

拆分会把这 5 个 install 的**相对位置整体后移**：`text` 会落到 `graphics::install_assets` 之后，
`camera` 会落到 `audio` 之后。下面逐条核对。

---

## 2. 关键机制（先读码定性）

### 2.1 loader 选取规则（决定「同扩展名」行为）

`kairos_asset/src/server/loaders.rs` 的 `AssetLoaders`：

- 注册即按顺序追加；注释明说 **last one registered wins**
  （`loaders.rs:42-47`：「Later registrations win when a lookup is ambiguous, mirroring bevy's "last one registered" resolution」）。
- `get_by_extension`：`self.extension_to_loaders.get(extension)?.last()`
  （`loaders.rs:143-147`）——同扩展名取**最后注册**的 loader。
- `get_by_path`：同样对每个候选扩展取 `.last()`（`loaders.rs:151-158`）。
- `find(asset_type_id, path)`（`loaders.rs:165-214`）是真正的解析入口，分两段：
  1. **路径无 label 时先按类型解析**（`loaders.rs:172-184`）：
     `candidates = type_id_to_loaders[type_id]`；
     `candidates.len() == 1` → **直接返回该 loader**（完全不看扩展名），
     `candidates` 为空 → 返回 `None`（`loaders.rs:179-180`）。
  2. 否则按扩展名：`try_extension` 在候选集内取 `.rev().find(...)`（= 候选里最后注册的），
     无类型候选时取 `indices.last()`（`loaders.rs:186-196`）。

**含义**：只要某资产类型只有 1 个 loader，**typed load 完全与注册顺序无关**；
只有 **untyped load**（`load_untyped` / `load_folder`，`asset_type_id = None`）才吃「最后注册者」。
注意第 1 段的类型解析在**路径带 label 时被跳过**（`loaders.rs:172-176`），
所以「按类型解析」这条在带 label 的路径上不成立（见 §3.2 的注）。

### 2.2 install 期的资源依赖

`kairos_asset/src/install.rs`：

- `init_asset_with_capacity::<A>`（`install.rs:424-466`）：
  - `L429`：`let server = self.resource::<AssetServer>().clone();`
  - `L453`：`let stages = *self.resource::<AssetStages>();`
  - `L454-464`：`self.resource_scope::<Schedules,_>(|world, mut schedules| { schedules.entry(stages.tracking).add_systems(...); schedules.entry(stages.event).add_systems(...) })`
- `register_asset_loader::<L>`（`install.rs:468-471`）：`self.resource::<AssetServer>().register_loader(loader)`——**只要 `AssetServer`，不要 `AssetStages`**。

`World::resource` 在资源缺失时 **panic**
（`kairos_ecs/src/world.rs:2285-2295`，消息为
`Requested resource {type} does not exist in the `World`…`）。
因此 `AssetServer` / `AssetStages` 必须已由 `asset::install` 插入。

`asset::install`（`install.rs:244-368`）确实插入两者：
`L337 world.insert_resource(server)`、`L341 world.insert_resource(stages)`。
`AssetServer::register_asset`（`server.rs:251-279`）登记该类型的 handle provider；
provider 缺失时，后续 `get_or_create_path_handle_erased` 会走
`create_handle_internal` 的 `missing_provider(...)`（`server/info.rs:321-341`；doc 注释 `info.rs:216-220` 明说 “Panics if no handle provider has been registered”）。
这条是**运行期** typed load 的约束，不是安装期顺序约束（见 §3.3）。

### 2.3 stage 不存在时的行为

- `Schedules::add_systems` / `configure_sets` 内部走 `self.entry(schedule)`
  （`kairos_ecs/src/schedule/schedule.rs:261-269`、`288-296`），
  而 `entry` 会 `or_insert_with(|| Schedule::new(label))`
  （`schedule.rs:176-180`）——**自动建 stage，不 panic**。
- `Schedules::get_mut(label).expect(...)` 才会 panic（`schedule.rs:169-173`）。
  - `graphics::install`：`.expect("the `Extract` schedule must exist: install the schedule rails first")`（`kairos_graphics/src/lib.rs:82-84`）。
  - `graphics::install_assets`：`.expect("the extract schedule must exist: install the schedule rails first")`（`kairos_graphics/src/asset_events.rs:138-141`）。
  - `camera::install`：`.expect("the `PostUpdate` schedule must exist: install the schedule rails first")`（`kairos_editor/camera.rs:195-198`）。

`schedule::install`（`kairos_engine/src/kairos_editor/schedule.rs:298-357`）一次性插入
`Startup / PreUpdate / FixedUpdate / Update / PostUpdate / Extract / Last` 等全部 label
（`L337-347`），所以只要 `schedule::install` 跑过，上述 `expect` 都有满足条件。

---

## 3. 逐条判定

### 3.1 `text` / `toml` / `syntax` vs `asset::install` 与 `font::install`

**vs `asset::install`：真前置。** 三者的 `install` 都是
`init_asset_with_capacity` + `register_asset_loader`：
- `text.rs:63-66`、`toml.rs:61-64`、`syntax.rs:359-362`。
二者都读 `AssetServer`（`install.rs:429/469`），`init_asset_with_capacity` 还读 `AssetStages`（`L453`）。
三者的 doc 也各自写明「Must run after `crate::asset::install`」（`text.rs:61-62`、`toml.rs:59-60`、`syntax.rs:357-358`）——**注释与代码一致，已验证**。

**vs `font::install`：非前置。** `font::install`（`kairos_ui/font.rs:59-62`）同样只是
`init_asset_with_capacity::<Font>` + `register_asset_loader(FontLoader)`；
`FontLoader::extensions() = &["ttf"]`（`font.rs:50-52`），与 text/toml/syntax 无扩展名重叠、
无类型重叠、无共享资源。当前 `build_world` 里 font 在它们之前只是分组习惯。

### 3.2 三者之间；`TomlLoader` 与 `SyntaxHighlightSettingsLoader` 的扩展名认领

**三者之间：非前置。** 证据：
- `TextLoader::extensions() = &["rs","md","txt","wgsl"]`（`text.rs:54-56`）
- `TomlLoader::extensions() = &["toml"]`（`toml.rs:52-54`）
- `SyntaxHighlightSettingsLoader::extensions() = &[]`（`syntax.rs:350-352`）

扩展名集合两两不相交，且各资产类型只有一个 loader，因此 typed load 走
`find` 的第 1 段「唯一类型 → 直接返回」（`loaders.rs:181-183`），与注册顺序无关。
`build_world` 注释自称三者「mutually independent, so the order here is a convenience grouping, not a precondition」（`kairos_editor.rs:102-104`）——**基本成立**（但见 §4 的 `.wgsl` 反例：那条注释漏掉了 text 与 graphics 的 `wgsl` 重叠）。

**`syntax.rs` 的「不认领 `.toml`」注释成立。**
`syntax.rs:346-352` 注释称语法 loader 按类型解析、不认领 `.toml`，代码 `extensions() = &[]` 与之一致。
于是未类型化 `.toml` 的候选只有 `TomlLoader`，取到它（`loaders.rs:194` `indices.last()`），
不会出现「两个 loader 争 `.toml`」——**注释核实为真**。

> 注（边界）：`SyntaxHighlightSettings` 目前都以 `load::<SyntaxHighlightSettings>(未带 label 的路径)` 加载
> （如 `ui/inspector/shader.rs:73`、`ui/inspector/code.rs:71`），落在 `find` 的类型分支上；
> 若哪天用**带 label** 的 `.toml#foo` 路径去 load 该类型，`find` 会跳过类型分支、落到 `TomlLoader`
> （`loaders.rs:172-176, 194`），语义会错。当前无此调用点，仅登记为边界条件。

### 3.3 `ui::inspector::texture::install` vs `pcm` / `audio` / `audio_ext`

**非前置。** `ui/inspector/texture/mod.rs:750-752` 的 `install` 只有一行
`world.init_asset::<TextureEdit>()`，**不注册任何 loader**（`TextureEdit` 只在
`ui/inspector/texture/edit.rs` 里经 `AssetServer::add_async` 落地，`edit.rs:39-44` 只实现 `Asset`）。
它只需要 `AssetServer` + `AssetStages`（来自 `asset::install`），与 audio 三者的
store/loader/扩展名均无交集。`build_world` 里它排在 audio 之前纯属分组。

### 3.4 audio 三者（`pcm` → `audio` → `audio_ext`）与 `texture` 夹层

**安装期：非前置（三者内部）；运行期：真前置。**
- 安装动作彼此独立：`pcm.rs:237-240`（`PcmData`，扩展名 `ogg/wav/mp3/flac`，`pcm.rs:226-230`）、
  `audio.rs:225-228`（`AudioAsset`，扩展名 `audio`，`audio.rs:214-218`）、
  `audio_ext.rs:91-94`（`AudioExt`，扩展名 `&[]`，`audio_ext.rs:78-84`）。
  三者的 `install` 都只读 `AssetServer`（+ `AssetStages`），互相之间**没有安装期读依赖**。
- `audio_ext.rs:87-90` 注释称 `AudioExt` “Must run after … the `AudioAsset` and `PcmData` stores”：
  这指的是**运行期**——`AudioExtLoader::load` 内 `load_context.load::<AudioAsset>` /
  `load::<PcmData>`（`audio_ext.rs:67-69`）是 typed nested load，
  需要那两个类型的 store/handle provider 已存在（§2.2；provider 缺失会在
  `create_handle_internal` → `missing_provider` 处失败）。
  这条只在**首次 `AudioExt` 加载**时兑现；只要三个 install 都先于首帧/首个 inspector 加载跑完即满足。
- **`texture::install` 不夹在约束中间**：它与 audio 三者无任何安装期交集。

所以：保留 `pcm → audio → audio_ext` 的相对顺序是**稳妥的**（因为统一 install 会把三者一次性装完），
但三者之间不存在必须如此的交错前置。

### 3.5 `camera::install` 的实际依赖

**只依赖 `schedule::install`；与 `graphics` 无关。** 全文 `camera.rs:189-199`：

```rust
pub fn install(world: &mut World) {
    world.init_resource::<SceneViewInput>();
    let mut schedules = world.get_resource_or_init::<Schedules>();
    schedules
        .get_mut(PostUpdate)                                 // ← 需要 PostUpdate 已存在
        .expect("the `PostUpdate` schedule must exist: install the schedule rails first")
        .add_systems(editor_camera_controller_system);
}
```

- 它不触碰 `GameView` / `SceneView`（那是 `graphics::install` 在 `kairos_graphics/src/lib.rs:78-79` 用
  `init_resource` 建的）、也不触碰 `Extract`。
- `editor_camera_controller_system` 的参数是
  `ResMut<SceneViewInput>`、`Res<Time>`、`Query<(&mut EditorCameraController, &mut LocalTransform)>`
  （`camera.rs:212-216`），没有 `Res<GameView>`、没有与 render 交接的资源。
- `PostUpdate` 与 `Time` 都由 `schedule::install` 提供（`schedule.rs:305` 插 `Time`，`L337-347` 建 `PostUpdate`）。
- `camera.rs:180-183` / `L11-15` 的注释讲的是**运行期**「PostUpdate 写在 Extract 读之前」，
  这是 schedule 边界属性，不是安装期前置。`build_world` 里 `graphics::install` 恰好在 `camera::install` 之前，
  但那是巧合，不是约束。

`camera/test.rs:58-71` 的 `boot()` 虽按 `schedule::install → graphics::install → camera::install` 排列，
但其中 `graphics::install` 是测试要跑 extract 断言所需，不是 `camera::install` 的要求。

**结论：`camera::install` 后移到 `Engine::new()` 之后完全安全**（`schedule::install` 已在 `build_world` 内跑过）。

### 3.6 系统注册与 stage 创建顺序

- `text`/`toml`/`syntax`/`texture` **确实注册系统**，但并非自己直接注册，而是经
  `init_asset_with_capacity` 的第 4 步（`install.rs:452-464`）：
  `Assets::<A>::track_assets` 进 `stages.tracking`（= `PreUpdate`），
  `Assets::<A>::asset_events` 进 `stages.event`（= `PostUpdate`）。
- 这些系统通过 `schedules.entry(...)` 挂载（`install.rs:457/459`），
  `entry` **不存在则新建**（`schedule.rs:176-180`），**不会 panic**。
  因此它们**不依赖 `physics`/`graphics` 建任何 stage**；唯一的 stage 相关依赖是
  `AssetStages` **资源**（哪个 label 算 tracking/event），而该资源由 `asset::install` 提供（`install.rs:341`）。
- 它们**都不注册进 `Extract`**。`graphics::install`/`install_assets` 才用 `get_mut(extract_stage).expect(...)`
  （`lib.rs:82-84`、`asset_events.rs:138-141`）——那是 graphics 自己依赖 `schedule::install`。
- 同一 stage 内的排序：`camera` 的控制器进 `PostUpdate` 且**未加入任何 set**
  （`camera.rs:198` 直接 `add_systems(editor_camera_controller_system)`），
  与 `AssetEventSystems` set（`install.rs:459-463`）无显式 `.before/.after`，两者读写资源不冲突，
  也无顺序歧义诉求。**无从注册先后产生的约束。**

### 3.7 `AssetServer` / `AssetStages` 存在性要求（汇总）

| 调用 | 需要 `AssetServer` | 需要 `AssetStages` | 缺失时行为 |
|---|---|---|---|
| `init_asset` / `init_asset_with_capacity` | 是（`install.rs:429`） | 是（`install.rs:453`） | `World::resource` panic（`world.rs:2285-2295`） |
| `register_asset_loader` | 是（`install.rs:469`） | 否 | 同上 panic |

---

## 4. 隐形约束：`.wgsl` 被两个 loader 认领（真顺序敏感）

这是本次核查最重要的发现，**票面与 `build_world` 注释都未提及**：

- `TextLoader::extensions()` 含 `"wgsl"`（`kairos_engine/src/kairos_editor/editor_assets/text.rs:54-56`）。
- `ShaderLoader::extensions()` = `&["wgsl"]`（`kairos_graphics/src/shader.rs:55-57`）。

当前顺序：`text::install`（`kairos_editor.rs:105`）**早于** `graphics::install_assets`
（`kairos_editor.rs:121`，`install_assets` 内 `shader::install` → `ShaderLoader` 在
`asset_events.rs:130`）。于是 `extension_to_loaders["wgsl"] = [text_idx, shader_idx]`，
**未类型化** `.wgsl` 选取 `.last()` = `ShaderLoader`。

拆分后 `text::install` 落到 `graphics::install_assets` 之后 → `extension_to_loaders["wgsl"] = [shader_idx, text_idx]`
→ 未类型化 `.wgsl` 会改为 `TextLoader`。这是**可观测的行为翻转**，只是当前**潜伏**：

- 生产代码里所有 `.wgsl` 加载都是 **typed**：`MaterialLoader` 里
  `load_context.load::<ShaderAsset>(...)`（`kairos_graphics/src/material.rs:89`），
  编辑器 shader 面板 `load::<Text>(...)`（`ui/inspector/shader.rs:67`，字段 `ShaderModel.handle: Handle<Text>` 见 `shader.rs:42`）。
  typed load 走 `find` 的类型分支，单 loader 直接命中（`loaders.rs:181-183`），与顺序无关。
- 生产代码**没有** untyped `.wgsl` 调用点：`load_untyped` 全仓仅出现在
  `AssetServer::load_folder_internal`（`kairos_asset/src/server.rs:1044`）与 `LoadBuilder` API，
  而 `load_folder` 在 `kairos_engine` 内无调用点。
- `asset_pipeline::bake_world`（`kairos_engine/src/asset_pipeline.rs:60-82`）不装编辑器项，
  因此 processor 的按扩展名解析不受影响。

**判定：`text::install` 相对 `graphics::install_assets` 是「真顺序敏感」，但当前无生产 untyped `.wgsl` 加载点，故现状不可观测。**
若要求拆分后**逐位保持**现状，这是唯一需要显式处理的一项（见 §5）。

---

## 5. 结论

### 5.1 「5 个 install 全后移到 `Engine::new()` 之后一次补装」是否安全？

**安装期的硬前置：安全。** 因为 `Engine::new()` 返回时，`build_world()` 已完整跑完，
`schedule::install`、`asset::install`、`graphics::install`、`graphics::install_assets`、
audio 三者都已就位：

- `text` / `toml` / `syntax` / `texture`：只需要 `asset::install` 的两个资源 → 满足。
- `camera`：只需要 `schedule::install` 的 `PostUpdate` + `Time` → 满足。
- 没有任何编辑器项是引擎项（physics/graphics/audio）的前置。

**唯一的顺序敏感点：`.wgsl` 的未类型化 loader 选取会从 `ShaderLoader` 翻转为 `TextLoader`**（§4）。
就当前代码库而言没有生产 untyped `.wgsl` 加载点，所以**运行结果不变**；
但这是一个语义翻转，属于必须显式承认/处理的行为变更。

> 结论口径：**「全后移」不会 panic、不会破坏 install 期前置，可判定为「安全但非中性」**——
> 安全的是依赖图，不中性的是 `.wgsl` untyped 解析。若团队要求严格保序，用 5.2 的最小方案。

### 5.2 最小可行插入点

- **位置**：`KairosEngine::new`（`kairos_editor.rs:146-160`）内，
  `let mut engine = Engine::new()?;`（`L147`）之后调用一次统一的 `kairos_editor::install(&mut engine.world)`。
  放在 `KairosGame::new(&mut engine)`（`L152`）之前或之后都可以：
  `KairosGame::new` 只加载 `Material`/`AudioAsset`/`Mesh`（`kairos_game.rs:189-202, 266-277`），
  不触碰编辑器类型，不构成约束。放在**任何编辑器 UI/`KairosEditorRuntime` 起来之前**即可。
- **若要逐位保持 `.wgsl` 现状**：最小代价是**保留 `text::install` 在 `build_world` 内、且仍在
  `graphics::install_assets` 之前**，只把 `toml` / `syntax` / `texture` / `camera` 后移。
  这是「4 个后移 + 1 个留守」的最小拆分；不必引入重复注册之类的 hack。

### 5.3 推荐的统一 `install` 顺序

若采用「全后移」，建议 `kairos_editor::install(world)` 内部按下述顺序（= 现状相对顺序，且把
`.wgsl` 决策点显式化为一处注释）：

```
1. editor_assets::text::install         // 注意：注册 TextLoader，认领 ["rs","md","txt","wgsl"]
2. editor_assets::toml::install         // TomlLoader 认领 ["toml"]
3. syntax::install                      // SyntaxHighlightSettingsLoader 认领 []（按类型解析）
4. ui::inspector::texture::install      // 仅 init_asset::<TextureEdit>()，无 loader
5. camera::install                      // 仅需 schedule::install 的 PostUpdate
```

并在 `1`（或统一 install 的代码注释）里写明：`TextLoader` 与 graphics 的 `ShaderLoader` 同时认领 `wgsl`，
在 `graphics::install_assets` 之后注册会让**未类型化** `.wgsl` 解析从 `ShaderLoader` 变为 `TextLoader`；
typed 加载不受影响。若未来引入 untyped `.wgsl` / `load_folder`，需在此显式定序。

### 5.4 拆分后需同步的非前置但会报错的点（登记）

与本问题无关、但拆分必然触发的连带修改（仅登记，非前置条件）：

- `kairos_editor/schedule/asset_test.rs` 的
  `build_world_registers_the_s2_leaf_assets`（`asset_test.rs:101-118`）断言
  `build_world` 注册 `Text`/`Toml`/`SyntaxHighlightSettings` 的 store；
  `build_world_registers_the_s6_audio_assets`（`L158-173`）同样。若这些 install 迁出 `build_world`，
  这些断言会失败，需改测 `kairos_editor::install` 而非 `build_world`。
- `build_world` 目前是**私有**函数（`kairos_editor.rs:78`），仅被 `Engine::new` 与
  同 crate 子模块测试调用；拆分 API 面时需一并考虑。

---

## 6. 验证记录

未新增/修改任何仓库文件，未建探针文件（读码为主；结论均由源码行号支撑）。实际执行的命令与结果：

| 命令 | 工作目录 | 结果 |
|---|---|---|
| `git rev-parse --short HEAD` | `KairosEngine/` | `4d827b0` |
| `cargo check --workspace --all-targets` | `KairosEngine/KairosEngine/` | `Finished dev profile … in 0.97s`（增量，0 error） |
| `cargo test-crate kairos_engine` | `KairosEngine/KairosEngine/` | `106 passed; 0 failed; 0 ignored` |
| `cargo test-crate kairos_asset` | `KairosEngine/KairosEngine/` | `239 passed; 0 failed; 0 ignored` |

未运行「拆分后的」探针——因为本票口径为只查事实、不改动仓库，且结论可由 loader 选取代码
（`server/loaders.rs:143-214`）与各 `install` 的资源读取（`asset/install.rs:429/453/469`）直接读出。
`loaders.rs` 自带的 registry 单测（`loaders.rs:251-375`）与 `kairos_asset` 239 项测试通过，
佐证了「按 name/type/extension 索引 + `reserve` 占位」的注册机理与本文一致。
