# Material Inspector 设计案 — Phase 2：功能分析与排期

## 2.1 需要新建/修改的文件总览

| 文件 | 操作 | 说明 |
|------|------|------|
| `kairos_editor/ui/inspector/material.rs` | **新建** | MaterialInspector 主体 |
| `kairos_editor/ui/inspector.rs` | 修改 | 添加 `pub mod material` |
| `kairos_editor/ui/inspector/creater.rs` | 修改 | `AssetKind::Material` 分支 + 增加 `project_graph` 参数 |
| `kairos_editor/ui/paths.rs` | 修改 | 添加 Material 相关的常量 |
| `kairos_editor/project_path_tree.rs` | 修改 | 添加 `find_assets_by_kind()` 方法 |
| `asset_loader/assets/asset/serialized_material.rs` | **新建** | SerializedMaterialAssetsSystem |
| `asset_loader/assets/asset.rs` | 修改 | 注册 serialized_material 模块 |
| `Preferences/Styles/Inspectors/Material.toml` | **新建** | Style TOML |
| `res/shaders/error_shader.wgsl` | **新建** | 紫色降级 shader |
| `res/textures/white.texture` (+ `white.texture_bin`) | **新建** | 白色降级纹理 |
| `graphics/render_state.rs` | 修改 | 为 BlendState/Face/PrimitiveTopology 添加包装类型 + Display/label |
| `graphics/material.rs` | 修改 | SerializedMaterial 可能需调整（确保与 SerializedMaterialAssetsSystem 兼容） |

## 2.2 功能模块划分

| 编号 | 模块 | 描述 | 前置依赖 |
|------|------|------|----------|
| A | 基础框架 | MaterialInspector 结构体 + Inspector trait 实现 + Style TOML + InspectorCreater 接入 | 无 |
| B | Shader 下拉栏 | 项目 `.wgsl` 查询（通过 ProjectPathGraph）+ ComboBox UI + shader 切换逻辑 | A |
| C | Texture 拖拽输入 | 拖拽目标框 UI + 清除按钮 + texture 路径更新逻辑 | A |
| D | Render State 编辑 | depth_test/depth_write/cull_mode/blend_mode/topology 的 egui 控件 | A |
| E | 持久化与脏标记 | Apply 按钮 + Ctrl+S + dirty 追踪 + 关闭确认对话框 | B, C, D |
| F | 3D Preview | 预览网格下拉栏 + 渲染通道 + orbit/zoom | A, E（编辑即生效机制） |
| G | 降级资源 | 创建 error_shader.wgsl + white.texture + 路径无效时的 fallback 逻辑 | 无（可独立完成） |

## 2.3 模块间依赖关系

```
   G（降级资源）      A（基础框架）
         │                │
         └───┬────────────┘
             │
    ┌────────┼────────┐
    ▼        ▼        ▼
 B(Shader) C(Texture) D(RenderState)
    └────────┬────────┘
             ▼
        E（持久化）
             │
             ▼
        F（Preview）
```

## 2.4 SerializedMaterialAssetsSystem

为了异步加载并缓存 `SerializedMaterial`（.mat 的 TOML 反序列化结果），新建 `SerializedMaterialAssetsSystem`，与现有的 `MaterialAssetsSystem`（存储运行时 `Material`）并行存在：

```
SerializedMaterialAssetsSystem        MaterialAssetsSystem
         │                                   │
         │ 读取同一 .mat 文件                  │ 读取同一 .mat 文件
         │ 反序列化为 SerializedMaterial       │ 构建运行时 Material
         │                                   │    (shader handle + texture handle)
         ▼                                   ▼
  SerializedMaterial                   Material
         │                                   │
         └──────────┬────────────────────────┘
                    ▼
            MaterialInspector
        持有两者的 handle → dirty 标记
        Apply 时: serialized.save_to_file()
```

Loader 实现很简单：`tokio::fs::read` + `toml::from_slice`。

## 2.5 TextureFormat 模式的 wgpu 类型包装

参考 `CompareFunction` 和 `TextureFormat` 的模式，为 RenderState 中需要编辑的 wgpu 类型添加项目级包装：

| 需要包装的类型 | 复杂度 |
|---|---|
| `BlendState`（含 `BlendComponent`、`BlendFactor`、`BlendOperation`） | 复杂——需要预设模式（Replace/Add/Multiply 等）+ Custom 展开子字段 |
| `Face`（cull_mod） | 简单——三个变体：None(不剔除)/Back/Front |
| `PrimitiveTopology` | 简单——几个变体 |

BlendState 编辑交互：
- 下拉栏显示常用预设（Replace / Add / Multiply 等）
- 选中 Custom 时展开子字段（color.srcFactor / color.dstFactor / color.operation / alpha.srcFactor / alpha.dstFactor / alpha.operation）的 dropdown

## 2.6 ProjectPathGraph 扩展

新增 `find_assets_by_kind(&self, kind: AssetKind) -> Vec<&ProjectTreeNode>` 方法，遍历图中所有节点，筛选出指定类型的资产路径。

`InspectorCreater::create_from_asseet_kind` 签名修改为：

```rust
pub fn create_from_asseet_kind(
    asset_kind: AssetKind,
    path: &Path,
    assets_server: &mut AssetsServer,
    project_graph: &ProjectPathGraph,  // 新增
) -> Result<Box<dyn Inspector>, Box<dyn std::error::Error>>
```

MaterialInspector::create() 通过 `project_graph.find_assets_by_kind(AssetKind::Shader)` 获取所有 `.wgsl` 路径缓存到下拉栏。

## 2.7 排期建议

按依赖关系分为三个批次：

| 批次 | 模块 | 预估工作量 |
|------|------|-----------|
| **第一批**（可并行） | A（基础框架）+ G（降级资源） | 1 session |
| **第二批**（可并行） | B（Shader）、C（Texture）、D（RenderState 含 wgpu 类型包装） | 1-2 session 每个，可并行 |
| **第三批** | E（持久化）+ F（Preview） | 1 session |

总计预估：约 4-6 个实现 session（不含设计讨论）。
