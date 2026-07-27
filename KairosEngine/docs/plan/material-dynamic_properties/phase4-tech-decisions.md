# Material 动态属性系统 — Phase 4：技术选型与坑点

## 4.1 naga 反射

### 选型

`naga::front::wgsl::parse_str()` 完整解析 WGSL，遍历 `module.global_variables` 提取属性。

当前项目已间接依赖 naga 29.0.3（通过 wgpu 29.0.3），无需新增依赖。

### 解析逻辑

```rust
fn extract_material_properties(source: &str) -> Vec<ShaderProperty> {
    let module = naga::front::wgsl::parse_str(source)?;
    let mut props = Vec::new();

    for (_, var) in module.global_variables.iter() {
        let binding = var.binding.clone()?;
        let ty = &module.types[var.ty];

        match (&ty.inner, var.space) {
            // var<uniform>: PerMaterialCBuffer struct → 展开 members
            (TypeInner::Struct { members, .. }, AddressSpace::Uniform) => {
                for m in members {
                    let member_ty = &module.types[m.ty];
                    props.push(ShaderProperty {
                        name: m.name.clone()?,
                        ty: map_scalar_type(member_ty),
                        binding_group: binding.group,
                        binding_index: binding.binding,
                    });
                }
            }
            // texture_2d<f32> / texture_cube<f32>
            (TypeInner::Image { dim, .. }, _) => {
                props.push(ShaderProperty {
                    name: var.name.clone()?,
                    ty: map_texture_type(dim),
                    binding_group: binding.group,
                    binding_index: binding.binding,
                });
            }
            _ => {}
        }
    }
    props
}
```

### 约定

每个 Shader 有且仅有一个 `var<uniform>`，类型为单层 struct（类似 Unity SRP Batcher 的 `PerMaterialCBuffer`）。

### 坑点

| 坑点 | 缓解 |
|------|------|
| naga 解析失败（WGSL 语法错误） | 返回空属性列表 + 日志，使用 error_shader 降级 |
| 默认值提取复杂 | 统一使用类型零值（0.0, [0,0,0]），后续完善 |
| sampler 不应作为 Material 属性 | 解析时忽略 `TypeInner::Sampler` |

## 4.2 std140 布局与 encase

### 问题

WGSL uniform buffer 遵循 std140 对齐规则，vec3 需要 16 字节对齐（末尾 4 字节 padding）。Rust 原生 struct 的布局与 GPU 布局不一致，不能用 `bytemuck` 直接传递。

### 选型

使用 `encase` crate（Bevy 也用它）：

```rust
use encase::{ShaderType, UniformBuffer};

#[derive(ShaderType)]
struct MaterialUniform {
    roughness: f32,
    metallic: f32,
    // encase 自动插入 8 字节 padding
    albedo: glam::Vec4,
}

let mut buffer = UniformBuffer::new(Vec::new());
buffer.write(&uniform_data).unwrap();
let bytes = buffer.into_inner();
// bytes 符合 std140 布局，可直接写入 wgpu buffer
```

### 性能

`encase` 的开销仅在 Material 属性变更时发生（非每帧），可接受。

## 4.3 Bind Group 创建策略

### 选型

按 Shader 共享 `BindGroupLayout`，按 Material 创建独立 `BindGroup` + `UniformBuffer`。

```
Shader → BindGroupLayout（缓存，shader 不变则复用）
Material → UniformBuffer + BindGroup（属性值变更时重建）
```

### 缓存键

```rust
struct MaterialBindGroupKey {
    material_id: usize,
    material_modify_count: u64,
}

HashMap<MaterialBindGroupKey, MaterialBindGroupCache>
```

与现有 `PipelineKey → PipelineCache` 模式保持一致。

### wgpu 约束

- `BindGroupLayoutEntry` 的 binding index 必须与 WGSL `@binding(N)` 严格一致
- 不能有空缺的 binding index（binding 0 和 2 有定义但 1 没有会报错）

## 4.4 属性元数据

### 选型

WGSL 注释标注（类似 Godot hints），放在变量声明行上方：

```wgsl
// @range(0.0, 1.0) @slider @group "PBR"
roughness: f32,
```

### 理由

- 属性定义和元数据在同一文件，修改 Shader 时自然同步
- 不需要维护独立的配置文件
- 注释在 naga 解析时被忽略，不影响 Shader 编译

### 解析方案

在 naga 解析前，用正则逐行提取注释中的 `@xxx` 标签，关联到后续的变量声明。

## 4.5 序列化格式

### 选型

TOML 扁平属性节，不向后兼容：

```toml
source_path = "res/materials/pbr.mat"
shader_path = "res/shaders/pbr.wgsl"

[render_state]
cull_mod = "Back"

[material.properties]
roughness = 0.5
metallic = 0.0
albedo = [1.0, 0.5, 0.2, 1.0]

[material.textures]
albedo_map = "res/textures/albedo.texture"
```

数值属性和纹理属性分开序列化（纹理存储路径字符串）。

### 迁移

所有现有 `.mat` 文件在实现时一次性迁移到新格式。

## 4.6 Inspector Shader 变更处理

当 Inspector 检测到 Shader `modify_count` 变更时：

1. 协调属性：同名且类型匹配的保留旧值，其余用默认值
2. **保留 dirty 标记**（用户之前的修改不丢失）
3. 刷新 UI 为新属性列表

如果用户 Apply 时属性类型与 Shader 不匹配，静默跳过不兼容的属性并 `log::warn!`。

## 4.7 AssetsServer::reload()

### 选型

新增 `Assets::reload()` 方法，不改变现有 `AssetIndex.version`，仅触发重新加载 + 更新 `modify_count`：

```rust
impl Assets {
    pub fn reload(&mut self, path: &PathBuf, ...) -> Arc<AssetHandle<System>> {
        let index = *self.path_to_index.get(path).expect("reload unloaded path");
        self.storages[index.index] = Entry::Loading { version: index.version };
        self.loader.load_asset(path.clone(), index, ...);
        self.get_asset_handle(index)  // 返回现有 handle
    }
}
```

下游通过比较 `modify_count` 感知变更。

## 4.8 文件监视器

### 选型

`notify` crate（macOS FSEvents / Linux inotify / Windows ReadDirectoryChanges），**仅在 `kairos_editor` 中引入**。

```rust
// kairos_editor/src/file_watcher.rs
pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
}

impl FileWatcher {
    pub fn poll(&mut self, assets_server: &mut AssetsServer) {
        // 按扩展名路由到 AssetsServer::reload()
    }
}
```

### 模块边界

```
kairos_engine:  AssetsServer::reload()  ← 纯 API，无 notify 依赖
kairos_editor:  FileWatcher             ← 依赖 notify，调用 reload()
```

## 4.9 运行时 API 行为

### 选型

静默失败 + `log::warn!`，与 Unity 一致。

```rust
impl Material {
    pub fn set_float(&mut self, name: &str, value: f32) {
        match self.properties.get_mut(name) {
            Some(MaterialProperty::Float(v)) => *v = value,
            Some(_) => log::warn!("Type mismatch for property '{}'", name),
            None => log::warn!("Property '{}' not found in material", name),
        }
    }
}
```

## 4.10 依赖清单

| 依赖 | 版本 | 用途 | 引入位置 |
|------|------|------|----------|
| `naga` | 29.0.3 | WGSL 反射 | 已存在（wgpu 传递依赖），`kairos_engine` |
| `encase` | 新增 | std140 uniform buffer 序列化 | `kairos_engine` |
| `notify` | 新增 | 文件系统监听 | `kairos_editor` |
