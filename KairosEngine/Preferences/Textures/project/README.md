# Preferences/Textures/project/

Project 面板（`kairos_editor_component/src/panels/project`）左树的行图标。

| 文件 | 用途 | 来源（Tuanjie 2022.3 编辑器资源包） |
|---|---|---|
| `folder.png` | 收起/叶子文件夹 | `d_Folder Icon`（dark skin） |
| `folder_open.png` | 展开的文件夹 | `d_FolderOpened Icon`（dark skin） |
| `file.png` | 文件（未注册类型的回退图标；按类型图标见下节） | `d_DefaultAsset Icon`（dark skin） |
| `eye_open.png` | 工具栏「隐藏文件」开：隐藏项显示 | `d_scenevis_visible@2x`（dark skin） |
| `eye_crossed.png` | 工具栏「隐藏文件」关：dot 文件/`.meta` 被过滤 | `d_scenevis_hidden@2x`（dark skin） |
| `light/*.png` | 上面五张的 light skin 版 | `Folder Icon` / `FolderOpened Icon` / `DefaultAsset Icon` / `scenevis_visible@2x` / `scenevis_hidden@2x` |

展开/收起三角不在这里：它由 `kairos_editor_widgets::tree` 按 `TreeStyle` 直接绘制（矢量、跟 DPI 走），
不占纹理。

## 按类型的行图标（M3 / T16）

M2 的 5 张是通用行图标；M3 起文件行按扩展名分到 8 个家族，各用一张自己的图标
（`kairos_editor_core/src/project/file_type.rs` 的 `FileType::icon` 指向它们）。
除 `script` 外的 7 个家族取自本机 **Tuanjie 2022.3.62t6** 的编辑器资源包（7 个家族 dark + light 实测都有），
Lanczos 缩到 64×64，dark 落在 `types/`、light 落在 `types/light/`；`script` 是项目原创（见下）：

| 家族 | 落点（light 版在同级 `light/`） | dark 源名 | light 源名 |
|---|---|---|---|
| texture | `types/texture.png` | `d_Texture Icon` | `Texture Icon` |
| mesh | `types/mesh.png` | `d_PrefabModel Icon` | `PrefabModel Icon` |
| material | `types/material.png` | `d_Material Icon` | `Material Icon` |
| audio | `types/audio.png` | `d_AudioClip Icon` | `AudioClip Icon` |
| shader | `types/shader.png` | `d_Shader Icon` | `Shader Icon` |
| script | `types/script.png` | 项目原创（页面轮廓取自 `d_cs Script Icon`，见下） | 项目原创（页面轮廓取自 `cs Script Icon`，见下） |
| document | `types/document.png` | `d_TextAsset Icon` | `TextAsset Icon` |
| font | `types/font.png` | `d_Font Icon` | `Font Icon` |

- **dark 源名带 `d_` 前缀，light 版就是去掉前缀的同一名字**，7 个家族全部对得上；
  这是编辑器资源包的命名规律，不是本项目另起的名字。`script` 已不取自资源包（见下）。
- `document` 用 `d_TextAsset Icon`：`.md`/`.txt`/`.json` 在 Unity 里就是 TextAsset，
  资源包里没有 `d_Document Icon`。`mesh` 用 `d_PrefabModel Icon`（Project 窗口显示的
  模型图标），`d_Mesh Icon` 是 Inspector 用的另一个。
- 当前主题钉死 Dark（`kairos_editor_app/src/app.rs`），**light 版本先提交、暂未消费**，
  换主题时不用再抽。
- 重新生成用同一个脚本，一次写出通用行图标 + 其余 7 个类型家族图标（`script` 不重生成）：

```sh
python scripts/extract_tuanjie_icons.py --editor "D:/UnityInstalls/2022.3.62t6"
```

依赖安装与 bundle 位置见文末「重新生成」。

### script：项目原创的 Rust 图标

`types/script.png` / `types/light/script.png` 对应 Rust（`.rs`）家族，是项目原创美术：
**页面轮廓与皮肤沿用原 C# 脚本图标**（dark 浅灰页、light 白页 + 灰边；页面像素与原图
Lanczos 到 64 的结果一致），只把页内的斜体 `#` 换成**同角度的斜体几何 `R`**（比 `#` 略小）。R 的颜色**沿用原 C# `#` 的绿**：
dark `#276A2A`（`rgb(39, 106, 42)`）、light `#2E7D32`（`rgb(46, 125, 50)`）——即 Unity/Tuanjie
脚本图标自己的绿，不再用 Rust 品牌橙。

绘制方式与光学参数：页面取自 Tuanjie 256×256 源图；R 在 **1024×1024** 上绘制后 Lanczos
降到 64×64（不直接画 64）。R 与 `#` 一样倾斜 **8.3°（右上→左下，和原 `#` 同向）**；按 256 源图标尺，R 字形高 120
（= 30.0 px@64）、宽 108（= 27 px@64，含斜体位移）、笔画 24（= 6.0 px@64）；64 px 下
字形外框中心 (31.0, 32.5)，原 `#` 为 (30.5, 33.0)——比 `#` 矮约 14%，视觉上更收敛。
R 墨迹面积约为 `#` 的 0.92×；18 px 行内仍可辨认（dark/light 两种皮肤都按 18 px 显示检查过）。

对比度（WCAG 2.x，以页面底色的相对亮度计）：绿对 dark 页 `#F0F0F0` = **5.79:1**，
对 light 页 `#FFFFFF` = **5.13:1**；均高于图形对象 3:1 的下限，也高于正文文本 4.5:1。

**重跑注意**：`scripts/extract_tuanjie_icons.py` 已删除 `d_cs Script Icon` /
`cs Script Icon` 两条，重跑**不会覆盖**这两张 PNG；它们没有自动生成来源，改图请直接替换文件。

## 为什么是 64×64 / 32×32

编辑器资源包里的原图是 256×256–2048×2048，而 `egui_extras` 0.35 的图片加载器**忽略 size hint**
（`ImageCrateLoader::load(..., _: SizeHint)`），也就是 `ui_image::preload` 的 `vec2(32, 32)` 不会降采样：
一张 2048² 图会占 16 MB 纹理。行图标显示尺寸是 18 px，64² 已留够 2×–3× 余量；
工具栏眼睛显示 16 px，取的就是它的 `@2x` 源（32×32，正好 2×），不需要再放大。

## 重新生成

```sh
python -m pip install UnityPy
python scripts/extract_tuanjie_icons.py --editor "D:/UnityInstalls/2022.3.62t6"
```

脚本会读取 `<editor>/Editor/Data/Resources/tuanjie editor resources`
（Unity 安装里是 `unity editor resources`），按名字取出 Texture2D，Lanczos 缩到 64² 写出。
`types/script.png` 与 `types/light/script.png` 不在脚本输出里，重跑不会覆盖它们（见下节）。

**provenance**：除 `types/script.png` / `types/light/script.png`（项目原创 Rust 图标）外，
这些图像是 Unity/Tuanjie 编辑器自带素材，不是本项目原创。
