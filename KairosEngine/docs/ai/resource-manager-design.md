# 资源管理器设计方案

> 来源：AI 辅助设计讨论（2026-05）  
> 状态：设计草稿，未在代码库中实现

## 1. 目标

为游戏引擎 / 编辑器实现统一的资源加载模块，满足：

| 需求 | 说明 |
|------|------|
| 按类型分桶 | 每种资源类型 `T` 拥有独立的 `Vec<T>`，同类型数据在内存中连续存放 |
| 泛型加载 API | `res_mgr.load_resource::<T>(path)`，编译期确定类型 |
| 路径去重 | 同一路径重复加载时返回已有句柄，避免重复 IO |
| 可扩展 | 运行时支持多种资源类型，无需为所有类型写死单一枚举 |

## 2. 核心约束（Rust）

- 无法在 `ResourceManager` 中用**单个** `Vec<T>` 存储「所有可能的 `T`」——`T` 必须在编译期固定。
- 标准做法：**每个 `T` 一个存储桶**，运行期通过 `TypeId` 索引到对应桶（类型擦除）。
- `Vec` 扩容会移动元素，**不应长期持有** `&T`；对外返回 **`Handle<T>`**（索引句柄），通过 `get(handle)` 按需借用。

## 3. 架构总览

```
┌─────────────────────────────────────────────────────────┐
│  App / EditorApp / Engine                               │
│    └── res_mgr: ResourceManager                         │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  ResourceManager                                        │
│    ├── TypeMap: TypeId → Box<dyn Any>                   │
│    │     ├── Bucket<Texture>  { Vec, path_map }        │
│    │     └── Bucket<Mesh>     { Vec, path_map }        │
│    └── LoaderRegistry: TypeId → LoaderErased             │
└─────────────────────────────────────────────────────────┘

load_resource::<T>(path)
    → 查 path 缓存 → 命中则返回 Handle<T>
    → 否则 Loader 解码 → push 进 Bucket<T> → 返回 Handle<T>
```

## 4. 核心数据结构

### 4.1 TypeMap（类型擦除映射）

用 `HashMap<TypeId, Box<dyn Any>>` 存放各类型的 `Bucket<T>`，通过 `downcast` 取回具体类型。

```rust
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub struct TypeMap {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref()
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut()
    }

    pub fn get_or_insert_with<T: 'static + Send + Sync, F: FnOnce() -> T>(
        &mut self,
        f: F,
    ) -> &mut T {
        let id = TypeId::of::<T>();
        if !self.map.contains_key(&id) {
            self.map.insert(id, Box::new(f()));
        }
        self.map.get_mut(&id).unwrap().downcast_mut().unwrap()
    }
}
```

### 4.2 Handle 与 Bucket

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Handle<T> {
    index: u32,
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub fn index(self) -> usize {
        self.index as usize
    }
}

pub struct Bucket<T> {
    dense: Vec<T>,
    path_to_handle: HashMap<PathBuf, Handle<T>>,
}

impl<T> Bucket<T> {
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            path_to_handle: HashMap::new(),
        }
    }

    pub fn get(&self, h: Handle<T>) -> &T {
        &self.dense[h.index()]
    }

    pub fn get_mut(&mut self, h: Handle<T>) -> &mut T {
        &mut self.dense[h.index()]
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        let index = self.dense.len() as u32;
        self.dense.push(value);
        Handle {
            index,
            _marker: std::marker::PhantomData,
        }
    }
}
```

**可选扩展**：世代句柄（generational arena）、freelist 复用槽位，用于安全删除与热重载；初期可只支持 `push`、不删除。

### 4.3 Resource 与 Loader trait

```rust
pub trait Resource: 'static + Send + Sync {}

pub trait Loader<T: Resource>: Send + Sync {
    fn extensions(&self) -> &[&str];
    fn load(&self, path: &Path) -> Result<T, LoadError>;
}

#[derive(Debug)]
pub struct LoadError(pub String);
```

## 5. ResourceManager 实现要点

### 5.1 类型擦除的 Loader 边界

```rust
trait LoaderErased: Send + Sync {
    fn load_any(&self, path: &Path) -> Result<Box<dyn Any + Send + Sync>, LoadError>;
}

struct TypedLoader<T: Resource> {
    inner: Box<dyn Loader<T>>,
}

