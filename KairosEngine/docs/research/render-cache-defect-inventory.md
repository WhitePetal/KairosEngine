# 渲染模块缺陷清单 —— 资源变更下的缓存键 / 失效 / 引用 / 泄漏

- **用途**: `kairos_graphics` 渲染模块重构时的参考单。记录一次针对「纹理 / 材质 / shader / 网格发生修改、删除时，图形侧是否正确响应（更新键、清缓存、引用错误、缓存泄漏）」的代码审计结论。
- **基准**: commit `3733d6b`（`feat(kairos_graphics): diagnose skipped draws`），clean tree。
- **范围**: `kairos_graphics/src/render_pipeline.rs`、`kairos_graphics/src/asset_events.rs`、`kairos_asset` 的事件面、`kairos_engine` 的 inspector 预览与 runtime 的 `present` 调用点。
- **方法**: 静态阅读为主；跑了 `cargo test-crate kairos_graphics`（65 passed）与 `cargo check --workspace --all-targets`（干净）。**没有做真机 GPU 行为观测**——凡是「实际会不会触发 wgpu 校验错」的判断都按语义推导，标注为潜伏 / 需实测。
- **关联**: [`pipeline-cache-key-design.md`](./pipeline-cache-key-design.md)（UE / Unity / Godot / Bevy 的 PSO 缓存键竞品调研）。本单 F1 / F2 的重构方向可直接对照它的「共同结论」——各家都是用**完全展开的 state descriptor** 做键，并且**都靠 shader 资源生命周期而非 LRU** 清理。
- **已落地**: `3733d6b` 把「被跳过的 draw」和「缓存失效」变成可观测日志（见 §6）。本单其余条目**均未修**。

---

## 0. 总表

| # | 严重度 | 类型 | 一句话 | 当前是否触发 |
|---|---|---|---|---|
| F1 | 中高 | 引用错误 | `PipelineKey` 不含 render target 格式与 texture bind group layout，同键两材质会拿到「编译给另一种格式」的 pipeline | 潜伏 |
| F2 | 中 | 缓存泄漏 | `pipeline_cache` 在 render_state 编辑下单调增长，旧 pipeline 永不回收 | 可触发 |
| F3 | 中 | 错误归属 | pipeline 按 (shader, render_state) 共享，错误却按 material 记，shader 编译失败时只会标记「先编译的那个」材质 | 可触发 |
| F4 | 低 | 引用/健壮性 | `white_texture_fallback` 是硬编码产物路径 + 永久强句柄，layout ①/③ 下永不解析 | 潜伏 |
| F5 | 低 | 漏失效 | `GraphicsAssetEvents` 是世界级资源但按窗口 drain，第二个 `RenderPipeline` 永远读到空集 | 潜伏（当前单窗口） |
| N1 | 低 | 死 API | `clear_pipeline_cache` / `clear_material_error` 全仓无调用方 | 事实 |

---

## 1. F1 — `PipelineKey` 不完整，可能拿到「编译给另一种格式」的 pipeline

**类型**: 引用错误（用错 GPU 对象）。**严重度**: 中高。**当前状态**: 潜伏。

### 现象

两个材质只要 `(shader, render_state)` 相同，就共用同一条 `wgpu::RenderPipeline`。但这条 pipeline 里还**烤进了两样不在键里的东西**：

1. **render target 格式**；
2. **texture bind group layout**。

任一样在第二个使用者那里不同，第二次绑定 / 绘制就是 wgpu 校验错。

### 机制（带行号，基于 `3733d6b`）

键的定义 —— 只有 shader 身份 + render state：

- `render_pipeline.rs:54-57` — `struct PipelineKey { shader: Option<AssetId<ShaderAsset>>, render_state: RenderState }`
- `render_pipeline.rs:59-66` — `PipelineKey::from_material`，只读 `material.shader` 与 `material.render_state`

烤进去但不在键里的 (1)：render target 格式

- 调用点 `render_pipeline.rs:834-844` 把 `color_attachments[0].…view.texture().format()` 作为 `render_target_format` 传给 `create_pipeline`
- `render_pipeline.rs:1259-1263` — `ColorTargetState { format: render_target_format, … }`

