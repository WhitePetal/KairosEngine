---
Status: accepted
---

# 资产元数据统一到 .meta 边车（RON），退役每类型 TOML wrapper

## 背景与决策

`kairos_asset` 正照 `bevy_asset` 0.19.1 重写（map #184）。旧栈把「loader 选择 + settings」塞进资产自己的文件里（`.audio` 的 `meta.source_path` + settings、`.mat` 的 `SerializedMaterial` 等），每个资产类型一套 wrapper。改为照 bevy：每个资产文件旁一个 **`.meta` 边车**（`foo` → `foo.meta`），承载 `AssetMeta`（loader 名 + `Settings` + `AssetAction::{Load, Process, Ignore}`）；`AssetMetaCheck` 默认 `Always`、缺失时回退 `loader.default_meta()`。边车格式选 **RON**：`AssetAction` 是 tagged enum，RON 原生表达，TOML 表达别扭。

## 考虑过的替代

- **保留每类型 TOML wrapper**：与蓝本劈叉，loader/settings 分散、无统一的 `AssetAction` 选择面。
- **`.meta` 但用 TOML**：与 kairos 现有手写风格统一，但 tagged enum 需绕（`action = "load"` + 子表），且与 bevy 生态不同构。

## 影响

- `.audio` 拆成「源音频文件 + `.meta`(settings)」；`.mat` 保留为 loader 自有格式、旁边加 `.meta`。字段级落点归迁移路径 #185。
- `.mesh_bin` / `.texture_bin` 这类「已加工产物」定位为 processed mode 的产物；`AssetProcessor` 本体延后（#196），迁移期现有手动加工管线原样保留。
- `.meta` 里的 loader 名以 `std::any::type_name` 为准（`Asset` 已去 `TypePath`，见 #188）。
