---
Status: accepted
---

# 加工布局④：成品与源同目录、一个资产一张 `.meta`

## 背景与决策

现状（ADR 0002 / 0004 / 0005 偏离 12，#239 / #240 落地）把加工产物镜像到另一棵树
`imported_assets/Default/res/…`，于是**一个资产有两张 `.meta`**：

- 源侧 `res/models/Ball.glb.meta`：`AssetAction::Process` + processor + 设置（导入设置）；
- 成品侧 `imported_assets/Default/res/models/Ball.glb.meta`：`AssetAction::Load` + `processed_info`
  （谁来读 + 上次加工到哪个源 hash）。

代价集中在两处：一个资产的相关文件散在两棵树里看不全；两张 `.meta` 的分工必须靠文档解释。

本决策改为**源旁成品**（Godot 的 `.import` 形态）：一个资产 = 同目录下的一簇文件。

```
res/models/Ball.glb          源
res/models/Ball.glb.meta     该资产唯一的 `.meta`：Process(processor, settings) + processed_info
res/models/Ball.mesh         成品字节（靠后缀区分）
```

- **一个 `.meta`**：`AssetMeta::processed_info` 本就是顶层字段，格式不变、`META_FORMAT_VERSION`
  不动；变化只在于 processor 把它写在**源侧**那张，而不是另写一张成品侧边车。因此 processor
  必须 read-modify-write 地只改这一个字段，绝不触碰用户写的 `asset`（导入设置）。
- **后缀区分**：成品用 `.mesh` / `.texture`。**不**沿用 `mesh_bin` / `texture_bin`——那两个是
  loaders 声明的**源格式**（`MeshLoader::extensions()`，`kairos_graphics/src/mesh/test.rs` 里就有
  当成源直接加载的用例）。同目录同后缀会让「这个文件是源还是成品」重新变成歧义，而这正是分树
  规避掉的问题。
- **成品后缀永不作为源**：processor 的初扫与源事件都要跳过它（分树时代这条由
  `unprocessed_exclude` 按目录承担）。
- **半写保护改为原子替换**：布局②用 gated processed reader 挡住半成品；源旁成品没有独立的成品根
  可 gate，改由「写临时文件 + rename 覆盖」保证读者只会看到完整的旧文件或完整的新文件。
- **成品是派生物**：进 `.gitignore`；processor 是它唯一的写者。
- **自动导入**：扫到没有 `.meta`、且被 processor 或 loader 认领的源，就在源旁补一张默认 `.meta`。

## 布局④

`AssetMode` 的两态描述的是 **app server 读哪边**。本决策新增的形态不是第三种 reader，而是
「reader 照布局①（成品就是普通文件）+ processor 在同一棵树上当构建步骤」。因此它由一个与
`AssetMode` 正交的**成品布局**开关表达：

- `ProductLayout::ProcessedRoot`（默认，bevy parity）：成品 = 源相对路径，落在 processed root
  下；记账写在成品侧 `.meta`。
- `ProductLayout::BesideSource`：成品 = 同目录 + `<stem><后缀>`；记账写在源侧 `.meta`；成品后缀
  不作源。

engine 选后者：运行时宿主继续跑布局①，离线 bake 用同一个开关跑 processor。

## 刻意偏离（相对 bevy_asset 0.19.1）

1. **成品路径不是「源路径 + 换根」**：上游写 `processed_writer.write(asset_path.path())`，成品
   路径恒等于源路径；布局④改成「同目录 + 换后缀」。
2. **一个资产一张 `.meta`**：上游源侧与成品侧各一张；布局④把记账与导入设置同居一文件（Godot 的
   `.import` 形态）。上游分开是为了布局③下「只发成品树、包里没有源」时成品仍能自描述；布局④
   放弃了「只发成品树」。
3. **成品后缀不注册为 loader 扩展名**：`.mesh` / `.texture` 只由布局④的命名约定决定；注册成
   loader 扩展名会让成品也能当源，重新引入歧义。
4. **半写保护用原子替换，不用 gated reader**：`ProcessorGatedReader` 以「成品 asset path」为键
   等待；布局④里成品路径与源路径不同、且不在独立的成品根下，gate 不再适用。
5. **成品树在 engine 侧退休**：`imported_assets/Default`、`unprocessed_exclude` 不再出现在
   engine 的布局里；`kairos_asset` 的布局②/③、gated reader、exclude 机制保留（bevy parity，
   别的 source 或 host 可能仍要）。
6. **自动导入**：上游提供 `AssetServer::write_default_loader_meta_file_for_path`（A8）由调用方
   显式触发；布局④下 processor 自己补默认 `.meta`。

## 考虑过的替代

- **保留双树双 meta（现状）**：上游形态，布局③可只发成品；但一个资产的文件散在两处，且要靠文档
  解释两张 `.meta` 的分工。
- **双 meta + 成品同目录**：保住「processor 独占写成品侧、源侧 meta 稳定可提交」这条干净边界；
  代价是每资产 4 个文件，而成品侧边车唯一剩下的价值只是记账。
- **成品进引擎私有缓存（Unity 形态）**：源目录干净、`.meta` 一张；但用户要的是「成品也在源旁」，
  且多一层缓存目录概念。
- **不做加工（布局①直接读源）**：每次加载重跑 glTF 解析，失去「构建期付一次」的全部收益。

## 影响

- `kairos_asset`：`AssetOptions` 增加成品布局开关；`AssetProcessor` 的成品路径、记账写入点、初扫
  跳过规则、事务回滚目标都要按布局分支。**事务回滚必须只删成品、绝不能删源**——分树时代
  `processed_writer.remove(path)` 是安全的，源旁形态下若不分支就会删掉用户的源文件。
- `kairos_engine`：`AssetRegistry::{analyse_path, processed_asset_path, source_path_for}`、
  `AssetKind::related_suffixes`（改名/删除要成对处理「源 + `.meta` + 成品」）与项目树的隐藏规则。
- `res/` 迁移：现有 5 组成品从 `imported_assets/Default` 挪到各自的源旁。
- 打包：源与成品同树，构建脚本必须按后缀排除源；`.gitignore` 排除成品。**放弃「只发成品树」。**

本决策推翻 #239 / #240 落下的成品树布局；`.meta` 边车化本身（ADR 0002）保留。

**落地状态**：决策已定，尚未实现——`res/` 目前仍按旧布局（双树双 `.meta`）提交。实现按切片推进：
成品布局开关 → processor 按布局分支（成品路径 / 记账写入点 / 初扫跳过 / 事务回滚目标）→ engine 的
配对与导入 → 迁移 `res/`。

## 关联

- [ADR 0002](./0002-asset-meta-sidecar-ron.md) — `.meta` 边车（本决策把两张合成一张）
- [ADR 0004](./0004-asset-default-source-root.md) — 源根 = cwd（本决策让 `unprocessed_exclude`
  在 engine 侧退休）
- [ADR 0005](./0005-kairos-asset-rewrite-strategy-and-deviations.md) — 偏离 12（成品子树排除）
- [ADR 0006](./0006-asset-hot-reload-watcher-and-deviations.md) — watcher 与源事件面