烤进去但不在键里的 (2)：texture bind group layout

- `create_texture`（`render_pipeline.rs:1276` 起）为**每条贴图**新建一个 `BindGroupLayout`，其条目依赖该贴图的 format：
  - `render_pipeline.rs:1339-1340` — `TextureSampleType` / `SamplerBindingType` 都由 `texture_asset.format` 推导
  - `render_pipeline.rs:1342-1352` — `BindingType::Texture { sample_type, … }`
  - `render_pipeline.rs:1354-1358` — `BindingType::Sampler(sampler_type)`
- 这两个推导在 `texture/format.rs` 里按 format 分叉：`sample_type()`（`format.rs:1677`）、`is_filterable()`（`format.rs:1705`，`R32Float` / `Rg32Float` / `Rgba32Float` 为 `false`，其余 Float 为 `true`）
- pipeline 建立时用的是**首个**创建该键的那条贴图的 layout（`PipelineKey` 里没有贴图），后续材质用自己的 layout 建 bind group

### 触发条件

**路径 A（render target 格式）** —— Material inspector 的预览把**被检材质本体**画进自己的 pass：

- `kairos_engine/src/kairos_editor/ui/inspector/material.rs:1241-1248` — 预览颜色附件写死 `AttachmentFormat::RGBA8UNorm`
- `material.rs:1284-1288` — `command.draw(mesh_handle.clone(), self.model.material_handle.clone(), …)`，画的是被检材质本身

而场景 / Game 窗口走的是 surface 格式：

- `render_pipeline.rs:243-249` — `RenderPipeline::new` 优先选 `Rgba8Unorm`，否则退 `surface_caps.formats[0]`
- `render_pipeline.rs:996-998` — `get_frame_buffer_format` 按 `surface_config.format` 映射

于是：**只要该材质同时出现在场景里和 inspector 预览里，且两者颜色格式不同**，第二次用到的就是为另一种格式编译的 pipeline。`Rgba8Unorm` 在多数平台可用所以侥幸一致；只能给 `Bgra8*` 的平台（部分 Metal 配置）会踩。

**路径 B（texture layout）** —— 两个材质共用 (shader, render_state)，但贴图的 `TextureSampleType` / `SamplerBindingType` 不同（如一条不可过滤的 `R32Float` 与一条 `Rgba8`）。当前 PNG / `.glb` 都产出可过滤的 `Rgba8`，所以同样是潜伏。

### 影响

`set_bind_group` / draw 触发 wgpu 校验错 → 被 `push_error_scope` 捕获 → `render_pipeline.rs:1089-1091` 把该材质塞进 `error_material_indices` → 该材质退化成紫色 fallback。所以表现是「某些物体突然变紫 / 不画」，而不是崩溃——**难定位**。

### 重构参考

把键补全到「GPU 状态全展开」：至少加入 **color target format**，以及 **texture bind group layout 的身份**（或等价地：`Option<TextureSampleType>` + `Option<SamplerBindingType>` + 是否有贴图）。这正是 `pipeline-cache-key-design.md`「共同结论」第 1 条——各家都用完全展开的 descriptor，保证 cache hit ≡ GPU 状态完全一致。

⚠️ 代价：键变细 → 条目数变多。建议同时想清楚 §2 的淘汰策略，否则两个问题会叠加。

---

## 2. F2 — `pipeline_cache` 在 render_state 编辑下单调增长

**类型**: 缓存泄漏（无界增长）。**严重度**: 中。**当前状态**: 可触发。

### 现象

`pipeline_cache` 只被 **shader 变更**清（`retain(key.shader != Some(id))`），材质改 render_state 时**只清错误标记、不回收旧 pipeline**。而 `RenderState` 是键的一部分，所以每次改 render_state 都新增一条，旧条目永久驻留。

### 机制（带行号）

