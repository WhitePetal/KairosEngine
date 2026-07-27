# Material 动态属性系统 — Phase 5：实现排期

## 5.1 阶段总览

```
Phase 1（基础设施）
  [1a naga 反射]  [1b 数据模型]  [1c reload()]
         │              │              │
         └──────┬───────┘              │
                ▼                      │
Phase 2（核心逻辑）
           [2a 协调器]                │
                │                      │
           ┌────┴────┐                 │
           ▼         ▼                 │
     [2b BindGroup] [2c 序列化]        │
           │         │                 │
           └────┬────┘                 │
                ▼                      │
Phase 3（编辑器功能）
           [3a 元数据] ←───────────────┘
                │            [3b FileWatcher]
                ▼                 │
           [3c Inspector UI] ←────┘
                │
                ▼
Phase 4（收尾）
     [4a API] [4b 测试] [4c 清理]
```

## 5.2 Phase 1：基础设施（可并行）

**目标**：WGSL 反射能力就绪 + Material 数据结构就绪

| # | 子任务 | 文件 | 操作 | 预估 |
|---|--------|------|------|------|
| 1a | naga 反射模块 | `kairos_engine/src/graphics/shader_reflection.rs` | **新建** | 1 session |
| 1b | Material 数据模型扩展 | `kairos_engine/src/graphics/material.rs` | 修改 | 0.5 session |
| 1c | AssetsServer::reload() | `kairos_engine/src/asset_loader/assets/asset.rs` | 修改 | 0.5 session |

### 1a 详情

- 实现 `extract_material_properties(source: &str) -> Vec<ShaderProperty>`
- 实现 `map_scalar_type(member_ty) -> ShaderPropertyType`
- 实现 `map_texture_type(dim) -> ShaderPropertyType`
- 集成到 `ShaderAssetsSystem::Loader`：加载时自动解析并缓存到 `ShaderAsset.properties`

### 1b 详情

- `Material` 增加 `properties: HashMap<String, MaterialProperty>`
- `SerializedMaterial` 增加 `properties: HashMap<String, SerializedProperty>` + `textures: HashMap<String, PathBuf>`
- 删除 `Material::texture`、`SerializedMaterial::texture_path`

### 1c 详情

- `Assets::reload()`：强制重新加载，version 不变，modify_count 在加载完成后自动更新

### 验收

- WGSL 加载后 ShaderAsset 包含正确的属性列表
- Material 结构体支持动态属性读写
- reload 触发重新加载 + modify_count 递增

## 5.3 Phase 2：核心逻辑（串行）

| # | 子任务 | 文件 | 操作 | 预估 |
|---|--------|------|------|------|
| 2a | 属性协调器 | `kairos_engine/src/graphics/material.rs` | 修改 | 0.5 session |
| 2b | 动态 Bind Group | `kairos_engine/src/graphics/render_pipeline.rs` | 修改 | 1.5 session |
| 2c | 序列化格式 | `kairos_engine/src/graphics/material.rs` + `res/materials/*.mat` | 修改 | 0.5 session |

### 2a 详情

- `Material::reconcile_with_shader(shader: &ShaderAsset)`：同名同类型保留，其余用默认值
- dirty 标记不自动清除
- 在 Material 加载和 modify_count 变更时调用

### 2b 详情

- 添加 `encase` 依赖到 `kairos_engine/Cargo.toml`
- 替换 `create_texture()` 为 `create_material_bind_group()`
- 根据 `ShaderAsset.properties` 动态构建 `BindGroupLayoutDescriptor`
- 使用 `encase` 按 std140 布局写入 uniform buffer
- `MaterialBindGroupKey` + `MaterialBindGroupCache` + `HashMap` 缓存
- 纹理属性：复用现有 `TextureAssetsSystem` 的纹理创建逻辑

### 2c 详情

- serde 支持 `[material.properties]` + `[material.textures]`
- 迁移现有 `res/materials/` 下的 `.mat` 文件

### 验收

- Material 加载后自动创建 bind group 并正确渲染
- `.mat` 文件读写新格式

## 5.4 Phase 3：编辑器功能（可部分并行）

