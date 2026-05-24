# Texture / Asset / Handle 设计讨论记录

> 来源：AI 辅助设计讨论，2026-05-24  
> 状态：设计草案，尚未在代码库中完整实现  
> 关联代码：`src/graphics/texture.rs`、`src/asset_loader.rs`、`src/asset_loader/texture.rs`、`src/ecs/entity.rs`

## 1. 背景

当前渲染层已经开始创建 wgpu 纹理、纹理视图、采样器和 bind group。随着引擎继续发展，纹理不会只是一份 GPU bytes，也不会只由渲染管线临时创建。更合理的方向是将纹理纳入统一资产系统：

```text
AssetLoader
    -> AssetServer / AssetDatabase
    -> CPU Assets<TextureAsset>
    -> GPU RenderAssets<GpuTexture>
    -> Material / BindGroup
```

这份记录整理本次讨论中的关键结论：CPU Texture 如何建模、TexturePool 是否应该持有 `Arc`、如何缓存查找、如何保证存储连续性、Handle 如何管理生命周期，以及 Bevy 在这一块的设计。

## 2. Texture / View / Sampler / BindGroup 是否拆开

建议拆开。

`TexturePool` 或 GPU 资源池应该只负责 GPU 资源的存储、复用和生命周期，不应该把“资源如何绑定到某个 shader”也塞进去。

建议分工：

| 对象 | 归属 | 说明 |
|------|------|------|
| `wgpu::Texture` | GPU Texture Pool / RenderAssets | 真正的 GPU 图像资源 |
| `wgpu::TextureView` | Texture View Cache | 对 texture 的访问视图，可指向整图、mip、array layer、cube face 等 |
| `wgpu::Sampler` | Sampler Cache | 采样状态，通常可以被多张纹理共享 |
| `wgpu::BindGroup` | Material / BindingSet / Render Asset | 与 bind group layout、shader、材质参数强相关，不属于纹理本身 |

原因是：同一张纹理在不同 shader 或材质中可能使用完全不同的 bind group layout。把 `BindGroup` 放进纹理池会让后续材质系统变得僵硬。

可以保留一个工程上的便利接口：创建普通 2D 纹理时，顺手创建默认 view，返回 `(TextureHandle, TextureViewHandle)`。但结构上仍然保持 texture 和 view 分离。

## 3. 缓存池如何查找

不要从 `Vec` 里线性查找。池中应该维护：

```rust
HashMap<TextureKey, TextureHandle>
Vec<GpuTexture>
```

创建前先构造稳定 key，查到就返回已有 handle，查不到才真正创建资源。