- 键含完整 `RenderState`：`render_pipeline.rs:54-57`
- `RenderState` 参与 `Hash`：`render_state.rs:503-513`
- 其中 `blend_mod: Option<BlendState>`（`render_state.rs:510`），`BlendComponent` = `18 种 BlendFactor × 5 种 BlendOperation`，`Custom` 的状态空间极大（`render_state.rs:119-137`、`219-225`）
- 失效路径 `/`：`render_pipeline.rs:1108` 起的 `invalidate_caches` 里，`Material` 的 Modified / Removed 只做 `error_material_indices.remove(&id)`，**不碰 `pipeline_cache`**
- 编辑入口：`kairos_engine/src/kairos_editor/ui/inspector/material.rs:1385` — `change_render_state`，每次调用都 `get_mut` 材质 → Modified → 新键 → 编译新 pipeline
- 唯一的全清 API `render_pipeline.rs:1172` `clear_pipeline_cache` —— 全仓**无调用方**

### 影响

`wgpu::RenderPipeline` 是重对象。交互式拖 blend / depth / cull / topology 会让 `pipeline_cache` 单调上涨且不回落，长会话可能攒出可观显存与条目数。

注：`texture_cache`（按 `AssetId<Texture>`）与 `mesh_buffer_cache`（按 `AssetId<Mesh>`）**不是**这个问题——它们由 `Removed` 界定，见 §7。

### 重构参考

三条路，权衡不同：

1. **键里放的 render_state 收窄**（例如 blend 只放 `BlendPreset` 而不是展开的 `BlendState`）——治本但改变语义；
2. **加显式淘汰**（LRU 或「同一 key 的旧版本在材质变更后回收」）。注意 `pipeline-cache-key-design.md` 「共同结论」第 3 条：UE / Unity / Godot / Bevy **都不做自动 LRU**，优先保帧率稳定，宁留 stale PSO。这条要作为反例权重看待；
3. **在编辑动作里回收**（`change_render_state` / `discard_changes` 时按旧键删条目）——最贴近现有语义，改动最小。

---

## 3. F3 — 错误归属粒度与 pipeline 共享粒度不一致

**类型**: 诊断 / 行为错误。**严重度**: 中。**当前状态**: 可触发（需 shader 编译失败）。

### 机制

- pipeline 的共享粒度是 **(shader, render_state)**：`render_pipeline.rs:54-57`、`render_pipeline.rs:824-826`（`pipeline_cache.entry(pipeline_key)`）
- 错误的记录粒度却是 **material**：`render_pipeline.rs:199`（`error_material_indices: HashSet<AssetId<Material>>`）、`render_pipeline.rs:1089-1091`
- 错误是从 **error scope** 里「pop 出来的那一个材质」观测到的：`render_pipeline.rs:830-833` 在 Vacant 分支 push scope，`render_pipeline.rs:564-566` pop 后 `log::error!` + insert

### 影响

材质 A、B 共用 shader 且该 shader 编译失败时：只有**先触发编译的那个**（A）被标记 errored 并走紫色 fallback，B 继续使用那条坏 pipeline，且不产生任何错误上报。

好消息：这类错误是**可自愈**的——修好 shader 后 `Modified` 会整体 `retain` 掉旧 pipeline（`render_pipeline.rs:1108` 起），下次重编译。但错误上报会**指错人**，排障时容易查错方向。

### 重构参考

让错误归属与共享粒度对齐：把「pipeline 编译失败」记在 **PipelineKey**（或 shader id）上，material 只是引用者。这样也不会出现「A 显示了紫色、B 没有」的不一致观感。

---

## 4. F4 — `white_texture_fallback` 是硬编码产物路径 + 永久强句柄

**类型**: 引用 / 健壮性。**严重度**: 低。**当前状态**: 潜伏（文件存在）。

### 机制

- `render_pipeline.rs:51` — `const PATH_WHITE_TEXTURE: &str = "imported_assets/Default/res/textures/white.png";`
- `render_pipeline.rs:314-316` — `RenderPipeline::new` 里 `load::<Texture>(…)`，句柄随 pipeline **永久存活**（强句柄，故意的：保证寿命覆盖所有帧）
- 该路径指向 processor 的**产物**（issue #239），在 layout ①（`AssetMode::Unprocessed`）/ ③（读源码侧）下不存在
- 解析失败时的表现：`render_pipeline.rs:786-794` — `textures.get(texture_handle)` 为 `None` → 跳过该 draw

