# 空间音频轨道池（Spatial Track Pool）设计方案

> 日期：2026-06-25
> 基于：kira 0.12 + KairosEngine ECS 架构
> 问题：spatial audio track 过多时导致音效播放异常

---

## 目录

1. [问题根因](#1-问题根因)
2. [方案总览](#2-方案总览)
3. [ECS 组件设计](#3-ecs-组件设计)
4. [核心数据结构](#4-核心数据结构)
5. [算法：每帧轨道分配](#5-算法每帧轨道分配)
6. [Listener 轨道分配权重](#6-listener-轨道分配权重)
7. [降级距离衰减模型](#7-降级距离衰减模型)
8. [跨轨道迁移](#8-跨轨道迁移spatial--fallback)
9. [滞回机制](#9-滞回机制)
10. [内存与缓存分析](#10-内存与缓存分析)
11. [效率优化：Top-K 堆代替全排序](#11-效率优化top-k-堆代替全排序)
12. [与现有代码的对接点](#12-与现有代码的对接点)
13. [边界情况处理](#13-边界情况处理)
14. [分阶段实现建议](#14-分阶段实现建议)
15. [关键参数推荐值](#15-关键参数推荐值)

---

## 1. 问题根因

当前 `SpatialAudioVolumeComponent::new()` 里每个 volume 都调用 `manager.add_spatial_sub_track()` 独立创建一条 `SpatialTrackHandle`。kira 的 spatial track 内部做 HRTF 运算，开销与轨道数成正比。当场景中有上百个 volume 时，音频线程被大量 HRTF 计算淹没，导致 dropout / 爆音。

**核心矛盾：空间化轨道是稀缺 CPU 资源，但 volume 数量没有上限。**

---

## 2. 方案总览

```
┌──────────────────────────────────────────────────────────────────┐
│                     SpatialTrackPool                             │
│                                                                  │
│  ┌─────────────────────┐    ┌─────────────────────┐             │
│  │  Listener 0          │    │  Listener 1          │            │
│  │  tracks[0..15] (16)  │    │  tracks[16..31] (16) │            │
│  └──────┬──────────────┘    └──────┬──────────────┘             │
│         │                          │                              │
│         ▼                          ▼                              │
│  ┌──────────────┐          ┌──────────────┐                      │
│  │ 32个预分配    │          │              │                      │
│  │ SpatialTrack  │          │              │                      │
│  └──────────────┘          └──────────────┘                      │
│                                                                  │
│  超出轨道的 volume ──► MainTrack (distance-attenuated fallback)  │
└──────────────────────────────────────────────────────────────────┘
```

三条核心规则：

| 规则 | 说明 |
|---|---|
| **固定轨道池** | 启动时预分配 N 条 `SpatialTrackHandle`（如 32），不再动态创建 |
| **Listener 权重分配** | M 个 Listener 按权重比例分得轨道；Listener 数量变化时重分配 |
| **距离排序 + 降级** | 每帧将 volume 按到 Listener 的距离排序，前 N 个走空间轨道（HRTF），后面的走主轨道（仅距离衰减音量，无 HRTF） |

---

## 3. ECS 组件设计

### 3.1 SpatialAudioListenerComponent

Listener 不再是游离对象，而是一个带有组件的普通 Entity：

```rust
/// 挂在摄像机/玩家实体上，将此实体的 Transform 作为空间音频的"耳朵"。
///
/// 支持多个 listener（如分屏合作），SpatialTrackPool 自动发现并管理。
#[derive(Debug, Clone)]
pub struct SpatialAudioListenerComponent {
    /// 此 listener 的重要性权重，影响轨道分配比例。
    /// 1.0 = 标准，> 1.0 = 更多轨道，< 1.0 = 更少轨道。
    pub weight: f32,
}

impl Component for SpatialAudioListenerComponent {}

impl Default for SpatialAudioListenerComponent {
    fn default() -> Self {
        Self { weight: 1.0 }
    }
}
```

使用示例：

```rust
// 创建带 listener 的摄像机
let camera = world.spawn((
    TransformComponent::new(position, rotation, scale),
    SpatialAudioListenerComponent::default(),
));

// 分屏第二个玩家，权重较低
let p2_camera = world.spawn((
    TransformComponent::new(p2_pos, p2_rot, scale),
    SpatialAudioListenerComponent { weight: 0.6 },
));
```

### 3.2 SpatialAudioVolumeComponent（改造后）

```rust
pub struct SpatialAudioVolumeComponent {
    // 【移除】 pub track: Option<SpatialTrackHandle>,

    pub audios: SmallVec<[AudioAssetHandle; 4]>,

    /// 当前播放模式（由 Pool 每帧写入）
    pub playback_mode: SpatialPlaybackMode,

    /// 空间轨道上的播放句柄
    pub spatial_handles: SmallVec<[SpatialSoundHandle; 4]>,

    /// 主轨道上的降级播放句柄
    pub fallback_handles: SmallVec<[SpatialSoundHandle; 4]>,

    /// 手动优先级偏移（> 0 = 更容易获得轨道，< 0 = 更倾向降级）
    pub priority_bias: f32,

    /// 降级模式下当前帧的距离衰减倍率（由 pool 每帧写入）
    pub fallback_attenuation: f32,

    pub auto_play: bool,
    pub state: SpatialAudioVolumeState,
}

impl Component for SpatialAudioVolumeComponent {}

pub enum SpatialPlaybackMode {
    /// 尚未开始或已停止
    Idle,
    /// 占用了一个空间轨道
    Spatial { track_slot_index: usize },
    /// 在主轨道上降级播放
    Fallback,
}
```

---

## 4. 核心数据结构

```rust
// ═══════════════════════════════════════════════════════
// 配置
// ═══════════════════════════════════════════════════════
pub struct SpatialTrackPoolConfig {
    /// 最大空间音频轨道数（所有 listener 共享的总数）
    pub max_spatial_tracks: usize,       // 默认 32
    /// 最大 listener 数
    pub max_listeners: usize,            // 默认 2
    /// 降级轨道的距离衰减系数
    pub fallback_rolloff: f32,           // 默认 1.0
    /// 降级轨道的参考距离（在此距离上音量约为 1/(1+rolloff)）
    pub reference_distance: f32,         // 默认 10.0
    /// 最大可听距离（超过此距离音量为 0，直接跳过播放）
    pub max_distance: f32,               // 默认 100.0
    /// 防止反复抢占的滞回系数（0.0 ~ 1.0）
    pub hysteresis: f32,                 // 默认 0.15
    /// 跨轨道迁移时的交叉淡入淡出时长（秒）
    pub migration_crossfade: f32,        // 默认 0.05
}

// ═══════════════════════════════════════════════════════
// 轨道槽位
// ═══════════════════════════════════════════════════════
struct SpatialTrackSlot {
    track: SpatialTrackHandle,
    /// 当前占用的 volume（Entity）
    occupant: Option<Entity>,
    /// 属于哪个 listener
    listener_index: usize,
    /// 稳定性计数：连续被同一 volume 占用的帧数
    stability_counter: u32,
}

// ═══════════════════════════════════════════════════════
// Listener 槽位
// ═══════════════════════════════════════════════════════
pub struct ListenerSlot {
    pub handle: ListenerHandle,
    pub entity: Entity,
    pub weight: f32,
    /// 在 tracks 数组中的起始索引
    track_offset: usize,
    /// 分配到的 track 数量
    track_count: usize,
}

// ═══════════════════════════════════════════════════════
// 堆元素（Top-K 选择用）
// ═══════════════════════════════════════════════════════
/// min-heap 元素：得分低的在堆顶，堆满时被淘汰
struct HeapEntry {
    score: f32,
    entity: Entity,
    distance: f32,
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.score.partial_cmp(&self.score).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}
impl Eq for HeapEntry {}

// ═══════════════════════════════════════════════════════
// 轨道池主结构
// ═══════════════════════════════════════════════════════
pub struct SpatialTrackPool {
    config: SpatialTrackPoolConfig,
    tracks: Vec<SpatialTrackSlot>,
    listeners: Vec<ListenerSlot>,

    /// 帧间复用的 Top-K 堆（每 listener 一个），clear 后复用，不重新分配
    heaps: Vec<BinaryHeap<HeapEntry>>,

    /// 上一帧的 occupant 记录（用于滞回判断）
    /// entity → (track_slot_index, stability)
    prev_occupants: HashMap<Entity, (usize, u32)>,
}
```

---

## 5. 算法：每帧轨道分配

```
┌─────────────────────────────────────────────────────────────────┐
│  SpatialTrackPool::update(world)                                  │
│                                                                   │
│  Phase 1: Listener 发现与同步                                     │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ query: (&Transform, &SpatialAudioListener)                  │ │
│  │                                                               │ │
│  │ 新出现的 entity → register_listener()                        │ │
│  │ 消失的 entity   → unregister_listener()                      │ │
│  │ 同步已有 listener 的 Transform 到 kira ListenerHandle       │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Phase 2: 流式 Top-K 选择（单次 query，纯 CPU）                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ query: (&Transform, &SpatialAudioVolume)                     │ │
│  │                                                               │ │
│  │ for each Playing volume:                                     │ │
│  │   nearest_listener = min(distance to each listener)          │ │
│  │   if distance > max_distance: skip (连降级也不播)            │ │
│  │   score = priority_bias + 1.0 / (distance + 0.001)           │ │
│  │   heap[listener].push_if_better(score, entity, distance)     │ │
│  │                                                               │ │
│  │ 复杂度: O(V log K)，V=volume 数，K=每个 listener 轨道数     │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Phase 3: 堆 → 分配结果                                          │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ 堆中元素 = Spatial（获得空间轨道）                           │ │
│  │ 其余      = Fallback（降级到主轨道）                         │ │
│  │                                                               │ │
│  │ 滞回：已在堆中的旧 occupant 享受 score bonus                 │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  Phase 4: 应用分配结果（mut query）                               │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │ 遍历所有 volume，按分配结果设置 playback_mode                │ │
│  │ mode 变化 → 执行迁移（migrate）                              │ │
│  │ mode 不变 → 仅更新位置（Spatial）或音量（Fallback）         │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 关键：两阶段查询避免 Borrow 冲突

ECS 不能同时持有 read-only query 和 mutable query。通过将"收集排序"和"写入分配结果"拆成两个阶段：

```
  时间线
  ──────────────────────────────────────────────────────────

  Phase 1+2:  world.query::<(&T, &Volume)>()     ← 只读
              │
              │  堆中填充 HeapEntry
              │
              ▼ drop read query ─────────── 释放 World borrow

  Phase 3:    堆 → HashSet<Entity>   ← 纯 CPU，不碰 World

              │
              ▼

  Phase 4:    world.query_mut::<&mut Volume>()    ← 可变
```

---

## 6. Listener 轨道分配权重

```rust
fn redistribute_tracks(&mut self) {
    let total = self.config.max_spatial_tracks;
    if self.listeners.is_empty() {
        return;
    }

    // 按权重分配轨道数
    let total_weight: f32 = self.listeners.iter().map(|l| l.weight).sum();
    let mut allocated = 0;
    for (i, slot) in self.listeners.iter_mut().enumerate() {
        let raw = total as f32 * (slot.weight / total_weight);
        let count = if i == self.listeners.len() - 1 {
            // 最后一个 listener 拿走剩余所有轨道（避免舍入损失）
            total - allocated
        } else {
            (raw.round() as usize).min(total - allocated)
        };
        slot.track_count = count;
        slot.track_offset = allocated;
        allocated += count;
    }
}
```

示例：

```
32 tracks, 2 listeners:  weight [1.0, 1.0]    → [16, 16]
32 tracks, 2 listeners:  weight [1.0, 0.3]    → [25,  7]   (主摄像机更多)
32 tracks, 3 listeners:  weight [1.0, 1.0, 0.5] → [13, 13, 6]
32 tracks, 1 listener:   weight [1.0]         → [32]
```

---

## 7. 降级距离衰减模型

对于降级播放的 volume，在主轨道上仅做距离衰减音量，不做 HRTF 空间化：

```rust
/// 计算降级播放时的音量倍率（无 HRTF，仅距离衰减）
fn fallback_attenuation(distance: f32, config: &SpatialTrackPoolConfig) -> f32 {
    if distance > config.max_distance {
        return 0.0;  // 超出最大距离，静音
    }
    // 类反平方衰减，近距离不会爆炸（分母的 +1 项）
    let d = distance / config.reference_distance;
    1.0 / (1.0 + d * d * config.fallback_rolloff)
}
```

衰减曲线：

```
distance=0    → attenuation=1.0    (全音量)
distance=ref  → attenuation=0.5    (半音量，ref=10m 时)
distance=20   → attenuation≈0.2    (很轻)
distance=100  → attenuation=0.0    (静音)
```

降级音量应用：

```rust
// Fallback mode 每帧更新
for handle in &mut vol.fallback_handles {
    if let SpatialSoundHandle::Some(h) = handle {
        h.set_volume(
            vol.base_volume * vol.fallback_attenuation,
            Tween::default(),
        );
    }
}
```

---

## 8. 跨轨道迁移（Spatial ↔ Fallback）

当 volume 在 Spatial / Fallback 之间切换时，需要平滑过渡。

### 8.1 简化版：硬切（Phase 1 实现）

```
Fallback → Spatial:
  1. 记录 fallback handle 的当前播放位置
  2. 在空间轨道上以相同位置启动播放
  3. 停止 fallback handle

Spatial → Fallback:
  1. 记录 spatial handle 的当前播放位置
  2. 在主轨道上以相同位置 + 衰减音量启动播放
  3. 停止 spatial handle
```

### 8.2 完整版：交叉淡入淡出（Phase 3 实现）

```
迁移时序（以 Fallback → Spatial 为例）：

  Frame N:   系统决定将此 volume 升级为 Spatial
             │
             ▼
  Frame N:   在空间轨道上以 volume=0 启动播放，设 start_position = fallback 当前位置
             │
             ▼
  Frame N+1: 空间轨 volume tween → 目标音量（migration_crossfade 秒）
             fallback volume tween → 0（migration_crossfade 秒）
             │
             ▼
  Frame N+K: 停止 fallback 播放句柄，释放
             将此 volume 的 playback_mode 设为 Spatial
```

迁移状态跟踪：

```rust
enum MigrationState {
    Stable,
    Promoting { target_track: usize, start_frame: u64 },
    Demoting { start_frame: u64, target_attenuation: f32 },
}
```

---

## 9. 滞回机制

当两个 volume 距离相近且恰好排在 cutoff 边界上时，防止每帧来回切换：

```rust
/// 已在轨道的 volume 享受滞回保护
fn effective_score(entry: &HeapEntry, is_prev_occupant: bool, hysteresis: f32) -> f32 {
    if is_prev_occupant {
        entry.score * (1.0 + hysteresis)  // 已在轨道：分数上浮 15%
    } else {
        entry.score
    }
}
```

举例：`hysteresis = 0.15`，已在轨道的 volume 距离 5m（score=0.2），新来者需要距离 ≤ 4.3m（score ≥ 0.23）才能抢走。

配合稳定性帧计数：

```
已在轨道的 volume: stability_counter 递增
新 candidate 需连续优势 3 帧才执行抢占
刚被抢占的 volume: 3 帧内不能反抢（冷却期）
```

---

## 10. 内存与缓存分析

### 10.1 零堆分配策略

所有 buffer 挂在 Pool 上复用：

```rust
pub struct SpatialTrackPool {
    // ...
    heaps: Vec<BinaryHeap<HeapEntry>>,  // ← 每帧 clear，不重新分配
}

// 每帧：
for heap in &mut self.heaps {
    heap.clear();  // 清空内容，保留底层 Vec 的 capacity
}
// 堆的 capacity 趋近历史最大值后，push 永远不触发堆分配
```

### 10.2 缓存行为

```
ECS Archetype Table (SoA 列存):
┌────────────────────────────────────────────────────┐
│ Archetype: (TransformComponent, SpatialAudioVolume, ...) │
│                                                             │
│ Column[Transform]:  [T0][T1][T2]...[Tn]  ← 连续存储        │
│ Column[Volume]:     [V0][V1][V2]...[Vn]  ← 连续存储        │
└────────────────────────────────────────────────────┘

遍历时 cache 行为:
  for each entity i:
    read Transform[i]   ← SoA 顺序扫描，L2/L3 预取友好
    read Volume[i]      ← 同上

问题：TransformComponent (~40 bytes) 中仅 position (12 bytes) 被使用，
      其余 28 bytes 被加载但未使用，浪费 ~70% cache 带宽。

      但这是 Transform 组件设计的问题，不值得仅为 audio 系统拆细组件
      （如拆分 PositionComponent），对全局架构改动过大。

Top-K 堆的 cache:
  HeapEntry = 24 bytes，64B cache line 装 2.6 个
  每个 listener 的堆仅 K 个元素，K=16 时 384 bytes，完全在 L1 内运行
```

### 10.3 复杂度对比

| 方法 | 时间复杂度 | 1000 volume + 16 track 场景 |
|---|---|---|
| 全排序 (Vec+sort) | O(V log V) | ~10,000 次比较 |
| **Top-K 堆（采用）** | **O(V log K)** | **~4,000 次比较** |
| 空间分配 | O(V) | 1000 次 distance² |
| mut query 写入 | O(V) | 1000 次单实体查询 |

每帧总开销 < 0.5ms，对 60fps 无影响。

### 10.4 成熟度：这是 ECS 中的标准做法

| 引擎 | 做法 |
|---|---|
| **Bevy (Rust)** | 渲染队列排序：`transparent_3d_phase.items.sort_by(...)` — 从 query 收集到 Vec，排序，再提交 |
| **Flecs (C)** | `ecs_query_order_by()` — 内部就是 `ecs_vector_t` 存 entity id，然后 qsort |
| **Unity DOTS** | `EntityQuery.ToEntityArray()` → `NativeArray.Sort()` → 回写 |
| **Unreal Mass Entity** | `FEntityQuery` + `Algo::Sort`，同样是把匹配的 entity handle 拷出来排序 |

**原因：ECS archetype 存储是按"组件组合"分组的，不是按"排序键"组织的。无论哪种引擎，跨实体排序都必须引入一个间接层。**

---

## 11. 效率优化：Top-K 堆代替全排序

### 11.1 为什么不需要全排序

每个 Listener 只需要前 K 个 volume（K = 其轨道数），其余全部降级。这是一个经典的"流式 Top-K"问题：

```
BinaryHeap<HeapEntry> (min-heap):
  - 堆顶 = 堆中 score 最小的元素（最容易淘汰的）
  - 堆大小 ≤ K

遍历所有 Playing volume:
  if heap.len() < K:
      heap.push(entry)                      // 堆未满，直接入
  else if entry.score > heap.peek().score:
      heap.pop(); heap.push(entry)          // 替换堆中最差的
  else:
      // entry 不够好，忽略
```

### 11.2 滞回版 Top-K

```rust
impl SpatialTrackPool {
    pub fn update(&mut self, world: &World, audio_manager: &mut AudioManager) {
        // Phase 1: Listener 发现与同步
        self.sync_listeners(world, audio_manager);

        // Phase 2: 为每个 listener 初始化堆（复用，clear 即可）
        for heap in &mut self.heaps {
            heap.clear();
        }

        // Phase 3: 流式 Top-K
        {
            let volumes = world.query::<(&TransformComponent, &SpatialAudioVolumeComponent)>();
            for (entity_ref, (trans, vol)) in volumes.entities() {
                if vol.state != SpatialAudioVolumeState::Playing {
                    continue;
                }

                let (li, dist) = self.nearest_listener(trans.position);
                if dist > self.config.max_distance {
                    continue; // 超出最大距离，连 fallback 都不播
                }

                let score = vol.priority_bias + 1.0 / (dist + 0.001);

                // 滞回：已在轨道的旧 occupant 分数上浮
                let is_occupant = self.prev_occupants
                    .get(&entity_ref.entity)
                    .map(|(_, stability)| *stability >= 3)
                    .unwrap_or(false);
                let eff_score = if is_occupant {
                    score * (1.0 + self.config.hysteresis)
                } else {
                    score
                };

                let heap = &mut self.heaps[li];
                let k = self.listeners[li].track_count;

                if heap.len() < k {
                    heap.push(HeapEntry {
                        score: eff_score,
                        entity: entity_ref.entity.clone(),
                        distance: dist,
                    });
                } else if let Some(top) = heap.peek() {
                    if eff_score > top.score {
                        heap.pop();
                        heap.push(HeapEntry {
                            score: eff_score,
                            entity: entity_ref.entity.clone(),
                            distance: dist,
                        });
                    }
                }
            }
        } // ← read query drop

        // Phase 4: 堆中 entity → HashSet，O(1) 判断
        let spatial_sets: Vec<HashSet<Entity>> = self.heaps
            .iter()
            .map(|h| h.iter().map(|e| e.entity.clone()).collect())
            .collect();

        // Phase 5: 应用结果（mut query）
        // ...
    }

    fn nearest_listener(&self, pos: float3) -> (usize, f32) {
        // 找距离最近的 listener，返回 (index, distance)
        // ...
    }
}
```

---

## 12. 与现有代码的对接点

### 12.1 文件改动清单

| 现有文件 | 改动 |
|---|---|
| `audio.rs` (AudioEngine) | 持有 `SpatialTrackPool`；`update()` 先调 `pool.update()`；`add_spatial_listener()` 代理到 `pool.register_listener()` |
| `audio/spatial_audio_volume.rs` | 移除 `track: Option<SpatialTrackHandle>`；添加 `playback_mode`, `fallback_handles`, `priority_bias`, `fallback_attenuation`；`new()` 不再创建 SpatialTrack |
| `audio/spatial_audio_listener.rs` | 填充 `SpatialAudioListenerComponent` 定义 |
| `kairos_game.rs` | 创建 volume 前先向 pool 注册 listener |

### 12.2 新增文件

```
kairos_engine/src/audio/
├── spatial_track_pool.rs    ← SpatialTrackPool, SpatialTrackSlot,
│                               ListenerSlot, SpatialTrackPoolConfig
├── spatial_audio_listener.rs ← SpatialAudioListenerComponent (已有空文件)
├── spatial_audio_volume.rs   ← 改造
└── ...
```

### 12.3 AudioEngine 最终形态

```rust
pub struct AudioEngine {
    manager: AudioManager,
    /// 空间轨道池（持有所有 SpatialTrackHandle + ListenerHandle）
    pool: SpatialTrackPool,
    /// 主轨道 handle（用于降级播放）
    main_track: TrackHandle,
}

impl AudioEngine {
    pub fn update(&mut self, world: &mut World, assets_server: &mut AssetsServer) {
        // 第一步：pool 完成 listener 发现 → Top-K 排序 → 轨道分配 → 迁移
        self.pool.update(world, &mut self.manager);

        // 第二步：处理 volume 状态机
        let volumes = world.query_mut::<(&TransformComponent, &mut SpatialAudioVolumeComponent)>();
        for (trans, vol) in volumes.into_iter() {
            match &vol.playback_mode {
                SpatialPlaybackMode::Spatial { track_slot_index } => {
                    // 已占有轨道，仅更新位置
                    let track = &mut self.pool.tracks[*track_slot_index].track;
                    track.set_position(trans.position, Tween::default());
                }
                SpatialPlaybackMode::Fallback => {
                    // 降级，更新音量衰减
                    for handle in &mut vol.fallback_handles {
                        if let SpatialSoundHandle::Some(h) = handle {
                            h.set_volume(
                                vol.base_volume * vol.fallback_attenuation,
                                Tween::default(),
                            );
                        }
                    }
                }
                SpatialPlaybackMode::Idle => {}
            }

            // ... 原有状态机（Created → WaitLoading → Playing 等）...
        }
    }
}
```

---

## 13. 边界情况处理

| 场景 | 策略 |
|---|---|
| **0 个 Listener** | 所有 volume 直接 Fallback，距离 decay 基于 volume 自身 position（无方向性） |
| **volume 数 ≤ 总轨道数** | 全部走 Spatial，无 Fallback |
| **Listener 被移除（Entity despawn）** | 其轨道释放，归属 volume 立即降级；其他 Listener 获得更多轨道 |
| **volume 暂停** | 立即释放轨道槽位给其他 volume |
| **volume 停止/销毁** | 立即释放轨道槽位 |
| **所有 volume 都远超 max_distance** | 不播放任何声音，但不释放轨道（空转开销很小） |
| **Listener 权重为 0** | 该 Listener 获得 0 个轨道，其所有 volume 直接走 Fallback |

---

## 14. 分阶段实现建议

| 阶段 | 内容 | 风险 |
|---|---|---|
| **Phase 1** | 固定轨道池 + Top-K 堆排序 + 硬切迁移（无交叉淡入淡出） | 低 |
| **Phase 2** | 滞回 + 稳定性帧计数 + 帧间 buffer 复用（零分配） | 低 |
| **Phase 3** | 交叉淡入淡出迁移 | 中 |
| **Phase 4** | priority_bias 支持、每-volume 自定义 rolloff、空间散列加速 | 低 |
| **Phase 5** | 动态轨道数（根据 CPU 负载自适应调整 max_spatial_tracks） | 高（可后延） |

---

## 15. 关键参数推荐值

```rust
SpatialTrackPoolConfig {
    max_spatial_tracks: 32,       // 移动端可降至 16，PC 可到 64
    max_listeners: 2,             // 典型：主摄像机 + 分屏
    fallback_rolloff: 1.0,        // 标准反平方衰减
    reference_distance: 10.0,     // 10 米处音量减半
    max_distance: 100.0,          // 100 米外静音
    hysteresis: 0.15,             // 15% 优势才抢占
    migration_crossfade: 0.05,    // 50ms 交叉淡入淡出
}
```

这些值应暴露为可配置项，允许游戏侧根据场景调整（室内 vs 开放世界）。

---

## 参考资料

- [kira crates.io](https://crates.io/crates/kira)
- [kira GitHub](https://github.com/tesselode/kira)
- [Bevy ECS ordering](https://bevyengine.org/learn/books/bevy-apps-and-data/ordering-systems/)
- [Flecs query order_by](https://www.flecs.dev/flecs/md_docs_2Queries.html#orderby)