impl<T: Resource> LoaderErased for TypedLoader<T> {
    fn load_any(&self, path: &Path) -> Result<Box<dyn Any + Send + Sync>, LoadError> {
        self.inner.load(path).map(|v| Box::new(v) as _)
    }
}
```

### 5.2 管理器主体

```rust
pub struct ResourceManager {
    buckets: TypeMap,
    loaders: HashMap<TypeId, Box<dyn LoaderErased + Send + Sync>>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            buckets: TypeMap::new(),
            loaders: HashMap::new(),
        }
    }

    pub fn register_loader<T: Resource, L: Loader<T> + 'static>(&mut self, loader: L) {
        self.loaders.insert(
            TypeId::of::<T>(),
            Box::new(TypedLoader {
                inner: Box::new(loader),
            }),
        );
    }

    pub fn load_resource<T: Resource>(
        &mut self,
        path: impl AsRef<Path>,
    ) -> Result<Handle<T>, LoadError> {
        let path = path.as_ref().to_path_buf();

        let bucket = self
            .buckets
            .get_or_insert_with(Bucket::<T>::new);

        if let Some(&h) = bucket.path_to_handle.get(&path) {
            return Ok(h);
        }

        let loader = self
            .loaders
            .get(&TypeId::of::<T>())
            .ok_or_else(|| {
                LoadError(format!(
                    "no loader registered for {}",
                    std::any::type_name::<T>()
                ))
            })?;

        let value = loader
            .load_any(&path)?
            .downcast::<T>()
            .map_err(|_| LoadError("downcast failed".into()))?;

        let handle = bucket.insert(*value);
        bucket.path_to_handle.insert(path, handle);
        Ok(handle)
    }

    pub fn get<T: Resource>(&self, h: Handle<T>) -> &T {
        self.buckets
            .get::<Bucket<T>>()
            .expect("bucket not created")
            .get(h)
    }
}
```

**说明**：

- `load_resource::<T>` 在编译期绑定 `Bucket<T>` 与 `Loader<T>`。
- 运行期通过 `TypeId::of::<T>()` 查找桶与 loader。
- 未注册 loader 的类型在首次加载时会报错。

## 6. 资源类型与注册示例

```rust
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
impl Resource for Texture {}

pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
}
impl Resource for Mesh {}

pub struct PngTextureLoader;

impl Loader<Texture> for PngTextureLoader {
    fn extensions(&self) -> &[&str] {
        &["png"]
    }

    fn load(&self, path: &Path) -> Result<Texture, LoadError> {
        // 读文件、解码...
        let _ = path;
        Ok(Texture {
            width: 0,
            height: 0,
            pixels: vec![],
        })
    }
}
```

初始化与使用：

```rust
let mut res_mgr = ResourceManager::new();
res_mgr.register_loader::<Texture, _>(PngTextureLoader);
res_mgr.register_loader::<Mesh, _>(GltfMeshLoader);

let tex = res_mgr.load_resource::<Texture>("assets/foo.png")?;
let mesh = res_mgr.load_resource::<Mesh>("assets/bar.gltf")?;

