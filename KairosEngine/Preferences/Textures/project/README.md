# Preferences/Textures/project/

Project 面板（`kairos_editor_component/src/panels/project`）左树的行图标。

| 文件 | 用途 | 来源（Tuanjie 2022.3 编辑器资源包） |
|---|---|---|
| `folder.png` | 收起/叶子文件夹 | `d_Folder Icon`（dark skin） |
| `folder_open.png` | 展开的文件夹 | `d_FolderOpened Icon`（dark skin） |
| `file.png` | 文件（M2 用泛型图标；按类型图标 M5 加） | `d_DefaultAsset Icon`（dark skin） |
| `eye_open.png` | 工具栏「隐藏文件」开：隐藏项显示 | `d_scenevis_visible@2x`（dark skin） |
| `eye_crossed.png` | 工具栏「隐藏文件」关：dot 文件/`.meta` 被过滤 | `d_scenevis_hidden@2x`（dark skin） |
| `light/*.png` | 上面五张的 light skin 版 | `Folder Icon` / `FolderOpened Icon` / `DefaultAsset Icon` / `scenevis_visible@2x` / `scenevis_hidden@2x` |

展开/收起三角不在这里：它由 `kairos_editor_widgets::tree` 按 `TreeStyle` 直接绘制（矢量、跟 DPI 走），
不占纹理。

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

**provenance**：这些图像是 Unity/Tuanjie 编辑器自带素材，不是本项目原创。
