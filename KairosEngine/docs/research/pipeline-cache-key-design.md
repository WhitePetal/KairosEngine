# 渲染管线缓存 / PSO 缓存键设计研究

> 分析 Unity、Unreal Engine、Godot、Bevy 四款引擎的 PSO（Pipeline State Object）缓存设计。
> 重点关注：缓存键结构、纹理格式对键的影响、热重载后的清理策略、以及关键权衡。

---

## 1. Unreal Engine

### 1.1 缓存键结构

- **Topic**: 使用完全展开的 `FGraphicsPipelineStateInitializer` 结构体作为缓存键，包含所有渲染状态字段。
- **Source**: `Engine/Source/Runtime/RenderCore/Public/PipelineStateCache.h` (lines 50-180), `Engine/Source/Runtime/RenderCore/Private/PipelineStateCache.cpp`
  - GitHub: [PipelineStateCache.h](https://github.com/EpicGames/UnrealEngine/blob/5.4/Engine/Source/Runtime/RenderCore/Public/PipelineStateCache.h)
  - 官方文档: [Pipeline State Objects in UE](https://docs.unrealengine.com/5.4/en-US/overview-of-pipeline-state-objects-in-unreal-engine/)
- **Details**: 
  - 缓存键是 `FGraphicsPipelineStateInitializer`，这是一个扁平化的结构体，直接包含 VS/PS/GS/DS/HS 着色器、光栅化状态、深度模板状态、混合状态、渲染目标格式数组、MSAA 样本数等全部 PSO 参数。
  - `FPipelineStateCache` 内部使用 `TMap<FGraphicsPipelineStateInitializer, FPSOState>`（实际包装为线程安全的 `FPipelineStateCache::FPipelineStateMap`）。
  - 键的哈希通过 `GetTypeHash(FGraphicsPipelineStateInitializer)` 实现，该函数将所有成员折叠为一个哈希值。
  - 不做"按材质标识"查缓存——而是**完全展开键**。即使两个材质共享同一套着色器和状态，只要在代码层面构造了不同 initializer，就会分叉。
  - UE 也提供了 `FRenderPipelineStateInitializer` 用于低层级的 Raytracing PSO 缓存, 遵循同样模式。

### 1.2 纹理格式与绑定布局的关系

- **Topic**: 纹理格式通过着色器平台的 resource binding 层间接影响 PSO 键，但 PSO cache 本身不显式存储纹理格式。
- **Source**: 同上文件 + `Engine/Source/Runtime/RHI/Public/PipelineStateCache.h`
- **Details**:
  - 在 D3D12/Vulkan RHI 实现中，纹理格式影响 descriptor heap layout（CBV/SRV/UAV 的类型和数量），但这些属于**根签名 / pipeline layout** 的一部分，不是直接进入 PSO initializer。
  - 着色器编译时若用到纹理格式相关的 sRGB 采样等属性，会编译出不同的着色器变体（通过 `SHADER_QUALITY` 等宏），这些变体会作为独立 PSO 生成。
  - 即 `texture_filterable` 等概念不直接出现在 PSO 缓存键中，而是通过**着色器变体 + sampler state** 隐式体现。
  - 最终键中的 `FSamplerStateRHIRef` 指向 sampler state（包含 filter、address mode 等），不同 filterable 需求对应不同 sampler。

### 1.3 缓存淘汰 / 清理策略

- **Topic**: 提供显式 `Cleanup` API，不支持自动 LRU；磁盘 PSO 缓存有版本化清理。
- **Source**: `Engine/Source/Runtime/RenderCore/Private/PipelineStateCache.cpp`
- **Details**:
  - 运行时 PSO 缓存（`FPipelineStateCache::FPipelineStateMap`）**不做自动 LRU 淘汰**。游戏需要手动调用 `CleanupPipelineStateCaches()` 来清空。
  - 热重载着色器时，引擎会重新编译变体并通过 `RecreatePipelineState` 替换 cache 中的条目；旧条目在下一帧被垃圾回收（引用计数归零后释放）。
  - 磁盘 PSO 缓存（`FPipelineFileCache`）使用文件版本号 + 着色器哈希来做渐进式更新。每次启动时比对着色器哈希是否有变化，有则丢弃对应 PSO。
  - 设计上倾向于**内存泄露比渲染崩溃好**——宁可 stale 条目留在 cache 中造成轻微内存浪费，也不要因过早淘汰导致运行时重新编译造成帧率尖峰。

### 1.4 关键设计权衡

- **Topic**: 精确匹配 vs 分组；哈希碰撞风险低；不自动淘汰以保障帧率稳定性。
- **Details**:
  - 使用完全展开键：确保 cache hit 等于是完全匹配，不会因 grouping 导致状态切换开销「隐藏」。代价是 cache 条目爆炸，但对现代 PSO 数量仍可接受（一个中等游戏几千到几万个 PSO）。
  - 哈希碰撞：`GetTypeHash` 使用 FNV 风格哈希，64位空间。UE 官方文档表示碰撞概率极低，且发生碰撞时 `==` 比较会回退到 `MemCmp`，不会导致错误渲染。
  - 不做自动淘汰：这是有明显的权衡——游戏长时间运行可能累积无用 PSO（如切换关卡后）。引擎期望游戏在加载界面显式调用 `CleanupPipelineStateCaches()`。

---

## 2. Unity (URP/HDRP / Scriptable Render Pipeline)

### 2.1 缓存键结构

- **Topic**: Unity 不暴露显式的 PSO Cache API，而是通过 Shader Variant + SRP Batcher 实现间接管线缓存。
- **Source**: 
  - [Unity Shader Compilation and Shader Variants](https://docs.unity3d.com/Manual/shader-variants.html)
  - [Scriptable Render Pipeline](https://docs.unity3d.com/Manual/ScriptableRenderPipeline.html)
  - 源码参考: `Runtime/Graphics/ScriptableRenderLoop/ScriptableRenderContext.cs`
- **Details**:
  - Unity SRP 通过 `RenderPipeline.Render()` 调用 `ScriptableRenderContext.DrawRenderers()`，内部由 C++ 侧管理 `RenderStateBlock` + `ShaderPassName`。
  - 管线状态的缓存键是：`Shader` + `Pass Index` + `Shader Keywords`（即变体）+ `RenderStateBlock`（覆盖的光栅化/混合/深度状态）。
  - 对 SRP Batcher 而言，缓存粒度是 **per-material constant buffer**——键是 `Material` + `ShaderPass`，但这不涉及完整 PSO，而只是 constant buffer 的布局匹配。匹配上后会复用 GPU constant buffer 的上传路径。
  - Unity 的 `CommandBuffer` 不暴露 PSO 句柄；每次 draw call 都需要完整描述状态，由底层图形设备驱动缓存（D3D12/Vulkan/Metal 驱动层或 Unity 的 wrapper 层做去重）。

### 2.2 纹理格式与绑定布局的关系

- **Topic**: Unity 的 `texture_filterable` 概念通过 `SamplerState` 声明或自动派生来处理，不在 PSO 层面体现。
- **Source**: [Unity Sampler States](https://docs.unity3d.com/Manual/SL-SamplerStates.html)
- **Details**:
  - Unity 允许在 shader 中用 `sampler_` 前缀约定或 `SamplerState` 类型来描述采样器。
  - 纹理格式是否 filterable 由**纹理导入设置**决定（`TextureImporter` 中的 `textureType` 和 `sRGB` 设置）。
  - 当纹理格式不支持 filtering（如某些整数格式）时，Unity 在材质验证阶段输出警告，**不会**改变 PSO 的 sampler state——sampler descriptor 是独立于 texture 的资源。
  - 所以 `texture_filterable` 是纹理的属性，不影响绑定布局 / PSO 键。

### 2.3 缓存淘汰 / 清理策略

- **Topic**: Unity 不暴露手动 PSO 清理 API；热重载通过 Shader Variant 重新编译实现。
- **Source**: [Shader Variant Collections](https://docs.unity3d.com/Manual/com.unity.shader-variant-collection.html)
- **Details**:
  - Unity 编辑器热重载 shader 时，会重新编译所有变体并重建 `ShaderPass` 内部的 PSO 句柄。旧的 PSO 句柄由图形设备自动释放（如 D3D12 的引用计数）。
  - 运行时不会自动清理 stale PSO，但 Unity 的**子场景加载机制**（`LoadSceneAsync`）会触发 Graphics API 的临时资源回收。
  - 建议使用 `ShaderVariantCollection` 预编译所有变体，避免运行时因首次加载造成卡顿。

### 2.4 关键设计权衡

- **Topic**: 隐藏底层 PSO 细节以简化 API，代价是高级用户对缓存行为的控制力弱。
- **Details**:
  - Unity 选择不暴露 PSO 缓存给用户代码，而是通过驱动层 + wrapper 层自然去重。这大大简化了 API 但意味着无法手动预热/清理。
  - 对 SRP Batcher 的设计是针对材质频繁切换的优化，而不是通用 PSO 缓存替代品。
  - Shader 变体爆炸是一个已知问题——Unity 官方建议通过 `#pragma multi_compile` 和 `#pragma shader_feature` 控制变体数量。

---

## 3. Godot

### 3.1 缓存键结构

- **Topic**: Godot 的 `RenderingDevice` 使用完整的 pipeline state descriptor 作为键，涉及显式的 `RD::Pipeline*State` 对象。
- **Source**: 
  - `servers/rendering/rendering_device.h` (Godot 4.x)
  - GitHub: [rendering_device.h](https://github.com/godotengine/godot/blob/master/servers/rendering/rendering_device.h)
  - 官方文档: [RenderingDevice class reference](https://docs.godotengine.org/en/stable/classes/class_renderingdevice.html)
- **Details**:
  - Godot 的 `RenderingDevice` 是显式的 GPU 抽象层，用户明确调用 `draw_list_begin()` + `draw_list_end()`。
  - Pipeline 通过 `render_pipeline_create()` 创建，参数包括：`shader` RID、`vertex_input_state`、`fragment_input_state`、`primitive_type`、`rasterization_state`、`multisample_state`、`depth_stencil_state`、`blend_state`。
  - C++ 内部的 pipeline cache 使用基于这些描述符拼接的哈希键，类型是 `HashMap<PipelineHash, RID>`。
  - `RenderingDevice` 自动去重：同样的 full state descriptor 只会创建一次 GPU 对象，后续返回缓存的 RID。

### 3.2 纹理格式与绑定布局的关系

- **Topic**: Godot 的绑定布局是 shader reflection 驱动的，纹理格式不会改变布局。
- **Source**: `servers/rendering/renderer_rd/` (多个文件)
- **Details**:
  - Godot 在 `_uniform_set_create()` 中验证 uniform 类型与 set 布局是否匹配。纹理格式的 filterability**不**影响 uniform 的 binding layout。
  - 当纹理格式无法被采样器所需（如非 filterable 格式传给 bilinear sampler），Godot 会在内部转换格式，而不是创建新的 PSO。
  - 不同 filterability 需求对应不同的 sampler state，sampler state 是绑定到 uniform set 的独立资源，不是 PSO 的一部分。
  - 因此 **`texture_filterable` 不会影响 PSO 键**——它只影响 texture 的创建路径和 runtime 验证。

### 3.3 缓存淘汰 / 清理策略

- **Topic**: PSO 缓存随 `RenderingDevice` 生命周期存在，无自动 LRU；`shader` 资源 free 时自动清理关联 PSO。
- **Source**: `servers/rendering/rendering_device.cpp`
- **Details**:
  - `RenderingDevice` 的 pipeline cache 存在一个 `LocalHashMap` 中，key 是 pipeline hash，value 是 VkPipeline / MTLRenderPipelineState 句柄。
  - 释放 shader（`free(shader_rid)`）后，关联的所有 pipeline 被标记为 stale。下一帧 `_cleanup_stale_pipelines()` 遍历并删除。
  - 不做 LRU 或内存感知淘汰。Godot 团队在 GitHub Issues 中讨论过[https://github.com/godotengine/godot/issues/61942]，认为自动淘汰增加的复杂度不值得——游戏的 PSO 数量通常是可控的（数千级别），且频繁重新创建 PSO 对帧率影响更严重。
  - `flush()` 或 `finalize()` 操作不涉及缓存清理。

### 3.4 关键设计权衡

- **Topic**: 自动去重 + 随 shader 生命周期清理；不做 LRU。
- **Details**:
  - 自动去重（相同的 full descriptor 返回相同 RID）减少了 GPU 状态对象创建次数，但使得用户**无法**控制单个 PSO 的生命周期。
  - 不做 LRU 淘汰：Godot 选择信任大多数游戏的 PSO 数量不会内存爆炸，并且自动 LRU 可能导致在复杂场景边缘反复创建/销毁 PSO（thrashing）。
  - 纹理格式不影响 PSO：简化了设计，但也意味着无法通过改变纹理格式来触发不同的 PSO 路径——必须通过不同的 shader 实现。

---

## 4. Bevy (wgpu)

### 4.1 缓存键结构

- **Topic**: Bevy 的 `PipelineCache` 使用完整的 `RenderPipelineDescriptor` 作为键，不自动去重。
- **Source**: 
  - `crates/bevy_render/src/render_resource/pipeline_cache.rs` (Bevy main branch)
  - GitHub: [pipeline_cache.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_render/src/render_resource/pipeline_cache.rs)
  - wgpu 官方文档: [wgpu::RenderPipelineDescriptor](https://docs.rs/wgpu/latest/wgpu/struct.RenderPipelineDescriptor.html)
- **Details**:
  - Bevy 的 `PipelineCache` 使用 `Vec<CachedPipeline>` 存储，每个 `CachedPipeline` 包含完整的 `PipelineDescriptor` + `CachedPipelineState`。
  - **不自动去重**——每次 `queue_render_pipeline()` 都插入新条目，返回递增的 ID。由使用方保证不重复插入相同 pipeline。
  - 键的实际匹配发生在 wgpu 内部。wgpu 对 `RenderPipelineDescriptor` 做哈希并去重（参考 wgpu 的 `Device::create_render_pipeline` 实现）。
  - `CachedPipelineState` 有 `Queued -> Creating (Async) -> Ok/Err` 的状态转换。
  - Bind group layout 缓存独立存在：`BindGroupLayoutCache` 使用 `BindGroupLayoutDescriptor` 做自动去重；`LayoutCache` 对 `(bind_group_layout_ids, immediate_size)` 元组做去重。

### 4.2 纹理格式与绑定布局的关系

- **Topic**: wgpu 中纹理格式影响 `BindGroupLayout` 的 `TextureViewDimension` 和 `TextureSampleType`，但不直接改变 pipeline key。
- **Source**: [wgpu::BindGroupLayoutEntry](https://docs.rs/wgpu/latest/wgpu/struct.BindGroupLayoutEntry.html)
- **Details**:
  - wgpu 的 `BindGroupLayoutEntry` 需要声明 `TextureSampleType`（如 `Float`、`Sint`、`Depth`），这决定了纹理的**可采样格式种类**，但不涉及 filterability。
  - `texture_filterable` 在 wgpu 中由 `TextureFormat` 的 `Features` 决定（如 `TextureFormat::R8Unorm` 支持 filtering，`TextureFormat::R32Uint` 不支持）。
  - Bind group layout **不随纹理格式变**——它只声明 `TextureSampleType`，不绑定具体格式。格式匹配在 `create_bind_group` 时验证。
  - 因此，`texture_filterable` 不影响 PSO 键或 bind group layout。

### 4.3 缓存淘汰 / 清理策略

- **Topic**: Bevy 不做 PSO 自动淘汰；shader 热重载时重新排队 pipeline。
- **Source**: `pipeline_cache.rs`, 同上。
- **Details**:
  - Bevy 的 `PipelineCache` 不做任何自动淘汰。shader 资源通过 `Assets<Shader>` 管理。
  - 热重载 shader 时，`extract_shaders()` 监听到 `AssetEvent::Modified`，调用 `set_shader()` 将关联的 pipeline 状态重置为 `Queued`，在下一帧重新编译。
  - 旧 pipeline 的 GPU 对象在 `RenderDevice` 释放时自动回收（wgpu 的 GPU 资源有引用计数）。
  - 无 disk cache 持久化——每次启动都重新创建所有 pipeline。
  - `synchronous_pipeline_compilation` 标志控制是否同步编译（macOS/wasm 默认同步）。

### 4.4 关键设计权衡

- **Topic**: 用户完全控制缓存策略；无自动去重但 wgpu 底层做去重。
- **Details**:
  - Bevy 选择让`PipelineCache` 做一个简单的存储层，让用户和系统插件自行管理去重。这增加了灵活性（例如可以为调试目的创建多个同名 pipeline），但增加了使用方复杂度。
  - 实际上去重由 wgpu 的底层 `Device` 实现的——wgpu 会检测相同 `RenderPipelineDescriptor` 返回已有的 GPU 对象。
  - Pipeline 编译异步化：`CachedPipelineState::Creating` 状态下 task 在 AsyncComputeTaskPool 中执行，主线程不被阻塞。
  - 不支持 LRU 淘汰：Bevy 选择优先保持实现简单，避免 pipeline 被运行时淘汰后重新编译导致的帧率抖动。

---

## 综合对比

| 维度 | Unreal Engine | Unity | Godot | Bevy / wgpu |
|------|--------------|-------|-------|-------------|
| **缓存键粒度** | 完全展开 `FGraphicsPipelineStateInitializer` | Shader + Pass + Keywords + RenderState (间接) | `RD::Pipeline*State` 组成的 descriptor | `RenderPipelineDescriptor` (全描述符) |
| **自动去重** | 是 (TMap 查找) | 驱动层 + 内部 wrapper | 是 (HashMap) | Bevy 层不自动去重，wgpu 层自动 |
| **`texture_filterable` 对 PSO 键影响** | 不影响 PSO 键，通过 SamplerState 隐式体现 | 不影响 PSO 键 | 不影响 PSO 键 | 不影响 PSO 键 |
| **热重载处理** | 重编译变体 + Cleanup API | 重编译变体，旧句柄驱动释放 | free shader →标记 stale pipeline →清理 | AssetEvent → pipeline 重置为 Queued |
| **自动 LRU 淘汰** | ❌ | ❌ (无公开 API) | ❌ | ❌ |
| **磁盘缓存持久化** | ✅ `FPipelineFileCache` | ✅ `ShaderVariantCollection` + `Caching` | ❌ | ❌ |
| **核心权衡** | Cache hit 精确 vs 条目数量 | API 简化 vs 控制力弱 | 自动去重 vs 用户可控 | 异步编译 vs 同步编译 |

### 共同结论

1. **所有引擎都使用完全展开的 state descriptor 作为 PSO 缓存键**——没有引擎单纯用材质/asset ID。这样可以保证 cache hit 等同于 GPU 状态完全一致，避免因某个字段不同导致错误渲染。

2. **`texture_filterable` 不影响任何引擎的 PSO 键**——filtering 是 sampler state 的属性，sampler state 要么是 PSO 的一部分（UE/Godot 中 SRV 关联的 sampler），要么是独立绑定的资源（wgpu/Unity）。纹理格式是否支持 filter 在创建纹理（而非 PSO）时验证。

3. **没有引擎做自动 LRU 淘汰**——所有引擎都优先保证帧率稳定性，宁可保留 stale PSO，也不要运行时重新编译。清理依赖显式调用或 shader 资源的生命周期。

4. **热重载的基本模式一致**：shader 重新编译 → 关联 pipeline 标记为需要重建 → 下一帧重建。区别在于重建是同步还是异步，以及旧 GPU 对象的释放策略。
