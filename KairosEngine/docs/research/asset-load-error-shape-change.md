# `AssetLoadError` 形状变更登记 — `MissingAssetLoaderForTypeName`

登记 [A5 · loader 门控与异步 loader 查询 #234](https://github.com/WhitePetal/KairosEngine/issues/234)
（commit `2deff3b`）对公开枚举 `AssetLoadError` 的形状改动，供仓库外的消费方
（引擎上层、编辑器、后续消费方）同步。本文件是
[#247](https://github.com/WhitePetal/KairosEngine/issues/247) 的登记/过渡说明产物。

## 变更对照

#234 为对齐 `bevy_asset` 0.19.1，把 loader 缺失面从「同步、只报
`AssetLoadError`、仅 TypeName 一个变体」改成「三个元组变体各包一个专用 error 结构体」。

### 枚举变体

| 项 | #234 之前 | #234 起（对齐 `bevy_asset` 0.19.1） |
|---|---|---|
| loader 名缺失 | `MissingAssetLoaderForTypeName { type_name: String }`（结构体变体） | `MissingAssetLoaderForTypeName(MissingAssetLoaderForTypeNameError)`（元组变体，`#[from]`） |
| loader 扩展名缺失 | 无 | `MissingAssetLoaderForExtension(MissingAssetLoaderForExtensionError)`（元组变体，`#[from]`） |
| loader 类型 id 缺失 | 无 | `MissingAssetLoaderForTypeIdError(MissingAssetLoaderForTypeIdError)`（元组变体，`#[from]`） |

`MissingAssetLoaderForTypeName` 的 `#[error]` 由字段内联改为 `#[error(transparent)]`，
消息文本改由被包的结构体提供，**渲染出的文本与旧版相同**（如
``no `AssetLoader` found with the name 'x'``）——变的只是枚举形状，不是消息。另两个
变体是净新增，无旧文本可比。

### 伴随的查询面变更

同一片把 loader 查询改为 `pub async`，并让返回值收窄到各自的专用 error：

| API | #234 之前 | #234 起 |
|---|---|---|
| `AssetServer::get_asset_loader_with_type_name` | `pub(crate) fn -> Result<_, AssetLoadError>` | `pub async fn -> Result<_, MissingAssetLoaderForTypeNameError>` |
| `AssetServer::get_path_asset_loader` | `pub(crate) fn -> Result<_, AssetLoadError>` | `pub async fn -> Result<_, MissingAssetLoaderForExtensionError>` |
| `AssetServer::get_asset_loader_with_extension` | 无 | `pub async fn -> Result<_, MissingAssetLoaderForExtensionError>` |
| `AssetServer::get_asset_loader_with_asset_type_id` / `..._asset_type` | 无 | `pub async fn -> Result<_, MissingAssetLoaderForTypeIdError>` |

## 破坏性等级

`AssetLoadError` 虽标了 `#[non_exhaustive]`，但该属性只允许新增变体，**不防
「结构体变体 → 元组变体」这类形状破坏**，也不阻止外部按旧形状构造或解构。
因此本次改动须按破坏性变更对待：任何按旧写法
`AssetLoadError::MissingAssetLoaderForTypeName { type_name }` 的匹配或构造都会
编译失败。改动发生在 `kairos_asset` 的公开面；`kairos_engine` 以
`pub use kairos_asset as asset` 暴露该枚举，下游可经 `kairos_engine::asset::AssetLoadError`
触达。

## 全仓核对（验收项 1）

全仓 grep `MissingAssetLoaderForTypeName` 命中 5 处，**全部为新形态，无旧写法**：

| 位置 | 形态 |
|---|---|
| `kairos_asset/src/lib.rs:112` | `pub use` 再导出 error 结构体 |
| `kairos_asset/src/server.rs:302` | `MissingAssetLoaderForTypeNameError { type_name }` 构造（查询返回） |
| `kairos_asset/src/server.rs:1037` | `AssetLoadError::MissingAssetLoaderForTypeName(_)` 匹配（元组，`load_folder_internal` 跳过缺失 loader） |
| `kairos_asset/src/server.rs:1943` | 枚举变体定义 |
| `kairos_asset/src/server.rs:2056` | error 结构体定义 |

补充核对：

- `MissingAssetLoaderForTypeName\s*\{` → 0 命中（旧结构体变体写法绝迹）。
- `AssetLoadError` / `MissingAssetLoader` 在 `kairos_engine`（含内置编辑器）与
  `kairos_graphics` 中 → 0 命中。工作区内只有这两个 crate 依赖 `kairos_asset`
  （见各 `Cargo.toml`），故本仓内的「引擎上层、编辑器」没有需要同步的匹配/构造点。

## 外部消费方过渡说明（验收项 2）

本仓内没有需要同步的消费点；仓库外的下游工程按下列方式迁移。

### 匹配（`match` / `if let`）

```rust
// before
AssetLoadError::MissingAssetLoaderForTypeName { type_name } => { /* type_name: String */ }

// after
AssetLoadError::MissingAssetLoaderForTypeName(error) => {
    let type_name = error.type_name; // 公开字段，String
}
```

### 构造

```rust
// before
AssetLoadError::MissingAssetLoaderForTypeName { type_name: name }

// after（二选一）
AssetLoadError::MissingAssetLoaderForTypeName(
    MissingAssetLoaderForTypeNameError { type_name: name },
)
// 或经 #[from]：
let error: AssetLoadError = MissingAssetLoaderForTypeNameError { type_name: name }.into();
```

### 穷举匹配

`AssetLoadError` 带 `#[non_exhaustive]`，外部 `match` 本就需要 `_` 分支；
从上游同步后 `MissingAssetLoaderForExtension` 与 `MissingAssetLoaderForTypeIdError`
两个新变体也须覆盖或落进 `_`。

### 查询面

`get_asset_loader_with_type_name` / `get_path_asset_loader` 由 `pub(crate)` 同步
改为 `pub async`，返回值不再是 `AssetLoadError` 而是各自的 `Missing*Error`。
调用点需 `.await`；若要把错误并入 `AssetLoadError`，加 `.map_err(AssetLoadError::from)`
（三个结构体都有 `#[from]`）。

## 上游 quirk（照搬，非笔误）

- 变体名 `MissingAssetLoaderForTypeIdError` 带 `Error` 后缀，是上游 0.19.1 的原样命名。
- 该变体与它包裹的结构体**同名**（都叫 `MissingAssetLoaderForTypeIdError`），
  在上游同样如此；不是笔误，阅读时勿按「变体名 = 结构体名」的惯例误判。
- `MissingAssetLoaderForExtensionError.extensions: Vec<String>` 为**私有**字段；无扩展名时
  消息为 ``no `AssetLoader` found for file with no extension``。
- `MissingAssetLoaderForTypeNameError.type_name`（`String`）与
  `MissingAssetLoaderForTypeIdError.type_id`（`TypeId`）为**公开**字段。

## 来源

- #234 · A5 · loader 门控与异步 loader 查询（commit `2deff3b`）
- #247 · 本登记票
- #215 · 延后项决议：AssetServer 其余公开 API 面（A5 设计）