### 影响

所有 `material.texture == None` 的材质（应当用白图）会**静默不画**。`3733d6b` 之后至少会打一条 `FallbackTexture` 警告，但仍是「整个物体不见了」。

### 重构参考

fallback 不该依赖产物路径。候选：随相机 / gizmo 一样的**内置资源**（embedded / `load_internal_asset`），或按 layout 解析「产物根 vs 源码根」，或提供一张程序生成的 1×1 白图（`create_purple_fallback`（`render_pipeline.rs:1008` 起）已经是同一手法，白图可以直接复用该模式）。

---

## 5. F5 — `GraphicsAssetEvents` 是世界级资源，却按窗口 drain

**类型**: 漏失效。**严重度**: 低。**当前状态**: 潜伏（当前只有一个 `RenderPipeline`）。

### 机制

- 事件集是世界级资源：`asset_events.rs:31-34`（`GraphicsAssetEvents { inner: Arc<Mutex<…>> }`）
- 采集器在 `Extract` 阶段攒事件：`asset_events.rs:63-116`
- `present` 里 `invalidate_caches` **drain** 它：`render_pipeline.rs:349-353`、`1108` 起

### 影响

再接一个 `RenderPipeline`（第二个窗口）时，第一个窗口的 `present` 会把集合抽干，第二个窗口永远读到空集、缓存永不失效 → 用旧 bind group / 旧 pipeline 画新资源。当前 runtime 只有一个 pipeline（`kairos_engine/src/kairos_editor/runtime.rs`，`render_pipeline: Arc<Mutex<Option<RenderPipeline>>>`），所以没炸。

### 重构参考

两个方向：把 drain 改成「读后不清、按帧号裁剪」，或给每个 `RenderPipeline` 一份自己的 `GraphicsAssetEvents`（采集器写进所有 pipeline）。多窗口落地前必须处理。

---

## 6. 附带观察（不是缺陷，但重构时要知道）

**N1 — 死公有 API。** `clear_pipeline_cache`（`render_pipeline.rs:1172`）与 `clear_material_error`（`render_pipeline.rs:1089-1091`）全仓无调用方。前者是 §2 的天然出口，若决定不做自动淘汰，应在 `change_render_state` 一类编辑动作里接上。

**N2 — `Modified` 由 `get_mut` 解引用触发。** `kairos_asset/src/assets.rs`（`AssetMutChangeNotifier::drop`，`assets.rs:662` 一带）——所以任何**每帧** `get_mut` 一个 mesh / texture 的系统，都会每帧触发 `Modified` → 每帧重建 GPU buffer。当前没有这种调用点（只有 Material inspector 在**用户操作时**改材质：`material.rs` 的 `change_shader` / `change_render_state` / `drop_texture` / `clear_texture` / `discard_changes`），所以现状安全。重构时若引入「每帧向 mesh 写入」的机制（如顶点动画、地形 LOD 流式），必须改用 `get_mut_untracked` 或另设变更通道。

**N3 — `present` 只在 surface 成功时跑（这是正确行为）。** `kairos_engine/src/kairos_editor/runtime.rs` 里 `get_window_surface()` 返回 `Occluded` / `Timeout` 时不调用 `present` → 当帧不失效。因为采集器每帧照跑、事件攒在 `HashSet` 里，恢复后会一次性全部失效，**不会丢**。这解释了「窗口被遮住再回来」为什么不会画错。重构时不要把「采集」和「失效」合并成一个只在 present 里跑的步骤。

**N4 — `3733d6b` 加的观测手段（重构时的现有基线）。**

- `render_pipeline.rs:97-121` — `UnreadyAsset` 枚举：`Mesh` / `Material` / `MaterialWithoutShader` / `Shader` / `Texture` / `FallbackTexture`
- `render_pipeline.rs:159-174` — `report_unready`：每个资源只 warn 一次，带 `AssetServer::get_path` 查出的路径
- 6 个调用点：`render_pipeline.rs:732` / `740` / `756` / `764` / `786` 各处
- `render_pipeline.rs:1157-1161` — `cache_sizes`，配合 `invalidate_caches` 末尾的聚合 `log::debug!`（`textures A→B, pipelines A→B, meshes A→B`）
- 失效后 `unready_assets` 里的报告会被清掉，所以「又坏一次」会再报