let pixels = &res_mgr.get(tex).pixels;
```

## 7. Kairos 编辑器：`res_mgr` 与 `ui::Context`

> 对应当前 crate 结构：`KairosEngine`（`eframe::App`）拥有 `ui_context: ui::Context`，UI 经 `Messager` + `Drawer` trait 驱动。

### 7.1 所有权（推荐布局）

`res_mgr` 与 `ui_context` **并列**挂在 `KairosEngine` 上，**不要**放进 `ui::Context` 里拥有，也不要在 `Context` 里存 `&mut ResourceManager`（无法安全跨帧）。

```rust
pub struct KairosEngine {
    res_mgr: ResourceManager,
    ui_context: ui::Context,
}
```

| 做法 | 评价 |
|------|------|
| `KairosEngine { res_mgr, ui_context }` | ✅ 引擎级服务，将来非 UI 也可访问 |
| `ui::Context { res_mgr: ResourceManager }` | ❌ 资源被 UI 独占 |
| `ui::Context` 内长期存 `&mut res_mgr` | ❌ 生命周期不可行 |
| 全局 `static` | ❌ 与当前 App 聚合根风格不一致 |

### 7.2 每帧传递链

`ui::Context` 作协调者（消息、Dock、`Drawer` 列表）；`res_mgr` 在 **`eframe::App::update` 每帧以借用注入**：

```rust
impl eframe::App for KairosEngine {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        // res_mgr 与 ui_context 为不同字段，可同时 mut borrow
        self.ui_context.handle(ctx, &mut self.res_mgr);
        self.ui_context.darw(ctx, frame, &mut self.res_mgr);
    }
}
```

```rust
impl ui::Context {
    pub fn handle(&mut self, ctx: &egui::Context, res_mgr: &mut ResourceManager) { /* ... */ }
    pub fn darw(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame, res_mgr: &mut ResourceManager) { /* ... */ }
}
```

`handle` 中按消息创建 `ToolBar` 等时，若构造阶段需要资源，同样传入 `res_mgr`：

```rust
Message::CreateToolbar => {
    let drawer = ToolBar::new(res_mgr)?;
    self.push_drawer(Box::new(drawer));
}
```

或在 Drawer 首次 `update` 中延迟加载（构造期不碰资源亦可）。

### 7.3 `Drawer` / `TabDrawer` 签名

与 `messager` 同级，在 trait 方法中增加 `res_mgr`；`DockArea::show_inside` 等中间层同步透传。

```rust
pub trait Drawer: Any {
    fn update(
        &mut self,  // 建议 &mut self：缓存 Handle<T>、面板状态
        ctx: &eframe::egui::Context,
        frame: &mut eframe::Frame,
        messager: &mut Messager,
        res_mgr: &mut ResourceManager,
    );
    // ...
}
```

Drawer **只缓存 `Handle<T>`**，不缓存 `&Texture`（`Vec` 扩容会使引用失效）：

```rust
struct ProjectPanel {
    folder_icon: Handle<Texture>,
}

impl Drawer for ProjectPanel {
    fn update(&mut self, ctx, frame, messager, res_mgr) {
        let h = res_mgr.load_resource::<Texture>("ui/icons/folder.png")?;
        self.folder_icon = h;
        let tex = res_mgr.get(self.folder_icon);
        // egui::Image::from_texture(...) 等
    }
}
```

### 7.4 `FrameCtx`（服务增多时）

避免 `handle` / `darw` / `Drawer::update` 参数膨胀，可抽每帧上下文（命名避开 egui 的 `Context`）：

```rust
pub struct FrameCtx<'a> {
    pub egui: &'a egui::Context,
    pub frame: &'a mut eframe::Frame,
    pub res_mgr: &'a mut ResourceManager,
    pub messager: &'a mut Messager,
}

// KairosEngine::update
let mut fcx = FrameCtx {
    egui: ctx,
    frame,
    res_mgr: &mut self.res_mgr,
    messager: &mut self.ui_context.messager,
};
self.ui_context.handle(&mut fcx);
self.ui_context.darw(&mut fcx);
```

新增 `device`、ECS `World` 等时只扩展 `FrameCtx` 字段。

### 7.5 与 `Messager` 的分工

| 机制 | 用途 |
|------|------|
| `Message` | 打开/关闭窗口、刷新样式等 **控制流** |
| `res_mgr` 参数 | `load_resource::<T>` 等 **需要 `&mut` + 泛型 `T`** 的 IO |

不宜把「加载某路径」塞进 `Message`（除非另做异步队列 + 主线程 commit，属后续阶段）。

### 7.6 架构示意

```
KairosEngine (eframe::App)
├── res_mgr: ResourceManager
└── ui_context: ui::Context
         │
         │ 每帧 &mut res_mgr
         ▼
    handle / darw
         │
         ├── Messager → 消息处理、创建 Drawer
         └── Drawer::update / TabDrawer::ui → load_resource / get(Handle)
```

---

## 8. 程序中如何获取 `res_mgr`（通用模式）

按推荐程度排序：

### 8.1 挂在应用根状态（推荐）

适用于单线程主循环（含 Kairos 编辑器，见 §7）：

```rust
pub struct KairosEngine {
    pub res_mgr: ResourceManager,
    pub ui_context: ui::Context,
}

impl KairosEngine {
    fn frame(&mut self, egui_ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.ui_context.handle(egui_ctx, &mut self.res_mgr);
        self.ui_context.darw(egui_ctx, frame, &mut self.res_mgr);
    }
}
```

优点：所有权清晰、易测试、无全局可变静态。

### 8.2 上下文结构体向下传递

```rust
pub struct EditorContext<'a> {
    pub res_mgr: &'a mut ResourceManager,
    pub device: &'a wgpu::Device,
}

