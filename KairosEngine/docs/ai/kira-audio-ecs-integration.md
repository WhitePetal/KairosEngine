# kira 音频系统与 KairosEngine ECS 集成方案

> 日期：2026-06-20
> 基于：kira 0.12.1 + KairosEngine 现有 ECS 架构

---

## 目录

1. [现有架构回顾](#1-现有架构回顾)
2. [整体架构分层](#2-整体架构分层)
3. [ECS 组件设计](#3-ecs-组件设计)
4. [资产管理集成](#4-资产管理集成)
5. [音频引擎核心](#5-音频引擎核心)
6. [System 设计](#6-system-设计)
7. [KairosGame 最终形态](#7-kairosgame-最终形态)
8. [使用示例](#8-使用示例)
9. [线程安全分析](#9-线程安全分析)
10. [Cargo.toml 变更](#10-cargotoml-变更)
11. [文件结构](#11-文件结构)
12. [扩展方向](#12-扩展方向)

---

## 1. 现有架构回顾

### ECS 核心

```rust
// kairos_engine/src/ecs/component.rs
pub trait Component: 'static {}

// kairos_engine/src/ecs/world.rs
pub struct World {
    pub time: Time,
    entities: EntityStorage,
    entity_datas: SparseSet<Entity, EntityData>,
    table_graph: TableGraph,
    pub assets_server: AssetsServer,
    // ...
}

// 关键 API:
world.spawn((components...))                    // 创建实体
world.spawn_batch(iter)                         // 批量创建
world.query_mut::<&mut T>()                     // 可变查询
world.query::<&T>()                             // 只读查询
world.query_one_mut::<Q>(&entity)               // 单实体可变查询
world.insert(entity, components)                // 动态添加组件
world.remove::<T>(entity)                       // 移除组件
world.despawn(entity)                           // 销毁实体
world.contains(&entity)                         // 检查实体是否存在
```

### 现有组件

```rust
// kairos_engine/src/base_components/transform_component.rs
pub struct TransformComponent {
    pub position: float3,     // 世界坐标
    pub rotation: quaternion, // 旋转四元数
    pub scale: float3,        // 缩放
}
impl Component for TransformComponent {}
```

### 资产系统

```rust
// kairos_engine/src/asset_loader/assets.rs
pub struct AssetsServer {
    handlers: TypeIdMap<Box<dyn AssetsHandler>>,
    // ... tokio channel based async loading
}

// 资产系统 trait
pub trait AssetsSystem: AssetsHandler {
    type AssetType;
    type LoadedEvent: LoadedEvent<Self::AssetType>;
    type DropEvent: DropEvent;
    type Loader: AssetLoader<Self::LoadedEvent>;
}

// 使用方式：
world.assets_server.push(MyAssetsSystem::new());
let handle = world.assets_server.load::<MyAssetsSystem>(path);
let data = world.assets_server.get::<MyAssetsSystem>(&handle);
```

### 游戏循环

```rust
// kairos_engine/src/kairos_game.rs
pub struct KairosGame {}

impl KairosGame {
    pub fn new(world: &mut World) -> Self { ... }
    pub fn update(&self, world: &mut World) { ... }
    pub fn render(&self, world: &mut World, graphics_command: &mut GraphicsCommand) { ... }
}
```

---

## 2. 整体架构分层

```
┌─────────────────────────────────────────────────────────────┐
│                       KairosGame                            │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              AudioEngine (资源层)                      │  │
│  │  - kira::AudioManager<DefaultBackend>                 │  │
│  │  - emitter_map: HashMap<Entity, EmitterHandle>        │  │
│  │  - listener_handle: ListenerHandle                    │  │
│  │  - master_track, sfx_track, music_track               │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │               ECS 组件层                               │  │
│  │  AudioSource       AudioListener    AudioGlobalSettings │  │
│  │  (发声实体)         (耳朵/摄像机)    (全局音量控制)      │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              SoundAssetsSystem                         │  │
│  │  遵循 AssetsSystem trait, 加载 StaticSoundData         │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                 │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Audio Systems (update 阶段)               │  │
│  │  1. spatial_sync: Transform → kira emitter/listener    │  │
│  │  2. playback_control: 处理 AudioSource 状态转换         │  │
│  │  3. cleanup: 清理已销毁实体的 emitter                   │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

**分层原则：**

| 层 | 职责 | 存放位置 |
|---|---|---|
| 资源层 `AudioEngine` | 持有 kira 管理器、轨道、emitter/ listener 映射 | `KairosGame.audio_engine` |
| 组件层 | 纯数据标记，实现 `Component` | `World` 中的 Entity |
| 资产层 `SoundAssetsSystem` | 异步加载音频文件，遵循 `AssetsSystem` | `World.assets_server` |
| 系统层 | 每帧执行，桥接 ECS 和 kira | `KairosGame::update()` 中调用 |

---

## 3. ECS 组件设计

### 3.1 AudioSource — 挂载到发声实体

```rust
// kairos_engine/src/audio/components.rs

use crate::ecs::component::Component;
use crate::audio::sound_asset::SoundAssetsSystem;
use crate::asset_loader::assets::AssetHandle;
use std::sync::Arc;

/// 挂在需要发出声音的实体上
///
/// # 使用示例
/// ```ignore
/// let gun_sound = world.assets_server.load::<SoundAssetsSystem>("res/audio/gunshot.ogg");
/// world.spawn((
///     TransformComponent::new(...),
///     AudioSource::new(gun_sound).with_volume(0.8),
/// ));
/// ```
pub struct AudioSource {
    /// 音频资产引用（通过 AssetsServer 加载）
    pub sound_handle: Arc<AssetHandle<SoundAssetsSystem>>,
    /// 是否循环播放
    pub looping: bool,
    /// 音量倍率 (0.0 ~ 1.0)
    pub volume: f64,
    /// 播放速率 (0.5 ~ 2.0，1.0 为原速)
    pub playback_rate: f64,
    /// 当前播放状态
    pub state: AudioSourceState,
}

impl Component for AudioSource {}

/// AudioSource 的播放状态机
pub enum AudioSourceState {
    /// 停止状态（初始状态）
    Stopped,
    /// 正在播放
    Playing {
        /// 标记是否刚被触发（由系统在本帧拾取并创建 kira handle）
        just_started: bool,
    },
    /// 暂停中
    Paused,
}

impl AudioSource {
    /// 创建新的 AudioSource
    pub fn new(sound_handle: Arc<AssetHandle<SoundAssetsSystem>>) -> Self {
        Self {
            sound_handle,
            looping: false,
            volume: 1.0,
            playback_rate: 1.0,
            state: AudioSourceState::Stopped,
        }
    }

    /// 设置为循环播放
    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    /// 设置音量倍率
    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    /// 设置播放速率
    pub fn with_playback_rate(mut self, rate: f64) -> Self {
        self.playback_rate = rate.clamp(0.25, 4.0);
        self
    }

    /// 触发播放（下一帧由系统拾取）
    pub fn play(&mut self) {
        self.state = AudioSourceState::Playing { just_started: true };
    }

    /// 停止播放
    pub fn stop(&mut self) {
        self.state = AudioSourceState::Stopped;
    }

    /// 暂停播放
    pub fn pause(&mut self) {
        if matches!(self.state, AudioSourceState::Playing { .. }) {
            self.state = AudioSourceState::Paused;
        }
    }

    /// 恢复播放
    pub fn resume(&mut self) {
        if matches!(self.state, AudioSourceState::Paused) {
            self.state = AudioSourceState::Playing { just_started: true };
        }
    }
}
```

### 3.2 AudioListener — 挂载到摄像机/玩家实体

```rust
/// 挂到玩家/摄像机实体上，作为空间音频的"耳朵"
///
/// 通常一个场景只有一个实体持有此组件。
/// 空间音频系统会将此实体的 Transform 同步到 kira Listener。
///
/// # 使用示例
/// ```ignore
/// let camera = world.spawn((
///     TransformComponent::new(float3(0.0, 1.7, 5.0), quaternion::identity(), float3(1.0, 1.0, 1.0)),
///     AudioListener,
/// ));
/// ```
pub struct AudioListener;

impl Component for AudioListener {}
```

### 3.3 AudioGlobalSettings — 全局音频配置

```rust
/// 全局音频设置，作为场景中的单例组件
///
/// 可挂在一个常驻实体上，或通过 `world.query_mut::<&mut AudioGlobalSettings>()` 访问。
pub struct AudioGlobalSettings {
    /// 主音量 (0.0 ~ 1.0)
    pub master_volume: f64,
    /// 音效音量
    pub sfx_volume: f64,
    /// 音乐音量
    pub music_volume: f64,
    /// 是否静音
    pub muted: bool,
}

impl Component for AudioGlobalSettings {}

impl Default for AudioGlobalSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            music_volume: 1.0,
            muted: false,
        }
    }
}
```

---

## 4. 资产管理集成

### 4.1 SoundAssetsSystem

```rust
// kairos_engine/src/audio/sound_asset.rs

use std::path::PathBuf;

use kira::sound::static_sound::StaticSoundData;
use tokio::sync::mpsc;

use crate::asset_loader::assets::{
    asset::AssetIndex,
    Assets, AssetsSystem, AssetsHandler,
    AssetLoader, DropEvent, LoadedEvent,
};
use crate::asset_loader::assets::DependencyLoadRequestEvent;

// ──── 事件类型 ────

/// 音频加载完成事件
pub struct SoundLoadedEvent {
    index: AssetIndex,
    data: StaticSoundData,
}

impl LoadedEvent<StaticSoundData> for SoundLoadedEvent {
    fn new(index: AssetIndex, asset: StaticSoundData) -> Self {
        Self {
            index,
            data: asset,
        }
    }

    fn get_index(&self) -> AssetIndex {
        self.index
    }

    fn get_asset(self) -> StaticSoundData {
        self.data
    }
}

/// 音频资产释放事件
pub struct SoundDropEvent {
    index: AssetIndex,
}

impl DropEvent for SoundDropEvent {
    fn new(index: AssetIndex) -> Self {
        Self { index }
    }

    fn get_index(&self) -> AssetIndex {
        self.index
    }
}

// ──── AssetLoader 实现 ────

/// 音频文件加载器（异步）
pub struct SoundAssetLoader;

impl AssetLoader<SoundLoadedEvent> for SoundAssetLoader {
    fn load_asset(
        &self,
        path: PathBuf,
        asset_index: AssetIndex,
        loaded_sender: mpsc::Sender<SoundLoadedEvent>,
        _dependency_request_sender: mpsc::Sender<DependencyLoadRequestEvent>,
    ) {
        tokio::spawn(async move {
            match StaticSoundData::from_file(&path) {
                Ok(data) => {
                    let _ = loaded_sender
                        .send(SoundLoadedEvent::new(asset_index, data))
                        .await;
                }
                Err(e) => {
                    log::error!("Failed to load sound asset {:?}: {}", path, e);
                }
            }
        });
    }
}

// ──── AssetsSystem 实现 ────

/// 音频资产系统
///
/// 遵循引擎现有的 `AssetsSystem` trait，与 `AssetsServer` 无缝集成。
/// 内部使用 `StaticSoundData`，它基于 `Arc` 共享底层数据，
/// clone 不消耗额外内存（kira 文档保证）。
pub struct SoundAssetsSystem {
    assets: Assets<Self>,
}

impl SoundAssetsSystem {
    pub fn new() -> Self {
        Self {
            assets: Assets::new(
                SoundAssetLoader,
                256,  // capacity: 最多同时加载 256 个音频资源
                64,   // loaded_channel_buffer_size
                128,  // drop_channel_buffer_size
            ),
        }
    }
}

impl AssetsHandler for SoundAssetsSystem {
    fn handle_receves(&mut self) {
        self.assets.handle_receves();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl AssetsSystem for SoundAssetsSystem {
    type AssetType = StaticSoundData;
    type LoadedEvent = SoundLoadedEvent;
    type DropEvent = SoundDropEvent;
    type Loader = SoundAssetLoader;

    fn get_assets(&self) -> &Assets<Self> {
        &self.assets
    }
    fn get_assets_mut(&mut self) -> &mut Assets<Self> {
        &mut self.assets
    }
}
```

### 4.2 关键设计点

- **`StaticSoundData` 内部使用 `Arc`**：kira 文档明确说 clone `StaticSoundData` 不消耗额外内存。与引擎现有的 `Arc<AssetHandle<T>>` 模式一致。
- **异步加载**：通过 `tokio::spawn` 在线程池中执行文件 I/O 和解码，不阻塞主线程。
- **错误处理**：加载失败时记录日志（assets 系统后续可扩展 `Entry::Failed` 状态）。
- **生命周期管理**：通过 `Arc<AssetHandle>` 的引用计数自动管理资源释放。

---

## 5. 音频引擎核心

### 5.1 AudioEngine

```rust
// kairos_engine/src/audio/audio_engine.rs

use std::collections::HashMap;

use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    sound::static_sound::StaticSoundHandle,
    spatial::{
        emitter::{EmitterHandle, EmitterSettings},
        listener::{ListenerHandle, ListenerSettings},
    },
    track::{TrackBuilder, TrackHandle},
};

use crate::ecs::entity::Entity;

/// 音频引擎核心
///
/// 持有 kira 的 AudioManager 以及所有与 ECS 的桥接状态。
/// 放在 `KairosGame` 上而不是 `World` 中，便于独占访问和生命周期管理。
pub struct AudioEngine {
    /// kira 音频管理器（内部有自己的音频线程）
    pub manager: AudioManager<DefaultBackend>,

    /// Entity → EmitterHandle 映射（空间音频发射器）
    pub emitters: HashMap<Entity, EmitterHandle>,

    /// 全局唯一的空间音频监听器（耳朵位置）
    pub listener: ListenerHandle,

    /// 活跃的播放实例 (需要持续控制的 handle)
    active_handles: HashMap<Entity, StaticSoundHandle>,

    /// 混音总线轨道
    pub main_track: TrackHandle,
    pub sfx_track: TrackHandle,
    pub music_track: TrackHandle,
}

impl AudioEngine {
    /// 创建并初始化音频引擎
    pub fn new() -> Result<Self, kira::manager::error::AudioManagerError> {
        let mut manager = AudioManager::<DefaultBackend>::new(
            AudioManagerSettings::default(),
        )?;

        // 创建分类轨道（可用于独立音量控制）
        let sfx_track = manager.add_sub_track(TrackBuilder::new())?;
        let music_track = manager.add_sub_track(TrackBuilder::new())?;

        // 创建空间音频监听器
        let listener = manager.add_listener(
            [0.0, 0.0, 0.0],
            ListenerSettings::default(),
        )?;

        Ok(Self {
            main_track: manager.main_track(),
            manager,
            emitters: HashMap::new(),
            listener,
            active_handles: HashMap::new(),
            sfx_track,
            music_track,
        })
    }

    /// 为实体注册空间音频发射器
    pub fn register_emitter(&mut self, entity: &Entity) -> Result<EmitterHandle, kira::manager::error::AddEmitterError> {
        if let std::collections::hash_map::Entry::Vacant(e) = self.emitters.entry(entity.clone()) {
            let emitter = self.manager.add_emitter(
                [0.0, 0.0, 0.0],
                EmitterSettings::default(),
            )?;
            e.insert(emitter.clone());
            Ok(emitter)
        } else {
            Ok(self.emitters[entity].clone())
        }
    }

    /// 注销实体的空间音频发射器
    pub fn unregister_emitter(&mut self, entity: &Entity) {
        self.emitters.remove(entity);
        self.active_handles.remove(entity);
    }

    /// 存储活跃的播放 handle
    pub fn store_handle(&mut self, entity: &Entity, handle: StaticSoundHandle) {
        self.active_handles.insert(entity.clone(), handle);
    }

    /// 获取活跃的播放 handle
    pub fn get_handle(&self, entity: &Entity) -> Option<&StaticSoundHandle> {
        self.active_handles.get(entity)
    }

    /// 获取活跃的播放 handle（可变）
    pub fn get_handle_mut(&mut self, entity: &Entity) -> Option<&mut StaticSoundHandle> {
        self.active_handles.get_mut(entity)
    }
}
```

### 5.2 DefaultBackend 说明

`kira::DefaultBackend` 内部使用 `cpal` 作为跨平台音频 I/O 后端：
- **macOS**: CoreAudio
- **Linux**: ALSA / PulseAudio
- **Windows**: WASAPI
- **WASM**: Web Audio API（有限支持）

无需额外配置，开箱即用。

---

## 6. System 设计

### 6.1 系统调用入口

所有音频系统作为自由函数，在 `KairosGame::update()` 中按顺序调用：

```rust
impl KairosGame {
    pub fn update(&mut self, world: &mut World) {
        world.time.update();

        // ===== 音频系统管线（按顺序执行） =====

        // 1. 同步空间音频：Transform → kira emitter/listener
        audio_spatial_sync_system(world, &mut self.audio_engine);

        // 2. 处理 AudioSource 状态机：play/stop/pause
        audio_playback_system(world, &mut self.audio_engine);

        // 3. 清理已销毁实体残留的 emitter
        audio_cleanup_system(world, &mut self.audio_engine);

        // 4. 同步全局音量设置
        audio_settings_sync_system(world, &mut self.audio_engine);

        // ===== 其他游戏逻辑 =====
        // ...
    }
}
```

### 6.2 System 1: 空间音频同步

```rust
/// 每帧将 ECS Transform 同步到 kira Emitter/Listener
///
/// - AudioListener → kira Listener（耳朵位置）
/// - AudioSource + Transform → kira Emitter（声源位置）
fn audio_spatial_sync_system(world: &World, engine: &mut AudioEngine) {
    use kira::tween::Tween;

    // ─── 更新 Listener（跟随摄像机/玩家） ───
    let listeners = world.query::<(&TransformComponent, &AudioListener)>();
    if let Some((transform, _)) = listeners.into_iter().next() {
        let pos = transform.position;
        engine.listener.set_position(
            [pos.x() as f64, pos.y() as f64, pos.z() as f64],
            Tween::default(),
        );
        // 未来可扩展：同步 listener 朝向到 rotation
    }

    // ─── 更新所有 Emitter ───
    let emitters = world.query::<(&TransformComponent, &AudioSource)>();
    for (entity_ref, (transform, _source)) in emitters.entities() {
        if let Some(emitter) = engine.emitters.get_mut(&entity_ref.entity) {
            let pos = transform.position;
            emitter.set_position(
                [pos.x() as f64, pos.y() as f64, pos.z() as f64],
                Tween::default(),
            );
        }
    }
}
```

### 6.3 System 2: 播放控制

```rust
use kira::sound::static_sound::StaticSoundData;
use kira::tween::Tween;

/// 处理 AudioSource 状态机转换
///
/// - Stopped → Playing: 调用 kira 播放，创建 handle
/// - Playing → Stopped: 停止播放（drop handle）
/// - Playing → Paused: 暂停（通过 handle 控制）
/// - Paused → Playing: 恢复播放
fn audio_playback_system(world: &mut World, engine: &mut AudioEngine) {
    use crate::audio::components::{AudioSource, AudioSourceState};

    // 收集当前帧需要处理的实体（避免 borrow 冲突）
    let mut play_requests: Vec<(Entity, StaticSoundData, f64, f64, bool)> = Vec::new();
    let mut stop_requests: Vec<Entity> = Vec::new();

    // Phase 1: 读取状态，收集请求
    {
        let query = world.query::<&AudioSource>();
        for (entity_ref, source) in query.entities() {
            match &source.state {
                AudioSourceState::Playing { just_started: true } => {
                    if let Some(sound_data) = world.assets_server
                        .get::<crate::audio::sound_asset::SoundAssetsSystem>(&source.sound_handle)
                    {
                        play_requests.push((
                            entity_ref.entity.clone(),
                            sound_data.clone(),
                            source.volume,
                            source.playback_rate,
                            source.looping,
                        ));
                    }
                }
                AudioSourceState::Stopped => {
                    // 如果之前有活跃 handle，需要停止
                    if engine.get_handle(&entity_ref.entity).is_some() {
                        stop_requests.push(entity_ref.entity.clone());
                    }
                }
                _ => {}
            }
        }
    }

    // Phase 2: 执行 kira 操作（需要 &mut World 和 &mut AudioEngine）
    for (entity, sound_data, volume, rate, looping) in play_requests {
        let mut data = sound_data;
        if looping {
            data = data.loop_region(..);
        }
        data = data.output_destination(&engine.sfx_track);

        match engine.manager.play(data) {
            Ok(mut handle) => {
                handle.set_volume(volume, Tween::default());
                if (rate - 1.0).abs() > f64::EPSILON {
                    handle.set_playback_rate(rate, Tween::default());
                }
                engine.store_handle(&entity, handle);

                // 确保有对应的 emitter（用于空间音频）
                let _ = engine.register_emitter(&entity);
            }
            Err(e) => {
                log::error!("Failed to play sound on entity {:?}: {}", entity, e);
            }
        }
    }

    for entity in stop_requests {
        engine.unregister_emitter(&entity);
    }

    // Phase 3: 更新 AudioSource 状态标记
    let mut query = world.query_mut::<&mut AudioSource>();
    for mut source in query.into_iter() {
        if matches!(source.state, AudioSourceState::Playing { just_started: true }) {
            source.state = AudioSourceState::Playing { just_started: false };
        }
    }
}
```

### 6.4 System 3: 清理

```rust
/// 清理已销毁实体的 emitter
///
/// 利用 Entity 的代际（generation）机制检测：如果 Entity 已被 despawn，
/// `world.contains()` 返回 false，则移除对应的 emitter。
fn audio_cleanup_system(world: &World, engine: &mut AudioEngine) {
    engine.emitters.retain(|entity, _| world.contains(entity));
    engine.active_handles.retain(|entity, _| world.contains(entity));
}
```

### 6.5 System 4: 全局音量同步

```rust
/// 将 AudioGlobalSettings 同步到 kira 轨道音量
fn audio_settings_sync_system(world: &World, engine: &mut AudioEngine) {
    use crate::audio::components::AudioGlobalSettings;
    use kira::tween::Tween;

    let query = world.query::<&AudioGlobalSettings>();
    if let Some(settings) = query.into_iter().next() {
        let sfx_vol = if settings.muted { 0.0 } else { settings.master_volume * settings.sfx_volume };
        let music_vol = if settings.muted { 0.0 } else { settings.master_volume * settings.music_volume };

        engine.sfx_track.set_volume(sfx_vol, Tween::default());
        engine.music_track.set_volume(music_vol, Tween::default());
    }
}
```

---

## 7. KairosGame 最终形态

```rust
// kairos_engine/src/kairos_game.rs

use std::path::PathBuf;

use crate::{
    asset_loader::assets::{
        MaterialAssetsSystem, MeshAssetsSystem, ShaderAssetsSystem, TextureAssetsSystem,
    },
    audio::{
        audio_engine::AudioEngine,
        components::{AudioListener, AudioGlobalSettings},
        sound_asset::SoundAssetsSystem,
        systems::{
            audio_spatial_sync_system,
            audio_playback_system,
            audio_cleanup_system,
            audio_settings_sync_system,
        },
    },
    base_components::TransformComponent,
    ecs::world::World,
    graphics::{
        graphics_graph::GraphicsCommand,
        lod_mesh_component::LODMeshComponent,
        material_component::MaterialComponent,
    },
    math::{float3, quaternion},
};

pub struct KairosGame {
    pub audio_engine: AudioEngine,
    settings_entity: Entity,
}

impl KairosGame {
    pub fn new(world: &mut World) -> Self {
        // ──── 现有资产系统 ────
        let assets_server = &mut world.assets_server;
        assets_server.push(TextureAssetsSystem::new());
        assets_server.push(ShaderAssetsSystem::new());
        assets_server.push(MaterialAssetsSystem::new());
        assets_server.push(MeshAssetsSystem::new());

        // ──── 新增：音频资产系统 ────
        assets_server.push(SoundAssetsSystem::new());

        // ──── 初始化音频引擎 ────
        let audio_engine = AudioEngine::new()
            .expect("Failed to initialize audio engine");

        // ──── 创建全局音频设置实体 ────
        let settings_entity = world.spawn(AudioGlobalSettings::default());

        // ──── 现有场景创建 ────
        let mesh = assets_server.load::<MeshAssetsSystem>(
            PathBuf::from("res/models/Suzanne.mesh"),
        );
        let material = assets_server.load::<MaterialAssetsSystem>(
            PathBuf::from("res/materials/material.mat"),
        );

        const NUM_INSTANCES_PER_ROW: i32 = 5;
        world.spawn_batch(
            (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW)
                .flat_map(|z| (-NUM_INSTANCES_PER_ROW..NUM_INSTANCES_PER_ROW).map(move |x| (x, z)))
                .map(|(x, z)| {
                    let position = float3::new(x as f32, 0.0, z as f32);
                    let rotation = quaternion::identity();
                    let scale = float3::new(1.0, 1.0, 1.0);
                    (
                        TransformComponent::new(position, rotation, scale),
                        LODMeshComponent::new(mesh.clone()),
                        MaterialComponent::new(material.clone()),
                    )
                }),
        );

        Self {
            audio_engine,
            settings_entity,
        }
    }

    pub fn update(&mut self, world: &mut World) {
        world.time.update();
        let total_time = world.time.total_time().as_secs_f32();

        // ──── 音频系统管线 ────
        audio_spatial_sync_system(world, &mut self.audio_engine);
        audio_playback_system(world, &mut self.audio_engine);
        audio_cleanup_system(world, &mut self.audio_engine);
        audio_settings_sync_system(world, &mut self.audio_engine);

        // ──── 原有更新逻辑 ────
        let transforms = world.query_mut::<&mut TransformComponent>().into_iter();
        transforms.for_each(|trans| {
            let position = &mut trans.position;
            let x = position.x();
            let y = (x + total_time).sin();
            *position = float3::new(x, y, position.z());
        });
    }

    pub fn render(&self, world: &mut World, graphics_command: &mut GraphicsCommand) {
        let renderers = world
            .query_mut::<(&TransformComponent, &LODMeshComponent, &MaterialComponent)>()
            .into_iter();
        renderers.for_each(|(trans, lod, mat)| {
            graphics_command.draw(
                lod.lod0.clone(),
                mat.material.clone(),
                trans.get_local_to_world(),
            );
        });
    }
}
```

---

## 8. 使用示例

### 8.1 加载音频资产

```rust
// 在任何可以访问 world.assets_server 的地方
let gunshot = world.assets_server
    .load::<SoundAssetsSystem>(PathBuf::from("res/audio/gunshot.ogg"));

let explosion = world.assets_server
    .load::<SoundAssetsSystem>(PathBuf::from("res/audio/explosion.ogg"));

let bgm = world.assets_server
    .load::<SoundAssetsSystem>(PathBuf::from("res/audio/ambient.ogg"));
```

### 8.2 创建空间音效实体

```rust
// 枪声 — 挂有 AudioSource 的实体
let gun_entity = world.spawn((
    TransformComponent::new(
        float3(5.0, 1.5, 0.0),
        quaternion::identity(),
        float3(1.0, 1.0, 1.0),
    ),
    AudioSource::new(gunshot.clone()).with_volume(0.8),
));

// 爆炸 — 可循环的环境音
world.spawn((
    TransformComponent::new(
        float3(0.0, 0.0, 10.0),
        quaternion::identity(),
        float3(1.0, 1.0, 1.0),
    ),
    AudioSource::new(explosion.clone())
        .with_volume(1.0)
        .looping(),
));
```

### 8.3 创建摄像机（带 AudioListener）

```rust
let camera = world.spawn((
    TransformComponent::new(
        float3(0.0, 1.7, 5.0),   // 耳朵高度 = 1.7m
        quaternion::identity(),    // 面朝 -Z
        float3(1.0, 1.0, 1.0),
    ),
    AudioListener,
));
```

### 8.4 触发播放

```rust
/// 开枪时触发音效
fn fire_gun(world: &mut World, gun_entity: &Entity) {
    if let Ok(mut source) = world.query_one_mut::<&mut AudioSource>(gun_entity) {
        source.play();
    }
}

/// 停止所有音效（如暂停菜单）
fn stop_all_sfx(world: &mut World) {
    let mut query = world.query_mut::<&mut AudioSource>();
    for mut source in query.into_iter() {
        source.stop();
    }
}
```

### 8.5 调整全局音量

```rust
/// 设置主音量为 50%
fn set_half_volume(world: &mut World) {
    let mut query = world.query_mut::<&mut AudioGlobalSettings>();
    if let Some(mut settings) = query.into_iter().next() {
        settings.master_volume = 0.5;
    }
}
```

---

## 9. 线程安全分析

| 组件 | 线程模型 | 安全性 |
|---|---|---|
| `kira::AudioManager<T>` | 内部独立音频线程（cpal 回调） | `Send + !Sync` — 独占所有者访问 |
| `StaticSoundData` | 内部 `Arc<Vec<f32>>` | `Send + Sync` — 可跨线程 clone |
| `StaticSoundHandle` | 通过 `mpsc::Sender` 与音频线程通信 | `Send` — 可跨线程移动 |
| `EmitterHandle` / `ListenerHandle` | 同 StaticSoundHandle | `Send` — 可跨线程移动 |
| ECS `QueryBorrow` / `QueryMut` | 编译期借用检查 | 编译期保证无数据竞争 |

**为什么 `AudioEngine` 放在 `KairosGame` 而非 `World` 中：**

1. `World` 目前不是 `Send`（内部有 `Mutex` 等），不需要它跨线程传递
2. `AudioEngine` 在 `KairosGame::update(&mut self, world: &mut World)` 中通过 `&mut self` 独占访问
3. 这自然保证了 `AudioEngine` 和 `World` 不会在音频线程中被同时访问

**并行处理潜力：**

当前设计中，ECS 查询（`query_mut`）在调用 kira 操作之前收集所有状态变化。未来可以：
- 使用 `rayon` 并行处理大量 `AudioSource` 的状态读取
- kira 内部已在独立线程运行音频处理，不阻塞渲染

---

## 10. Cargo.toml 变更

```toml
# kairos_engine/Cargo.toml

[dependencies]
# ... 现有依赖 ...

# 取消注释 kira
kira = "0.12"

# 可选：如果需要额外的音频格式支持
# kira 默认支持 ogg/vorbis，如需 mp3/aac/flac 等：
# symphonia = { version = "0.6", features = ["all"] }
```

kira 的 feature flags（0.12.x）：

| Feature | 说明 |
|---|---|
| `default` | 包含 `ogg` 格式支持 |
| `mp3` | 启用 MP3 解码（通过 symphonia） |
| `flac` | 启用 FLAC 解码 |
| `wav` | 启用 WAV 解码 |
| `symphonia` | 完整 symphonia 后端，支持所有格式 |

---

## 11. 文件结构

```
kairos_engine/src/
├── audio/
│   ├── mod.rs                  # 模块声明 + re-export
│   ├── components.rs           # AudioSource, AudioListener, AudioGlobalSettings
│   ├── audio_engine.rs         # AudioEngine 核心结构
│   ├── sound_asset.rs          # SoundAssetsSystem (AssetSystem trait impl)
│   └── systems.rs              # 所有 system 函数
├── asset_loader/               # (现有)
├── base_components/            # (现有)
├── ecs/                        # (现有)
├── graphics/                   # (现有)
├── kairos_game.rs              # 修改：添加 AudioEngine + system 调用
├── lib.rs                      # 修改：添加 pub mod audio
└── ...
```

---

## 12. 扩展方向

### 12.1 音频事件系统

在 `AudioSource` 上添加回调：

```rust
pub enum AudioEvent {
    Started,
    Finished,
    Looped { iteration: u32 },
}

// 通过 ECS 的 Event 系统或 channel 发送
```

### 12.2 高级音效路由

```rust
/// 标记组件，指定声音路由到哪个轨道
pub struct AudioRoute(pub AudioTrackType);

pub enum AudioTrackType {
    Sfx,
    Music,
    Voice,
    Ambient,
}
```

### 12.3 遮挡/混响区域

```rust
/// 音频遮挡组件（如墙体）
pub struct AudioOcclusion {
    pub attenuation: f64,       // 衰减系数
    pub low_pass_cutoff: f64,   // 低通滤波截止频率
}

/// 混响区域（如洞穴、大厅）
pub struct AudioReverbZone {
    pub radius: f32,
    pub wet_level: f64,
    pub room_size: f64,
}
```

### 12.4 距离衰减曲线

kira 的 `EmitterSettings` 支持自定义距离衰减，可配合引擎配置：

```rust
use kira::spatial::emitter::EmitterSettings;

let emitter_settings = EmitterSettings::default()
    .attenuation(kira::spatial::emitter::Attenuation::InverseSquare)
    .max_distance(100.0);
```

### 12.5 流式音频（大文件/背景音乐）

```rust
use kira::sound::streaming::StreamingSoundData;

// 适用于长背景音乐，边播边加载
let stream_data = StreamingSoundData::from_file("res/audio/long_bgm.ogg")?;
// 注意：StreamingSoundData 不是 'static，需要特殊处理
```

---

## 参考资料

- [kira crates.io](https://crates.io/crates/kira)
- [kira GitHub](https://github.com/tesselode/kira)
- [kira 文档](https://docs.rs/kira/)
- [Symphonia (音频解码)](https://github.com/pdeljanov/Symphonia)