---

## 7. 现状正确的部分（重构时不要破坏）

| # | 结论 | 依据 |
|---|---|---|
| 1 | 阶段顺序保证采集器读到**当帧**事件 | `AssetStages { tracking: PreUpdate, event: PostUpdate, startup: Startup }`（`kairos_engine/src/kairos_editor/schedule/asset_test.rs:38-48`）；子阶段序 `First → PreUpdate → RunFixedMainLoop → Update → PostUpdate → Extract → Last`（`schedule/test.rs:36-44`）；采集器在 `Extract`（`asset_events.rs:129-152`） |
| 2 | `Removed` 一定发出，事件不漏 | `Assets::remove`（`assets.rs:423-431`）、`Assets::remove_dropped`（`assets.rs:447-470`，仅在值真的存在时补发）。`Unused` 总在 `Removed` 前，采集器忽略 `Unused`（`asset_events.rs:58-62`） |
| 3 | 槽位回收不会命中旧缓存键 | `AssetId` 带 generation，`remove_dropped` 里 `allocator.recycle(index)` 递增 generation；`render_pipeline.rs:349-353` 的注释写明这点 |
| 4 | 帧内被引用的资源不会被移除 | `DrawCommand`（`drawer.rs:39-49`）、`InstancingRenderer`（`graphics_node.rs:66-70`）、`MaterialComponent` / `LODMesh` 都持**强**句柄 → `Removed` 不可能插进正在绘制的一帧 |
| 5 | 无跨帧堆积 | `GraphicsCommand` / `RenderPassNode` / `draw_instances` 每帧重建（`graphics_command.rs:87-98`、`graph.rs:296-331`） |
| 6 | `texture_cache` / `mesh_buffer_cache` 被 `Removed` 界定 | `render_pipeline.rs:1108` 起的 `invalidate_caches` 对 Modified+Removed 一视同仁地 `remove` |
| 7 | egui 纹理不泄漏 | `EguiTextureHandle::Drop` → 通道 → `present` 里 `free_texture`（`egui_texture_handle.rs:18-22`） |
| 8 | 材质改贴图复用 pipeline 是**正确**设计 | 贴图走 bind group，不参与键；只有 shader / render_state 需要重建 |

---

## 8. 验证与遗留

**已验证**

- `cargo test-crate kairos_graphics` → 65 passed
- `cargo check --workspace --all-targets` → 干净
- `kairos_graphics/src/render_pipeline.rs` 编辑器诊断无 error / warning

**遗留 / 存疑（重构前最好补上）**

1. **F1 未实测。** 「路径 A / B 真的会触发 wgpu 校验错」是从 wgpu 语义推导的（pipeline layout 的 bind group layout 必须与 `set_bind_group` 的兼容；`ColorTargetState.format` 必须与附件格式一致），**没有真机验证过**。验证成本低：临时把某个预览附件的 `AttachmentFormat` 改成与 surface 不同，或加载一张 `R32Float` 贴图给共享 shader 的第二个材质，看是否落进 `error_material_indices`。
2. **日志没有自动化测试。** `report_unready` 的关键行为是「往 `HashSet` 插一次」，而唯一可断言的稳定输出要抓 log 后端；`invalidate_caches` 整条路径需要真 GPU device，纯单测起不来。
3. **事件集只在 `present` 成功时被 drain。** `GraphicsAssetEventSets` 的 8 个 `HashSet`（`asset_events.rs:38-47`）每次 `invalidate_caches` 都会被清空，所以常态不累积；窗口被遮挡 / surface 超时时 `present` 不跑，集合会在那段时间内随「变更过的不同 id」增长，恢复后一次性清空。这不是泄漏，但意味着**遮挡时长 × 资源变更速率**会短暂抬高内存。