| # | 子任务 | 文件 | 操作 | 预估 |
|---|--------|------|------|------|
| 3a | 属性元数据解析 | `kairos_engine/src/graphics/shader_reflection.rs` | 扩展现有 | 0.5 session |
| 3b | FileWatcher | `kairos_editor/src/file_watcher.rs` | **新建** | 0.5 session |
| 3c | Inspector 动态 UI | `kairos_editor/src/ui/inspector/material.rs` | 修改 | 1.5 session |

### 3a 详情

- 正则提取 `// @range(0,1) @color @group "X"` 注释
- 输出 `HashMap<String, PropertyMetadata>`

### 3b 详情

- 添加 `notify` 依赖到 `Cargo.toml`（workspace 级别或 kairos_editor）
- `FileWatcher::new(root_dir)` 启动监听
- `FileWatcher::poll()` 在 `Engine::update()` 中调用
- debounce 100ms 防抖
- 按扩展名路由：`.wgsl` → `reload::<Shader>`、`.texture_bin` → `reload::<Texture>`

### 3c 详情

- 在现有 RenderState 编辑区下方新增动态属性面板
- `ShaderPropertyType → egui widget` 映射表
- 属性元数据驱动控件样式（Slider vs DragValue、ColorPicker、分组）
- Shader 变更时自动 reconcile + 保留 dirty + 刷新 UI
- 数值属性编辑同上：修改即写入运行时 Material（编辑即生效）

### 验收

- Zed 中修改 `.wgsl` → Inspector 自动刷新属性列表
- Slider/ColorPicker/DragValue 按元数据正确渲染
- 属性值修改 → Preview 实时更新

## 5.5 Phase 4：收尾

| # | 子任务 | 预估 |
|---|--------|------|
| 4a | 运行时 API | 0.5 session |
| 4b | 集成测试 | 0.5 session |
| 4c | 清理 + 文档 | 0.5 session |

### 4a 详情

- `Material::set_float` / `get_float`
- `Material::set_vec4` / `get_vec4`
- `Material::set_texture` / `get_texture`
- 静默失败 + `log::warn!`

### 4b 详情

- Rust 集成测试：naga 反射覆盖常见 Shader、TOML 序列化往返
- Kairos Test Harness TOML 测试：渲染管线 bind group 创建、Inspector 交互

### 4c 详情

- 删除已废弃字段
- 迁移 `res/` 下所有 `.mat` 文件
- 更新 `CONTEXT.md`

## 5.6 工作量估算

| Phase | Session 数 | 可并行度 |
|-------|-----------|----------|
| Phase 1 | 2（并行为 1） | 1a/1b/1c 可并行 |
| Phase 2 | 2.5 | 串行（2a → 2b + 2c） |
| Phase 3 | 2.5（并行为 2） | 3a + 3b 可并行，3c 串行 |
| Phase 4 | 1.5 | 4a/4b/4c 可并行 |
| **总计** | **约 7-8 sessions** | |

## 5.7 风险矩阵

| 风险 | 影响 | Phase | 缓解 |
|------|------|-------|------|
| naga 解析复杂 Shader 失败 | 高 | 1a | 项目现有 Shader 做 CI 测试；失败降级为空属性 |
| encase std140 布局特殊组合失败 | 中 | 2b | 先覆盖 f32/vec4（90%场景），逐步扩展 |
| RenderPipeline 改造引入渲染回归 | 高 | 2b | Phase 2 完成后立即引擎中实际渲染验证 |
| FileWatcher 平台兼容性 | 低 | 3b | notify 成熟稳定；macOS/Linux 分别验证 |
| 属性协调丢失用户数据 | 中 | 2a | 同名同类型严格保留；仅类型不匹配时丢数据 + 日志 |

## 5.8 里程碑

| 里程碑 | Phase 完成 | 可演示内容 |
|--------|-----------|-----------|
| M1: 反射就绪 | Phase 1 | 单元测试验证 WGSL → ShaderProperty[] |
| M2: 渲染通路 | Phase 2 | 引擎正常渲染，bind group 动态创建 |
| M3: 编辑器完整 | Phase 3 | Inspector 动态 UI + FileWatcher 实时同步 |
| M4: 交付 | Phase 4 | 全功能可用 + 测试覆盖 + 文档 |
