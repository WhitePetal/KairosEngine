# editor-ui 架构图（prototype 重构后）

本目录是 `prototypes/editor-ui/docs/architecture-tour.md` 的配图，共 7 张 PNG。
所有图都由同目录的 `render_diagrams.py` 生成（不联网、不装包），颜色/字体/页眉一致。

| 文件 | 内容 |
|---|---|
| `01-crates.png` | Crate 分层与依赖方向（Cargo 强制）；标注 components 不能 use wgpu、core 无 egui |
| `02-objects.png` | 运行时对象与所有权：EditorApp / App / PanelHost / HostState / Frame / UiOut（owns vs borrows） |
| `03-frame-phases.png` | 一帧的五个阶段：logic → build_ui（可能多 pass）→ dispatch → apply_effect → present |
| `04-draw-path.png` | **关键图**：面板被画出来的唯一调用链（PanelHost::draw → draw_slot → draw_panel_body → Panel::draw） |
| `05-message-path.png` | 交互消息路径：egui 输入 → Action → UiOut → pump → reducer → Effect → Host → complete；附 3 个真实示例 |
| `06-viewport-lifecycle.png` | 一个 ViewportId 的状态机：none / pending_bind / bound / pending_pick / bind_failed |
| `07-code-map.png` | 「我要找 X 看哪里」代码地图：8 个问题入口 → 文件 / 函数 |

## 重新生成

在仓库根目录执行（任何 cwd 均可，脚本按自身路径输出）：

```bash
python3 prototypes/editor-ui/docs/diagrams/render_diagrams.py
```

只重画其中几张：

```bash
python3 prototypes/editor-ui/docs/diagrams/render_diagrams.py 04-draw-path 06-viewport-lifecycle
```

依赖：macOS 自带的 `/usr/local/bin/dot`（graphviz 14.x）、python3 + matplotlib + Pillow；
不需要 mmdc / mermaid，也不需要网络。每张图固定 1800px 宽、白底，源图先按 2x 渲染再
LANCZOS 缩到 1720px 内容宽，最后加统一页眉/页脚。

## 字体

- 中文：**Hiragino Sans GB**（`/System/Library/Fonts/Hiragino Sans GB.ttc`，W3/W6）
- 代码标识符 / 文件路径 / 类型名：**Menlo**（`/System/Library/Fonts/Menlo.ttc`）
- matplotlib 里用 family 回退链（`[Menlo, Hiragino Sans GB]`）保证一行内中文与等宽代码都不出豆腐块；
  graphviz 的 HTML-like label 里用逐段 `<FONT FACE=...>` 达到同样效果。

## 说明

- 图中出现的类型/函数名对应当前 prototype 代码（`crates/editor-app`、`editor-components`、
  `editor-core`、`editor-viewport`、`ui-widgets`）；代码改了以后重跑脚本即可同步（图的内容定义在
  `render_diagrams.py` 的 `diag_01` … `diag_07` 里）。
- 只改本目录；不涉及 prototype 源码、crates/ 或 git。