fn draw_viewport(ctx: &mut EditorContext<'_>) {
    let h = ctx.res_mgr.load_resource::<Texture>("...").unwrap();
    let _ = ctx.res_mgr.get(h);
}
```

适合 UI 面板多、不想传递整个 `App` 的场景。Kairos 中可由 `FrameCtx`（§7.4）统一承载。

### 8.3 `Arc<RwLock<ResourceManager>>`（多线程加载）

```rust
let res_mgr = Arc::new(RwLock::new(ResourceManager::new()));
// 后台线程解码，通过 channel 将结果提交到主线程 insert
```

加载可在后台进行，**向 `Vec` 写入应在主线程**（或加锁同步）；`Handle<T>` 在主线程桶内有效。

### 8.4 ECS 集成（若使用 Bevy 或自研 World）

```rust
#[derive(Resource)]
pub struct KairosResMgr(pub ResourceManager);

fn my_system(mut res: ResMut<KairosResMgr>) {
    let _ = res.0.load_resource::<Texture>("...");
}
```

与根状态方案本质相同，由调度器注入 `ResMut`。

### 8.5 全局单例（不推荐作默认）

`OnceLock<Mutex<ResourceManager>>` 仅适合快速原型；测试困难、隐式依赖多。

**Kairos 建议**：§7（`KairosEngine` + 每帧注入 `ui::Context`）；通用场景见 **8.1 + 8.2**；异步 IO 见 **8.3 + 主线程 commit**。

## 9. 扩展话题

### 9.1 Loader 注册

Rust 无法对「未注册的 `T`」自动加载。可选方案：

- **显式注册**（推荐）：启动时对每种 `T` 调用 `register_loader`。
- **链接期收集**：`inventory` crate 或过程宏自动注册（工程量大）。

### 9.2 遍历「所有资源」

纯 `TypeMap` 默认无法在不知 `T` 的情况下遍历所有桶。需要时可：

- 维护 `Vec<TypeId>` 注册表，并为每种 `T` 注册 `visit` 回调；或
- 使用 `enum ResourceKind { Texture(Handle<Texture>), ... }`（枚举会随类型增长）。

### 9.3 GPU 资源分离

`Bucket` 存 CPU 侧 `Texture`（像素或元数据）；`GpuTexture` 可由 `Handle<Texture>` 在渲染阶段延迟创建，缓存于 `HashMap<Handle<Texture>, GpuTextureId>`，避免将 wgpu 对象塞进泛型加载核心路径。

### 9.4 热重载

保留 `path_to_handle` 中的 `Handle`，替换 `dense[index]` 或 bump 世代号，并通知依赖方刷新。

### 9.5 错误分类

`LoadError` 可细分为：IO 失败、解码失败、未注册 loader、扩展名不匹配等。

## 10. 方案对比

| 方案 | 每类型 Vec | `load::<T>` | 多类型存储 | 复杂度 |
|------|------------|-------------|------------|--------|
| **TypeMap + Bucket\<T\>**（本文） | ✅ | ✅ | ✅ | 中，推荐 |
| `enum Asset { Tex, Mesh, ... }` 单容器 | ❌ 混合 | 需 match | 单结构 | 低，不利于按类型遍历 |
| ECS `Assets<T>` 每类型一表 | ✅ | 框架提供 | ✅ | 与 Bevy 一致 |
| `slotmap` 代替 Vec | 稳定 ID | ✅ | 同 TypeMap | 删资源更安全 |

## 11. 小结

1. **存放位置**：`ResourceManager` 放在 `KairosEngine` 等 App 根状态；**Kairos**：与 `ui_context` 并列，每帧 `&mut` 注入 `handle` / `darw` / `Drawer`（§7）。
2. **内部存储**：`TypeId → Box<dyn Any>`，值为 `Bucket<T> { Vec<T>, path_to_handle }`。
3. **对外 API**：`register_loader::<T>` + `load_resource::<T>(path) -> Handle<T>` + `get(handle) -> &T`。
4. **关键实践**：使用 `Handle` 而非长期 `&T`；每类型注册 Loader；路径级去重在 `Bucket` 内完成；控制流走 `Messager`，加载走 `res_mgr` 参数。

## 12. 后续可选方向

- 异步加载：staging 状态 + 主线程 `commit` 进 `Bucket`。
- 与 ECS 的 `Assets<T>` 对齐，避免双份资源表。
- 资源依赖图（加载材质前先加载贴图）。