示例：

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextureKey {
    File(std::path::PathBuf),
    RenderTarget(RenderTargetKey),
    Runtime(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RenderTargetKey {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub sample_count: u32,
}
```

不同资源使用不同 key：

| 资源类型 | 推荐 key |
|----------|----------|
| 文件纹理 | 规范化后的 asset path |
| 程序生成纹理 | 显式资源名、UUID 或业务 id |
| RenderTarget | `name + width + height + format + usage + sample_count` |
| TextureView | `texture_handle + view_desc` |
| Sampler | sampler descriptor 参数 |
| BindGroup | material/layout + texture views + samplers + uniform bindings |

重要原则：查找用业务身份或描述符，不用 GPU 对象本身。

## 4. CPU Texture 不应只是 bytes

CPU 端 Texture 应该是完整资产数据，不只是 `Vec<u8>`。

建议建模：

```rust
pub struct TextureAsset {
    pub desc: TextureDesc,
    pub pixels: TexturePixels,
    pub import_settings: TextureImportSettings,
}

pub struct TextureDesc {
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub format: TextureFormat,
    pub mip_count: u32,
    pub dimension: TextureDimension,
    pub color_space: ColorSpace,
}
```

这里 `width`、`height`、`format`、`mip_count`、`dimension` 都是资产主体的一部分。应用层如果修改这些字段，缓存层、版本号、dirty 状态和 GPU 同步系统都应该感知到变化。

纹理加载入口也不应该是 `TexturePool::load(path)`，而应该是：

```text
asset_server.load::<TextureAsset>(path)
```

或引擎提供的便捷入口：

```text
asset_loader.load_texture(path)
```

Pool / Storage 只是后台实现。

## 5. `Arc<AssetCell<T>>` 与连续内存问题

如果池里直接存：

```rust
HashMap<PathBuf, Arc<AssetCell<TextureAsset>>>
```

那么每个 asset 通常都是一次独立堆分配，无法保证资产记录连续分布。

如果希望资源记录在内存中连续，应让中央资产池拥有连续数组：

```rust
pub struct Assets<T> {
    slots: Vec<AssetSlot<T>>,
    free_list: Vec<u32>,
}

pub struct AssetSlot<T> {
    generation: u32,
    version: u64,
    dirty: bool,
    asset: Option<T>,
}
```

外部不要长期持有 `&TextureAsset`，而是持有 `Handle<TextureAsset>`。

注意：`Vec<AssetSlot<TextureAsset>>` 只保证 `TextureAsset` 记录连续。如果 `TextureAsset` 内部有 `pixels: Vec<u8>`，每张图的像素数据仍然是单独堆分配。若将来真的需要像素数据也统一连续，可以改为：

```rust
pub struct TextureAsset {
    pub desc: TextureDesc,
    pub pixels: BlobRange,
}

pub struct BlobRange {
    pub offset: usize,
    pub len: usize,
}

pub struct TextureBlobArena {
    bytes: Vec<u8>,
}
```

但初期没有必要过早把所有像素放入全局 blob arena。纹理通常是大对象，真正频繁遍历的是 metadata、version、dirty、handle 关系。

## 6. 是否需要 `RwLock`

`RwLock` 是读写锁：

```rust
storage.read()   // 多个读者可同时存在
storage.write()  // 写者独占，写时不能读也不能写
```

`Arc<RwLock<T>>` 的含义是：

```text
Arc    -> 多处共享所有权
RwLock -> 共享对象内部可安全读写
T      -> 真正数据
```

如果采用“Texture 门面对象”方案，可以这样：

```rust
pub struct Texture {
    handle: Handle<TextureAsset>,
    storage: Arc<RwLock<Assets<TextureAsset>>>,
}
```

这样应用层可以调用：

```rust
texture.edit(|asset| {
    asset.desc.width = 1024;
    asset.desc.height = 1024;
});
```

内部通过 handle 找到中央 storage 中的 slot，修改后 bump version 并标记 dirty。

不过从完整 ECS 引擎架构看，更推荐 Bevy 风格：组件、材质和系统持有 `Handle<TextureAsset>`，修改资产时由系统显式拿到 `ResMut<Assets<TextureAsset>>`。这样更符合 ECS 的数据访问模型，也更容易做调度、并行和变更追踪。

## 7. 推荐的 Kairos 资产层结构

推荐采用类似 Bevy 的中央资产表模型：

```text
应用 / ECS Component / Material
    持有 Handle<TextureAsset>

AssetServer
    path cache
    loader registry
    load state
    handle drop events

Assets<TextureAsset>
    CPU 端纹理资产池
    Vec<AssetSlot<TextureAsset>>

RenderAssets<GpuTexture>
    GPU 端纹理缓存
    HashMap<AssetId<TextureAsset>, GpuTexture>

Material / PreparedMaterial
    根据 Texture Handle 和 GpuTexture 创建 BindGroup
```

组件示例：

```rust
pub struct Sprite {
    pub texture: Handle<TextureAsset>,
}
```

材质示例：

```rust
pub struct StandardMaterial {
    pub albedo: Option<Handle<TextureAsset>>,
    pub normal: Option<Handle<TextureAsset>>,
}
```

CPU 资产修改示例：

```rust
fn edit_texture(
    mut textures: ResMut<Assets<TextureAsset>>,
    handle: Res<SelectedTexture>,
) {
    if let Some(texture) = textures.get_mut(&handle.0) {
        texture.desc.width = 1024;
        texture.desc.height = 1024;
        texture.resize_pixels_for_desc();
    }
}
```

修改时 `Assets<T>::get_mut` 应该触发：

```text
version += 1
dirty = true
AssetEvent::Modified { id }
```

GPU 同步系统看到 `Modified` 或 version 变化后决定：

| CPU 变化 | GPU 动作 |
|----------|----------|
| 只变 pixels | `queue.write_texture` |
| width / height / format / mip_count / usage 变化 | 重建 `wgpu::Texture`、默认 view 和相关 bind group |
| sampler desc 变化 | 重建 sampler 或查 sampler cache |

## 8. Handle 如何管理生命周期

`Handle<T>` 不直接拥有资源数据。它引用中央资产池里的资产。

核心结构：

```rust
pub struct AssetId<T> {
    index: u32,
    generation: u32,
    _marker: std::marker::PhantomData<fn() -> T>,
}

pub struct Handle<T> {
    id: AssetId<T>,
    strong: std::sync::Arc<StrongHandle>,
}

pub struct StrongHandle {
    id: UntypedAssetId,
    drop_sender: crossbeam_channel::Sender<AssetDropEvent>,
}
```

`generation` 用于防止旧 handle 误访问复用后的 slot。比如 index 5 的纹理被删除，slot 5 后来给了另一张纹理；如果没有 generation，旧 handle 可能错误访问新纹理。

强 handle 的生命周期：

```text
Handle clone
    -> Arc strong count +1

Handle drop
    -> Arc strong count -1

最后一个 Handle drop
    -> StrongHandle::drop()
    -> 发送 AssetDropEvent::Unused(id)
```

`StrongHandle::drop` 不应该直接删除资源，只发送消息：

```rust
impl Drop for StrongHandle {
    fn drop(&mut self) {
        let _ = self.drop_sender.send(AssetDropEvent::Unused(self.id));
    }
}
```

原因：

- `drop()` 可能发生在任意系统中；
- `drop()` 可能发生在后台线程；
- `drop()` 中拿不到 `Assets<T>` 的安全可变访问；
- GPU 资源可能当前帧仍在使用，不能随意立即释放。

真正释放应由 AssetServer 或资产维护系统在安全时机集中处理：

```rust
impl AssetServer {
    pub fn process_dropped_handles(&mut self, storages: &mut AssetStorages) {
        while let Ok(event) = self.drop_receiver.try_recv() {
            match event {
                AssetDropEvent::Unused(id) => {
                    storages.remove_untyped(id);
                    self.remove_path_cache_for(id);
                    self.events.push(AssetEvent::Unused { id });
                }
            }
        }
    }
}
```

释放链路：

```text
所有组件 / 材质 / UI 不再持有 Handle
    -> 最后一个 Arc<StrongHandle> drop
    -> StrongHandle 发送 Unused(id)
    -> AssetServer 下一帧处理 drop event
    -> Assets<TextureAsset>.remove(id)
    -> AssetEvent::Unused(id)
    -> RenderAssets<GpuTexture>.remove(id)
```

重要注意：`path_cache` 不应该持有强 handle。

错误示例：

```rust
path_cache: HashMap<PathBuf, Handle<TextureAsset>>
```

这会让缓存本身一直持有强引用，资源永远不会卸载。

推荐：

```rust
path_cache: HashMap<PathBuf, AssetId<TextureAsset>>
```

或存 weak handle。强 handle 只交给真正使用资源的地方。

## 9. Bevy 的相关设计

Bevy 0.18.1 的设计可概括为：

```text
应用 / ECS Component
    存 Handle<Image>

AssetServer
    load(path)
    path cache
    load state
    hot reload

Assets<Image>
    CPU 端资产池

RenderAssets<GpuImage>
    Render World 中的 GPU 端资产缓存

Material / PreparedMaterial
    根据 Handle<Image> / GpuImage 创建 BindGroup
```

关键点：

- `Handle<A>` 是资产引用，不是资产数据本身。
- 资产数据存放在 `Assets<A>` 中，避免多份拷贝。
- 强 handle 内部使用 `Arc<StrongHandle>` 管理“是否还有人在引用该资产”。
- `Assets<A>` 对运行时 index asset 使用 dense vec-like storage；UUID asset 使用 hashmap。
- 修改资产数据时，通过 `Assets<A>::get_mut(handle)` 修改中央资产池中的数据。
- CPU `Image` 包含像素数据、texture descriptor、sampler、texture view descriptor、asset usage 等完整纹理信息。
- GPU `GpuImage` 包含 `Texture`、`TextureView`、`Sampler`、format、size、mip count。
- RenderAsset 系统监听资产事件，将新增或修改过的 CPU asset 提取到 render world，并 prepare 成 GPU asset。

参考资料：

- Bevy `Handle`: <https://docs.rs/bevy_asset/latest/bevy_asset/enum.Handle.html>
- Bevy `StrongHandle`: <https://docs.rs/bevy_asset/latest/bevy_asset/struct.StrongHandle.html>
- Bevy `Assets`: <https://docs.rs/bevy_asset/latest/bevy_asset/struct.Assets.html>
- Bevy `AssetServer`: <https://docs.rs/bevy/latest/bevy/asset/struct.AssetServer.html>
- Bevy `AssetLoader`: <https://docs.rs/bevy_asset/latest/bevy_asset/trait.AssetLoader.html>
- Bevy `Image`: <https://docs.rs/bevy_image/latest/bevy_image/struct.Image.html>
- Bevy render asset source: <https://docs.rs/crate/bevy_render/latest/source/src/render_asset.rs>
- Bevy `GpuImage`: <https://docs.rs/bevy/latest/bevy/render/texture/struct.GpuImage.html>

## 10. Kairos 的阶段性实现建议

第一阶段先实现 CPU 资产系统：

```text
Handle<T>
AssetId<T>
StrongHandle
Assets<T>
AssetEvent
AssetServer path cache
TextureAsset
TextureAssetLoader
```

第二阶段接入 GPU 纹理缓存：

```text
GpuTexture
GpuTextureView
GpuSampler
RenderAssets<GpuTexture>
TextureViewCache
SamplerCache
```

第三阶段接入材质：

```text
MaterialAsset
PreparedMaterial
BindGroup cache
Pipeline / layout compatibility
```

第四阶段再考虑：

- 异步 IO；
- 热重载；
- asset dependency graph；
- import settings / meta 文件；
- unload policy；
- GPU 上传预算；
- Render World 与 Main World 分离。

## 11. 本次讨论的核心结论

1. `Texture`、`TextureView`、`Sampler` 应拆开存；`BindGroup` 应属于材质或绑定系统。
2. 缓存池查找应使用 `HashMap<Key, Handle>`，不要线性扫描 `Vec`。
3. CPU Texture 应是完整资产数据，包含 bytes、width、height、format、mips、dimension、import settings 等。
4. Loader 是加载入口，Pool / Storage 是后台实现。
5. 若需要资产记录连续存储，采用 `Vec<AssetSlot<T>> + Handle`，不要每个 asset 一个 `Arc<AssetCell<T>>`。
6. ECS 风格下更推荐 `Handle<T> + Assets<T>`，而不是让 `Texture` 对象自己持有 `Arc<RwLock<Storage>>`。
7. 强 handle 用 `Arc<StrongHandle>` 管引用生命周期；最后一个强 handle drop 后通过 channel 通知 AssetServer。
8. `path_cache` 不应持有强 handle，否则资源无法自动卸载。
9. CPU asset 变化通过 version / dirty / AssetEvent 同步到 GPU render asset。
10. Bevy 的资产系统是一个值得参考的完整模型，但 Kairos 可以按阶段实现较小版本。

## 12. 后续补充：`Assets<T>` 应该从哪里获取

继续讨论后，一个关键问题被明确下来：如果通过 `Handle<TextureAsset>` 访问纹理，仍然需要某个 `Assets<TextureAsset>`。那么这个 `Assets<TextureAsset>` 由谁持有，应用层又从哪里拿到它？

参考 Bevy，答案不是：

```rust
handle.get_texture()
```

也不是让 `Handle<T>` 内部持有 `Arc<RwLock<Assets<T>>>`。Bevy 的设计是：

```text
Handle<T>
    只是一把 typed key / reference

Assets<T>
    是 World 里的 Resource

SystemParam: Res<Assets<T>> / ResMut<Assets<T>>
    由 scheduler 从 World 自动借出
```

也就是说，真实结构是：

```text
App
  -> World
      -> Resource: AssetServer
      -> Resource: Assets<Image>
      -> Resource: Assets<Mesh>
      -> Resource: Assets<StandardMaterial>
      -> Resource: AssetEvent<Image>
      -> Entities / Components / Systems
```

Bevy 中 `Assets<A>` 本身实现了 `Resource`，并且文档说明它 stores asset values identified by `AssetId`。因此系统中写：

```rust
fn edit_image(
    mut images: ResMut<Assets<Image>>,
    selected: Res<SelectedImage>,
) {
    if let Some(image) = images.get_mut(&selected.handle) {
        image.resize(new_size);
    }
}
```

这里的 `images` 不是用户手动传入的普通参数，而是 Bevy scheduler 运行系统时从 `World` 中借出的 `Assets<Image>` resource。

Bevy 的 ECS 文档也明确说明：system 函数参数会自动获取或修改 ECS state，前提是参数类型实现了 `SystemParam`。`Res<T>` 和 `ResMut<T>` 就是用于访问 resource 的 system param。

### 12.1 `Assets<T>` 是什么时候插入 World 的

Bevy 通过 `AssetApp::init_asset::<A>()` 注册资产类型。这个注册流程会：

- 在 `AssetServer` 注册资产类型；
- 初始化对应的 `AssetEvent<A>` resource；
- 添加相关 systems 和 resources；
- 对 `Assets<A>` resource 的调度歧义做处理。

内建资产类型，例如 `Image`、`Mesh`、`StandardMaterial`，由 Bevy 的相关插件初始化。用户自定义资产则需要在插件中调用类似：

```rust
app.init_asset::<TextureAsset>();
app.init_asset_loader::<TextureAssetLoader>();
```

映射到 Kairos，如果参考 Bevy，应该有一个资产插件或初始化阶段：

```rust
world.insert_resource(AssetServer::new());
world.insert_resource(Assets::<TextureAsset>::new());
world.insert_resource(AssetEvents::<TextureAsset>::new());
```

后续如果 Kairos 有 `App` / `Plugin` 系统，可以收敛成：

```rust
app.init_asset::<TextureAsset>();
app.init_asset_loader::<TextureAssetLoader>();
```

### 12.2 不在 system 里时如何访问

Bevy 也允许直接从 `World` 取 resource。`World` 提供：

```rust
world.resource::<Assets<Image>>();
world.resource_mut::<Assets<Image>>();
world.get_resource_mut::<Assets<Image>>();
world.insert_resource(...);
world.init_resource::<T>();
```

这通常用于：

- plugin 初始化；
- one-shot editor command；
- 独占系统；
- 测试；
- 还没有进入常规 schedule 的引擎启动流程。

对应 Kairos，可以先实现一个简化版：

```rust
let mut textures = world.resource_mut::<Assets<TextureAsset>>();

if let Some(texture) = textures.get_mut(&handle) {
    texture.resize(new_size);
}
```

当 UI 还不是 ECS system 时，不要急着发明全局单例或把 storage 塞进 handle。更贴近 Bevy 的做法是：UI 或编辑器根上下文拿到 `&mut World`，需要资产时从 `World` 取对应 resource。

因此 Kairos 的访问路径可以分两类：

```text
常规运行时逻辑：
    SystemParam -> Res<Assets<T>> / ResMut<Assets<T>>

编辑器命令 / 初始化 / 临时独占逻辑：
    &mut World -> world.resource_mut::<Assets<T>>()
```

核心结论：`Assets<T>` 不从 `Handle<T>` 获取，而从 `World` 获取；`Handle<T>` 只作为访问 `Assets<T>` 的 key。

## 13. 后续补充：单张纹理修改与 system 的边界

另一个问题是：将 `edit_texture` 作为一个 system 会不会很麻烦？因为纹理修改多数不是批量操作，而是针对某一张纹理的单次修改。

参考 Bevy，这里要区分两件事：

```text
system
    是一段可以被 scheduler 调度的逻辑

资产修改动作
    可以发生在某个较大的系统内部，也可以发生在 World 独占访问中
```

Bevy 并不会为每个“修改某一张纹理”的动作都创建一个独立 system。更常见的是：

```rust
fn texture_inspector_ui(
    mut images: ResMut<Assets<Image>>,
    selected: Res<SelectedTexture>,
) {
    if ui_changed_width {
        if let Some(image) = images.get_mut(&selected.handle) {
            image.resize(new_size);
        }
    }
}
```

这里的 system 是“Texture Inspector UI”或“Editor UI”这类功能系统，而不是 `resize_one_texture` 这种微小操作。用户点击一次按钮，只是在这个系统内部触发一次 `images.get_mut(handle)`。

如果某个修改来自编辑器命令，也可以由命令处理阶段以独占方式访问 `World`：

```rust
fn apply_editor_command(world: &mut World, command: EditorCommand) {
    match command {
        EditorCommand::ResizeTexture { handle, size } => {
            let mut textures = world.resource_mut::<Assets<TextureAsset>>();
            if let Some(texture) = textures.get_mut(&handle) {
                texture.resize(size);
            }
        }
    }
}
```

因此，Kairos 不需要把每个单次资产修改都建成一个 system。更合理的是：

```text
系统：
    承载一类功能，例如 Inspector UI、Asset Import Monitor、RenderAsset Sync

单次修改：
    在系统内部或 editor command handler 内通过 Assets<T>::get_mut(handle) 完成
```

### 13.1 修改资产数据的语义

Bevy 文档中特别强调：如果修改 `Assets<T>` 中某个 handle 指向的资产数据，那么所有使用这个 handle 的实体都会看到变化。

这和“让某个实体使用另一张纹理”是不同语义：

```text
修改组件里的 Handle<T>
    只让这个实体指向另一份资产

修改 Assets<T> 里的资产数据
    所有持有同一 handle 的实体都受影响
```

这对 Kairos 很重要。比如：

- Inspector 修改某个 `TextureAsset` 的 import setting：所有引用该 texture 的材质或 sprite 应一起更新；
- 只想让某个 sprite 换图：应该修改该 sprite component 中保存的 `Handle<TextureAsset>`；
- 程序生成纹理并希望多个对象共享：多个对象持有同一个 handle；
- 单个对象独有运行时纹理：应创建一份新的 asset，拿到新的 handle。

### 13.2 Kairos 当前阶段的落地建议

如果 Kairos 的 ECS / scheduler 还没完整实现，短期可以按 Bevy 的世界模型先建立资源访问边界：

```text
KairosEngine
    owns World

World
    Resource: AssetServer
    Resource: Assets<TextureAsset>
    Resource: AssetEvents<TextureAsset>
```

UI 或编辑器模块需要修改资产时，不直接持有 `Assets<TextureAsset>` 字段，也不通过 handle 反查 storage，而是拿 `&mut World`：

```rust
pub fn draw_texture_inspector(world: &mut World, selected: &Handle<TextureAsset>) {
    let mut textures = world.resource_mut::<Assets<TextureAsset>>();

    if let Some(texture) = textures.get_mut(selected) {
        // draw UI and mutate selected texture
    }
}
```

等 Kairos 的 scheduler 做起来，再把它收敛成 system 参数：

```rust
fn texture_inspector_ui(
    mut textures: ResMut<Assets<TextureAsset>>,
    selected: Res<SelectedTexture>,
) {
    if let Some(texture) = textures.get_mut(&selected.handle) {
        // draw UI and mutate selected texture
    }
}
```

这和 Bevy 的方向一致：`Assets<T>` 是 `World` resource，`Handle<T>` 是 key，修改发生在拿到 resource 的上下文中。

## 14. 后续补充后的结论

1. `Assets<T>` 的获取入口应该是 `World`，不是 `Handle<T>`。
2. `Res<Assets<T>>` / `ResMut<Assets<T>>` 是 scheduler 从 `World` 自动注入的 system param。
3. 不在 system 里时，可以通过 `world.resource_mut::<Assets<T>>()` 访问。
4. `init_asset::<T>()` 这类初始化负责把 `Assets<T>`、`AssetEvent<T>` 和相关系统注册进应用。
5. 单张纹理修改不必对应一个独立 system；它可以是编辑器 UI system 或 command handler 内的一次 `Assets<T>::get_mut(handle)`。
6. 修改 `Assets<T>` 中的资产数据会影响所有持有同一 handle 的对象；只想替换单个对象的资源时，应修改该对象组件里的 handle。
7. Kairos 若参考 Bevy，应优先建立 `World -> Resource: Assets<TextureAsset>` 的边界，再做 system param 自动注入。

补充参考：

- Bevy asset 模块文档：<https://docs.rs/bevy/latest/bevy/asset/index.html>
- Bevy `Assets`: <https://docs.rs/bevy_asset/latest/bevy_asset/struct.Assets.html>
- Bevy `AssetApp`: <https://docs.rs/bevy/latest/bevy/asset/trait.AssetApp.html>
- Bevy ECS system 文档：<https://docs.rs/bevy_ecs/latest/bevy_ecs/system/index.html>
- Bevy ECS `World`: <https://docs.rs/bevy_ecs/latest/bevy_ecs/world/struct.World.html>
