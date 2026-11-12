#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Regenerate the seven architecture diagrams for the editor-ui prototype.

Usage (from anywhere):
    python3 prototypes/editor-ui/docs/diagrams/render_diagrams.py
    python3 render_diagrams.py            # when cwd is this directory
    python3 render_diagrams.py 04-draw-path 07-code-map   # subset

Outputs 01-crates.png ... 07-code-map.png next to this file (1800 px wide,
white background). Requirements: graphviz 'dot', python3 + matplotlib +
Pillow, and macOS system fonts (Hiragino Sans GB for Chinese, Menlo for code).
No network access is used.
"""
from __future__ import annotations

import logging
import shutil
import subprocess
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import font_manager  # noqa: E402
from matplotlib.patches import (  # noqa: E402
    Circle,
    FancyArrowPatch,
    FancyBboxPatch,
)
from PIL import Image, ImageDraw, ImageFont  # noqa: E402

# --------------------------------------------------------------------------
# paths / constants
# --------------------------------------------------------------------------
HERE = Path(__file__).resolve().parent
OUT = HERE

FINAL_W = 1800          # every delivered PNG is exactly this wide
MARGIN = 40             # left/right white margin
CONTENT_W = FINAL_W - 2 * MARGIN   # 1720 px of drawing area
HEADER_H = 96
BOTTOM_H = 34

DOT = shutil.which("dot") or "/usr/local/bin/dot"
HIRAGINO_TTC = "/System/Library/Fonts/Hiragino Sans GB.ttc"
MENLO_TTC = "/System/Library/Fonts/Menlo.ttc"
ARIAL_UNICODE = "/Library/Fonts/Arial Unicode.ttf"

# --------------------------------------------------------------------------
# fonts
# --------------------------------------------------------------------------
for _path in (HIRAGINO_TTC, MENLO_TTC, ARIAL_UNICODE):
    if Path(_path).exists():
        try:
            font_manager.fontManager.addfont(_path)
        except Exception:  # pragma: no cover - older matplotlib
            pass

logging.getLogger("matplotlib.font_manager").setLevel(logging.ERROR)

# family lists act as fallback chains: first font wins for glyphs it has.
F_CJK = ["Hiragino Sans GB", "Arial Unicode MS", "Heiti SC", "sans-serif"]
F_MONO = ["Menlo", "Hiragino Sans GB", "monospace"]
F_SANS = F_CJK

# graphviz font names (single face per run; mixed rows use inline FONT tags)
GV_CJK = "Hiragino Sans GB"
GV_MONO = "Menlo"

# --------------------------------------------------------------------------
# palette -- one colour per architectural area, shared by every diagram
# --------------------------------------------------------------------------
WHITE = "#FFFFFF"
INK = "#1F2937"
MUTED = "#6B7280"
FAINT = "#9CA3AF"
RULE = "#E5E7EB"
DOTGRID = "#F3F4F6"

PAL = {
    "app":        {"fill": "#E8F1FC", "edge": "#2F5DA8", "accent": "#2F5DA8"},
    "components": {"fill": "#E7F5EE", "edge": "#2E7D5B", "accent": "#2E7D5B"},
    "viewport":   {"fill": "#F2ECFB", "edge": "#6B46A8", "accent": "#6B46A8"},
    "widgets":    {"fill": "#FDF3E3", "edge": "#B26A00", "accent": "#B26A00"},
    "core":       {"fill": "#FCEBEC", "edge": "#A63D40", "accent": "#A63D40"},
    "host":       {"fill": "#E9F4F8", "edge": "#2C6E82", "accent": "#2C6E82"},
    "extern":     {"fill": "#F4F5F7", "edge": "#8A9099", "accent": "#6B7280"},
    "note":       {"fill": "#FFF8DC", "edge": "#C9A227", "accent": "#C9A227"},
    "highlight":  {"fill": "#FFE8B3", "edge": "#D97706", "accent": "#D97706"},
    "danger":     {"fill": "#FDECEA", "edge": "#C0392B", "accent": "#C0392B"},
    "neutral":    {"fill": "#FFFFFF", "edge": "#9CA3AF", "accent": "#6B7280"},
}

EDGE_OWN = "#5B6673"
EDGE_BORROW = "#2F5DA8"
EDGE_DONE = "#2E7D5B"
EDGE_ERR = "#C0392B"


def c(name):
    return PAL[name]


# --------------------------------------------------------------------------
# graphviz helpers
# --------------------------------------------------------------------------
def gv_span(text, face=GV_CJK, size=14, color=INK, bold=False):
    """One inline font run for an HTML-like label."""
    text = (text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))
    b = "<B>" if bold else ""
    e = "</B>" if bold else ""
    return ('<FONT FACE="%s" POINT-SIZE="%s" COLOR="%s">%s%s%s</FONT>'
            % (face, size, color, b, text, e))


def gv_row(html_runs, align="CENTER"):
    return '<TR><TD ALIGN="%s">%s</TD></TR>' % (align, html_runs)


def gv_label(rows, cellpadding=3):
    """HTML-like label string (with the outer double angle brackets)."""
    body = "".join(rows)
    return ('<<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" '
            'CELLPADDING="%d">%s</TABLE>>' % (cellpadding, body))


def _has_cjk(text):
    return any("\u4e00" <= ch <= "\u9fff" or "\u3000" <= ch <= "\u303f"
               or "\uff00" <= ch <= "\uffef" for ch in text)


def gv_edge_label(lines, size=12.5):
    rows = [gv_row(gv_span(t, GV_CJK if _has_cjk(t) else GV_MONO, size))
            for t in lines]
    return gv_label(rows, cellpadding=1)


def render_dot(name, src):
    """Render a graphviz source at 2x the target width, then compose."""
    dot_path = OUT / (".%s.dot" % name)
    probe = OUT / (".%s.probe.png" % name)
    raw = OUT / (".%s.raw.png" % name)
    dot_path.write_text(src, encoding="utf-8")
    try:
        subprocess.run([DOT, "-Tpng", "-Gdpi=96", str(dot_path), "-o", str(probe)],
                       check=True, capture_output=True)
        with Image.open(probe) as p:
            w96 = p.width
        dpi = max(120, min(600, round(96 * (CONTENT_W * 2) / w96)))
        subprocess.run([DOT, "-Tpng", "-Gdpi=%d" % dpi, str(dot_path), "-o", str(raw)],
                       check=True, capture_output=True)
        compose(name, raw)
    finally:
        for p in (dot_path, probe, raw):
            try:
                p.unlink()
            except OSError:
                pass


# --------------------------------------------------------------------------
# header / page composition
# --------------------------------------------------------------------------
def _pil_font(path, size, index=0):
    return ImageFont.truetype(path, size, index=index)


def _cjk_pil(size, bold=False):
    # face 0 = W3, face 2 = W6
    return _pil_font(HIRAGINO_TTC, size, index=2 if bold else 0)


def _mono_pil(size, bold=False):
    return _pil_font(MENLO_TTC, size, index=1 if bold else 0)


DIAGRAM_TITLES = {
    "01-crates": "Crate 分层与依赖方向（Cargo 强制）",
    "02-objects": "运行时对象与所有权：谁拥有谁、谁借用谁",
    "03-frame-phases": "一帧的五个阶段（frame phases）",
    "04-draw-path": "面板到底在哪里被画出来？（唯一绘制调用链）",
    "05-message-path": "交互消息怎么走：Action → Effect → 完成回调",
    "06-viewport-lifecycle": "一个 Viewport 的生命周期（状态机）",
    "07-code-map": "我要找 X，应该看哪里？（代码地图）",
}
DIAGRAM_PALETTE = {
    "01-crates": PAL["app"],
    "02-objects": PAL["host"],
    "03-frame-phases": PAL["app"],
    "04-draw-path": PAL["components"],
    "05-message-path": PAL["core"],
    "06-viewport-lifecycle": PAL["viewport"],
    "07-code-map": PAL["widgets"],
}


def compose(name, raw_png):
    """Scale content to CONTENT_W, add the uniform header + footer, save."""
    number = name.split("-")[0]
    accent = DIAGRAM_PALETTE[name]["accent"]
    title = DIAGRAM_TITLES[name]

    src = Image.open(raw_png).convert("RGB")
    scale = CONTENT_W / src.width
    content = src.resize((CONTENT_W, max(1, round(src.height * scale))),
                         Image.LANCZOS)
    height = HEADER_H + content.height + BOTTOM_H
    canvas = Image.new("RGB", (FINAL_W, height), WHITE)
    canvas.paste(content, (MARGIN, HEADER_H))
    draw = ImageDraw.Draw(canvas)

    # accent badge with diagram number
    draw.rounded_rectangle([MARGIN, 26, MARGIN + 54, 80], radius=12, fill=accent)
    draw.text((MARGIN + 27, 54), number, font=_mono_pil(26, True),
              fill=WHITE, anchor="mm")

    draw.text((MARGIN + 72, 53), title, font=_cjk_pil(30, True),
              fill=INK, anchor="lm")
    draw.text((FINAL_W - MARGIN, 55),
              "KairosEditor · prototypes/editor-ui（重构后）",
              font=_cjk_pil(16), fill=MUTED, anchor="rm")
    draw.line([MARGIN, HEADER_H - 13, FINAL_W - MARGIN, HEADER_H - 13],
              fill=RULE, width=2)

    draw.text((FINAL_W - MARGIN, height - 16),
              "generated by docs/diagrams/render_diagrams.py · graphviz + matplotlib",
              font=_mono_pil(11), fill=FAINT, anchor="rs")

    out_path = OUT / ("%s.png" % name)
    canvas.save(out_path, format="PNG", optimize=True)
    print("  wrote %s  (%dx%d)" % (out_path.name, canvas.width, canvas.height))


# ==========================================================================
# 01 -- crate layering / dependency arrows
# ==========================================================================
def diag_01():
    def node(nid, rows, area, shape="box", penwidth=1.8):
        return ('  %s [shape=%s, style="rounded,filled", '
                'fillcolor="%s", color="%s", penwidth=%s, label=%s];\n'
                % (nid, shape, c(area)["fill"], c(area)["edge"], penwidth,
                   gv_label(rows)))

    rows_app = [
        gv_row(gv_span("editor-app-prototype", GV_MONO, 18, bold=True)),
        gv_row(gv_span("宿主 / composition root", GV_CJK, 14, MUTED)),
        gv_row(gv_span("main.rs · app/{mod,pipeline,effects,shell}.rs", GV_MONO, 11, FAINT)),
    ]
    rows_comp = [
        gv_row(gv_span("editor-components-prototype", GV_MONO, 16.5, bold=True)),
        gv_row(gv_span("面板框架 / dock / Frame / UiOut", GV_CJK, 13, MUTED)),
        gv_row(gv_span("panel/host.rs · frame.rs · *_panel.rs", GV_MONO, 11, FAINT)),
    ]
    rows_view = [
        gv_row(gv_span("editor-viewport-prototype", GV_MONO, 16.5, bold=True)),
        gv_row(gv_span("offscreen 渲染 / 封装 wgpu", GV_CJK, 13, MUTED)),
        gv_row(gv_span("host.rs · renderer.rs · camera.rs", GV_MONO, 11, FAINT)),
    ]
    rows_wid = [
        gv_row(gv_span("ui-widgets-prototype", GV_MONO, 16.5, bold=True)),
        gv_row(gv_span("纯 egui 控件 / 不懂面板", GV_CJK, 13, MUTED)),
        gv_row(gv_span("atoms.rs · compounds.rs · theme.rs", GV_MONO, 11, FAINT)),
    ]
    rows_core = [
        gv_row(gv_span("editor-core-prototype", GV_MONO, 16.5, bold=True)),
        gv_row(gv_span("唯一状态树 + 纯 reducer", GV_CJK, 13, MUTED)),
        gv_row(gv_span("无 egui / 无 wgpu / 无 workspace 依赖", GV_CJK, 12, MUTED)),
    ]
    rows_ext = [
        gv_row(gv_span("eframe / egui", GV_MONO, 15, bold=True)),
        gv_row(gv_span("egui-wgpu / wgpu", GV_MONO, 15, bold=True)),
        gv_row(gv_span("bytemuck", GV_MONO, 15, bold=True)),
        gv_row(gv_span("外部 crate（只允许在这里出现）", GV_CJK, 11.5, FAINT)),
    ]
    rows_note = [
        gv_row(gv_span("约束由 Cargo 强制", GV_CJK, 15.5, INK, bold=True)),
        gv_row(gv_span("components ", GV_MONO, 13) +
               gv_span("不能 use wgpu", GV_CJK, 13, MUTED)),
        gv_row(gv_span("core ", GV_MONO, 13) +
               gv_span("无 egui / 无 wgpu", GV_CJK, 13, MUTED)),
        gv_row(gv_span("app ", GV_MONO, 13) +
               gv_span("是唯一同时认识 UI 与 GPU 的 crate", GV_CJK, 13, MUTED)),
    ]

    src = """digraph crates {
  graph [rankdir=TB, bgcolor="white", pad="0.18", nodesep="0.42",
         ranksep="0.62", splines="spline"];
  node [fontname="%s", margin="0.10,0.06"];
  edge [color="%s", penwidth=1.6, arrowsize=0.75,
        fontname="%s", fontsize=12];

  %s
  %s
  %s
  %s
  %s
  %s
  %s

  app -> comp;
  app -> view;
  app -> wid;
  app -> core;
  app -> ext;

  comp -> wid;
  comp -> core;
  comp -> ext;

  view -> ext;
  view -> wid;
  wid -> ext;

  { rank=same; app; }
  { rank=same; comp; view; wid; }
  { rank=same; core; ext; note; }
  ext -> note [style=invis, constraint=false, weight=10];

  graph [label=%s, labelloc="b", fontsize=13];
}
""" % (
        GV_CJK, EDGE_OWN, GV_CJK,
        node("app", rows_app, "app"),
        node("comp", rows_comp, "components"),
        node("view", rows_view, "viewport"),
        node("wid", rows_wid, "widgets"),
        node("core", rows_core, "core"),
        node("ext", rows_ext, "extern"),
        node("note", rows_note, "note", shape="note", penwidth=1.3),
        gv_label([
            gv_row(gv_span("箭头 = 依赖方向：依赖方 → 被依赖方；全部由 Cargo.toml 强制", GV_CJK, 13, MUTED)),
            gv_row(gv_span("core 没有任何出边", GV_CJK, 12.5, MUTED)),
        ]),
    )
    return src


# ==========================================================================
# 06 -- viewport lifecycle state machine
# ==========================================================================
def diag_06():
    def state(nid, area, rows):
        return ('  %s [shape=box, style="rounded,filled", '
                'fillcolor="%s", color="%s", penwidth=2.0, label=%s];\n'
                % (nid, c(area)["fill"], c(area)["edge"], gv_label(rows)))

    def edge(a, b, short, color=EDGE_OWN, constraint=True, dashed=False,
             dir_both=False):
        attrs = ['color="%s"' % color, "label=%s" % gv_edge_label([short])]
        if not constraint:
            attrs.append("constraint=false")
        if dashed:
            attrs.append("style=dashed")
        if dir_both:
            attrs.append("dir=both")
        return "  %s -> %s [%s];\n" % (a, b, ", ".join(attrs))

    def m(text):
        return gv_span(text, GV_MONO, 12)

    def z(text):
        return gv_span(text, GV_CJK, 12)

    def item(num, *runs):
        return gv_span(num + "  ", GV_MONO, 12.5, MUTED) + "".join(runs)

    def legend_table(cells):
        rows = []
        for i in range(0, len(cells), 2):
            rows.append('<TR><TD ALIGN="LEFT">%s</TD><TD ALIGN="LEFT">%s</TD></TR>'
                        % (cells[i], cells[i + 1]))
        return rows

    s_none = [
        gv_row(gv_span("none", GV_MONO, 18, bold=True)),
        gv_row(gv_span("ViewportState 中没有这个 id", GV_CJK, 13, MUTED)),
        gv_row(gv_span("未打开 / 已销毁", GV_CJK, 12, FAINT)),
    ]
    s_bind = [
        gv_row(gv_span("pending_bind", GV_MONO, 18, bold=True)),
        gv_row(gv_span("已发 CreateViewport，等 host 回报", GV_CJK, 13, MUTED)),
        gv_row(gv_span("bound=false · pending_bind=Some(t)", GV_MONO, 11.5, FAINT)),
    ]
    s_bound = [
        gv_row(gv_span("bound", GV_MONO, 18, bold=True)),
        gv_row(gv_span("host 已 bind，texture 可显示", GV_CJK, 13, MUTED)),
        gv_row(gv_span("bound=true · last_error=None", GV_MONO, 11.5, FAINT)),
    ]
    s_pick = [
        gv_row(gv_span("pending_pick", GV_MONO, 18, bold=True)),
        gv_row(gv_span("拾取请求在途（最多一个）", GV_CJK, 13, MUTED)),
        gv_row(gv_span("pending_pick=Some(t)", GV_MONO, 11.5, FAINT)),
    ]
    s_err = [
        gv_row(gv_span("bind_failed", GV_MONO, 18, bold=True)),
        gv_row(gv_span("最后一次 bind 失败", GV_CJK, 13, MUTED)),
        gv_row(gv_span("last_error=Some(err) · bound=false", GV_MONO, 11, FAINT)),
    ]

    cells = [
        item("1)", m("Ensure(w, h)"), z(" 首帧：分配 ticket →"),
             m(" Effect::CreateViewport")),
        item("5)", m("Pick(ndc)"), z(" → "), m("Effect::PickViewport"),
             z("（ticket "), m("PickSceneObject"), z("）")),
        item("2)", m("ViewportBound"), z(" → "), m("App::complete(ticket)")),
        item("6)", m("SceneObjectPicked"), z(" → "),
             m("selection.selected_scene_object"), z(" 更新")),
        item("3)", m("ViewportBindFailed"), z(" → "), m("last_error = Some(err)")),
        item("7)", z("尺寸变化 "), m("Ensure"), z(" → "),
             m("Effect::SyncViewportSize")),
        item("4)", z("重试 "), m("Ensure"), z(" → 新 ticket")),
        item("8)", m("ClosePanel"), z(" → "), m("Effect::DestroyViewport"),
             z(" → "), m("none")),
    ]
    legend_rows = [
        '<TR><TD COLSPAN="2" ALIGN="LEFT">%s</TD></TR>'
        % gv_span("转换细节（编号 = 边上的短标签）", GV_CJK, 13, INK, bold=True),
    ] + legend_table(cells) + [
        '<TR><TD COLSPAN="2" ALIGN="LEFT">%s</TD></TR>'
        % gv_span("注意：resize 时 host 重新注册 texture，TextureId 可能变化 —— 原型可能空白一帧",
                  GV_CJK, 12, MUTED),
    ]

    src = """digraph lifecycle {
  graph [rankdir=LR, bgcolor="white", pad="0.20", nodesep="0.34",
         ranksep="0.55", splines="spline",
         label=%s, labelloc="b", fontsize=13];
  node [fontname="%s", margin="0.14,0.08"];
  edge [fontname="%s", fontsize=12.5, penwidth=1.7, arrowsize=0.8];

  %s
  %s
  %s
  %s
  %s

  %s
  %s
  %s
  %s
  %s
  %s

  { rank=same; pending_bind; bind_failed; }
  { rank=same; bound; pending_pick; }
}
""" % (
        gv_label(legend_rows),
        GV_CJK, GV_CJK,
        state("none", "extern", s_none),
        state("pending_bind", "app", s_bind),
        state("bound", "components", s_bound),
        state("pending_pick", "widgets", s_pick),
        state("bind_failed", "danger", s_err),
        edge("none", "pending_bind", "1) Ensure(w, h)"),
        edge("pending_bind", "bound", "2) ViewportBound"),
        edge("pending_bind", "bind_failed", "3) bind 失败 / 4) 重试",
             color=EDGE_ERR, constraint=False, dashed=True, dir_both=True),
        edge("bound", "pending_pick", "5) Pick(ndc) / 6) SceneObjectPicked",
             color=EDGE_DONE, constraint=False, dir_both=True),
        edge("bound", "bound", "7) SyncViewportSize"),
        edge("bound", "none", "8) ClosePanel",
             constraint=False, dashed=True),
    )
    return src


# ==========================================================================
# 02 -- runtime objects & ownership (graphviz tree, owns / borrows)
# ==========================================================================
def diag_02():
    def node(nid, area, title, desc, mono_size=14.5, shape="box"):
        rows = [gv_row(gv_span(title, GV_MONO, mono_size, bold=True)),
                gv_row(gv_span(desc, GV_CJK, 11.5, MUTED))]
        return ('  %s [shape=%s, style="rounded,filled", fillcolor="%s", '
                'color="%s", penwidth=1.7, margin="0.10,0.055", label=%s];\n'
                % (nid, shape, c(area)["fill"], c(area)["edge"], gv_label(rows)))

    nodes = "".join([
        node("app", "app", "EditorApp", "组合根：core · panels · host · ui_out · frame_count"),
        node("core", "core", "core: App", "state + commands + outbox"),
        node("state", "core", "EditorState", "唯一状态树（纯数据）"),
        node("cmds", "core", "CommandRegistry", "工具栏 / 菜单 / 快捷键共享"),
        node("panels", "components", "panels: PanelHost", "registry + instances + active"),
        node("registry", "components", "PanelRegistry", "PanelId -> DockSlot + factory"),
        node("instances", "components", "instances: BTreeMap<PanelId, PanelInstance>",
             "每个打开的 kind 一个实例"),
        node("active", "components", "active: BTreeMap<DockSlot, PanelId>",
             "每个 slot 的活动面板"),
        node("instance", "components", "PanelInstance",
             "id · panel: Box<dyn Panel> · slot"),
        node("boxdyn", "components", "Box<dyn Panel>",
             "ProjectPanel / InspectorPanel / ConsolePanel / ViewportPanel / GalleryPanel"),
        node("host", "host", "host: HostState", "viewport: Option<ViewportHost> + log"),
        node("vhost", "viewport", "ViewportHost", "offscreen target + texture 注册"),
        node("ventry", "viewport", "ViewportEntry", "target · camera · selected"),
        node("uiout", "widgets", "ui_out: UiOut", "Vec<Action> 帧动作缓冲"),
        node("frame", "note", "Frame<'a>", "&EditorState + &dyn HostView（只读借用）"),
        node("hostview", "note", "trait HostView", "viewport_texture / console_lines"),
    ])

    own = EDGE_OWN
    borrow = EDGE_BORROW
    src = f'''digraph objects {{
  graph [rankdir=LR, bgcolor="white", pad="0.15", nodesep="0.30",
         ranksep="0.55", splines="spline",
         label={gv_label([
             gv_row(gv_span("实线 = owns（拥有 / 持有）；虚线 = borrows（借用 &）", GV_CJK, 13, MUTED)),
             gv_row(gv_span("面板实例由 PanelHost 拥有；面板只拿到 &Frame 和 &mut UiOut", GV_CJK, 12.5, MUTED)),
         ])}, labelloc="b", fontsize=13];
  node [fontname="{GV_CJK}"];
  edge [fontname="{GV_CJK}", fontsize=12, penwidth=1.5, arrowsize=0.7];

  {nodes}

  app -> core [color="{own}"];
  app -> panels [color="{own}"];
  app -> host [color="{own}"];
  app -> uiout [color="{own}"];

  core -> state [color="{own}"];
  core -> cmds [color="{own}"];

  panels -> registry [color="{own}"];
  panels -> instances [color="{own}"];
  panels -> active [color="{own}"];
  instances -> instance [color="{own}"];
  instance -> boxdyn [color="{own}"];

  host -> vhost [color="{own}"];
  vhost -> ventry [color="{own}"];

  uiout -> frame [style=dashed, color="{borrow}", arrowhead=none];
  frame -> state [style=dashed, color="{borrow}"];
  frame -> hostview [style=dashed, color="{borrow}"];
  hostview -> vhost [style=dotted, color="{borrow}", arrowhead=none];
  panels -> frame [style=dashed, color="{borrow}", constraint=false];

  {{ rank=same; state; frame; }}
  {{ rank=same; vhost; hostview; }}
}}
'''
    return src


# ==========================================================================
# 03 -- one frame: sequence of the five phases (matplotlib)
# ==========================================================================
LIFELINES = [
    ("eframe", "宿主窗口", "extern"),
    ("EditorApp / pipeline", "帧阶段实现", "app"),
    ("PanelHost", "面板宿主", "components"),
    ("具体 Panel", "只读 Frame", "widgets"),
    ("App / reducer", "唯一写 state", "core"),
    ("Host", "ViewportHost / IO", "viewport"),
]


def diag_03():
    H = 1000

    def draw(ax):
        xs = [170, 420, 650, 880, 1120, 1430]
        head_y = 970
        head_h = 70
        for (name, role, area), x in zip(LIFELINES, xs):
            pal = c(area)
            rbox(ax, x - 108, head_y - head_h, 216, head_h, pal["fill"],
                 pal["edge"], lw=1.8, radius=10)
            txt(ax, x, head_y - 24, name, mono=True, size=12.5, weight="bold",
                color=pal["edge"], ha="center")
            txt(ax, x, head_y - 48, role, size=10.5, color=MUTED, ha="center")
            ax.plot([x, x], [head_y - head_h, 160], color="#D5DBE3", lw=1.2,
                    zorder=0, linestyle=(0, (4, 4)))

        bands = [
            (836, 890, "P1 logic", "app"),
            (560, 832, "P2 build_ui", "components"),
            (455, 548, "P3 dispatch", "core"),
            (300, 443, "P4 effects", "viewport"),
            (196, 288, "Present", "extern"),
        ]
        for y0, y1, label, area in bands:
            pal = c(area)
            ax.add_patch(FancyBboxPatch((112, y0), 1588, y1 - y0,
                         boxstyle="round,pad=0,rounding_size=8",
                         linewidth=0, facecolor=pal["fill"], alpha=0.35, zorder=0))
            txt(ax, 86, (y0 + y1) / 2, label, size=10.5, weight="bold",
                color=pal["edge"], rotation=90, ha="center", va="center")

        step = [0]

        def num(y):
            step[0] += 1
            ax.add_patch(Circle((140, y), 12, facecolor=INK, edgecolor=WHITE,
                                lw=1.2, zorder=8))
            txt(ax, 140, y - 1, str(step[0]), mono=True, size=10, color=WHITE,
                ha="center", va="center", zorder=9)

        def msg(y, x1, x2, label, color=EDGE_OWN, dashed=False):
            num(y)
            arrow(ax, [(x1, y), (x2, y)], color, lw=1.7, dashed=dashed, ms=12)
            txt(ax, (x1 + x2) / 2, y + 14, label, mono=True, size=11,
                color=INK, ha="center")

        def selfmsg(y, x, label, color=EDGE_OWN):
            num(y)
            ax.annotate("", xy=(x + 34, y), xytext=(x + 4, y + 2),
                        arrowprops=dict(arrowstyle="-|>", color=color, lw=1.7,
                                        shrinkA=0, shrinkB=0, mutation_scale=11,
                                        connectionstyle="arc3,rad=-2.4"),
                        zorder=5)
            txt(ax, x + 46, y, label, mono=True, size=11, color=INK)

        # P1
        selfmsg(876, xs[1], "frame_count += 1")
        msg(846, xs[1], xs[5],
            "render_viewports()：每个 bound viewport → ViewportHost::render")
        # P2
        selfmsg(806, xs[1], "UiOut::reset（每次 pass 从空开始）")
        selfmsg(776, xs[1], "handle_shortcuts / draw_top_bar")
        msg(746, xs[1], xs[2], "panels.sync(&state)")
        msg(716, xs[1], xs[2], "panels.draw(ui, &Frame, &mut UiOut)  ← 单绘制入口")
        msg(686, xs[2], xs[3], "draw_slot → draw_panel_body → Panel::draw")
        msg(656, xs[3], xs[1], "Action::push → UiOut", color=c("widgets")["edge"])
        # P3
        selfmsg(522, xs[1], "if !ctx.will_discard(): ui_out.take()")
        msg(492, xs[1], xs[4], "App::dispatch(action) → Vec<Effect>")
        selfmsg(462, xs[4], "纯 reducer：只有 reducer 写 state")
        # P4
        msg(410, xs[1], xs[5], "apply_effect(effect)：执行 GPU / IO", color=c("viewport")["edge"])
        msg(380, xs[5], xs[4], "EffectOutcome → App::complete(ticket)（同帧）",
            color=EDGE_DONE, dashed=True)
        selfmsg(350, xs[4], "reducer 更新 EditorState")
        # present
        selfmsg(250, xs[1], "request_repaint_after(16ms)（viewport 要连续重绘）")
        msg(218, xs[1], xs[0], "eframe 呈现 egui", color=c("extern")["edge"])

        def note(x, y, w, h, title, lines):
            rbox(ax, x, y, w, h, c("note")["fill"], c("note")["edge"], lw=1.3,
                 radius=8)
            txt(ax, x + 12, y + h - 18, title, size=10.5, weight="bold",
                color="#8A6D1A")
            ty = y + h - 40
            for line in lines:
                txt(ax, x + 12, ty, line, size=9.8, color="#6B5A2A")
                ty -= 17

        note(1545, 700, 172, 148, "egui 可能多 pass",
             ["max_passes = 2", "非最后 pass 的", "UiOut 会被丢弃；", "只在", "!will_discard()", "时才 dispatch"])
        note(1545, 330, 172, 116, "无 GPU 也能测",
             ["PanelHost::draw", "只依赖 Frame；", "host 可传", "EmptyHost"])

        ax.plot([100, 150], [150, 150], color=EDGE_OWN, lw=1.8)
        txt(ax, 160, 150, "调用 / 数据流", size=10.5, color=MUTED)
        ax.plot([320, 370], [150, 150], color=EDGE_DONE, lw=1.8,
                linestyle=(0, (5, 3)))
        txt(ax, 380, 150, "完成回调（同帧进入 reducer）", size=10.5, color=MUTED)
        txt(ax, 100, 118,
            "一帧延迟：camera intent 在 frame N 的 phase 4 应用，frame N+1 的 phase 1 才渲染（与真实引擎一致）",
            size=11.2, color=c("viewport")["edge"])
        txt(ax, 100, 92,
            "ViewportPanel::wants_continuous_repaint = true → build_ui 里 request_repaint_after(16ms)",
            mono=True, size=10.5, color=MUTED)

    render_mpl("03-frame-phases", H / 100.0, draw)


# ==========================================================================
# 04 -- where is a panel drawn? (the key call-path diagram)
# ==========================================================================
def diag_04():
    H = 1000

    def draw(ax):
        def box(x, y, w, h, title, desc, area, mono_size=11.5, highlight=False,
                title_mono=True):
            pal = c("highlight") if highlight else c(area)
            rbox(ax, x, y, w, h, pal["fill"], pal["edge"],
                 lw=(2.6 if highlight else 1.7), radius=10, shadow=True)
            txt(ax, x + 16, y + h - 22, title, mono=title_mono, size=mono_size,
                weight="bold", color=INK)
            dy = y + h - 44
            for line in desc:
                txt(ax, x + 16, dy, line, size=10.2, color=MUTED)
                dy -= 16
            if highlight:
                txt(ax, x + w - 14, y + h - 22, "★ 关键", size=11,
                    weight="bold", color=pal["edge"], ha="right")

        col_a, col_b, col_c = 40, 470, 900
        w, h, gap = 390, 58, 18
        ys = [H - 40 - i * (h + gap) - h for i in range(8)]
        A = col_a + w / 2
        B = col_b + w / 2
        C = col_c + w / 2

        # column A: entry + frame loop
        box(col_a, ys[0], w, h, "main.rs · main()", ["启动原型；组装 eframe"], "app")
        box(col_a, ys[1], w, h, "eframe::run_native", ["eframe 驱动 frame loop"], "extern")
        box(col_a, ys[2], w, h, "EditorApp::new", [
            "注册面板：editor_defaults() + 2 个 host 面板"], "app")
        box(col_a, ys[3], w, h, "EditorApp::logic（phase 1）", ["每帧一次"], "app")
        box(col_a, ys[4], w, h, "pipeline::render_viewports",
            ["每个 bound viewport → ViewportHost::render"], "viewport")
        box(col_a, ys[5], w, h, "EditorApp::ui（phase 2-4）",
            ["每个 UI pass；最后 pass 才 pump()"], "app")
        box(col_a, ys[6], w, h, "pipeline::build_ui",
            ["UiOut::reset · shortcuts · top bar"], "app")

        # column B: app shell
        box(col_b, ys[6], w, h, "app::shell: shortcuts / top bar",
            ["只碰 Action / Command"], "app")
        box(col_b, ys[7], w, h, "panels.sync(&state)",
            ["open_panels → 创建 / 丢弃实例"], "components")
        ys_b3 = ys[7] - (h + gap)
        box(col_b, ys_b3, w, h, "PanelHost::draw",
            ["ui · &Frame · &mut UiOut；app shell 只调这里"], "components", highlight=True)

        # column C: inside editor-components
        cys = [H - 40 - i * (h + gap) - h for i in range(7)]
        box(col_c, cys[0], w, h, "egui::Panel::left/right/bottom",
            ["每 slot 一个 side panel；Center 用 CentralPanel"], "widgets")
        box(col_c, cys[1], w, h, "PanelHost::draw_slot", ["按 slot 取 active panel"], "components")
        box(col_c, cys[2], w, h, "ui-widgets: TabBar::show", ["tab chrome；点击 → LayoutAction"], "widgets")
        box(col_c, cys[3], w, h, "PanelHost::draw_panel_body",
            ["★ 唯一调用 Panel::draw；只画 active panel"], "highlight",
            highlight=True)
        box(col_c, cys[4], w, h, "Box<dyn Panel>::draw", ["动态派发到具体实现"], "components")
        box(col_c, cys[5], w, h, "具体面板实现（5 个 Panel）",
            ["Project / Inspector / Console / Viewport / Gallery"],
            "components", title_mono=False)
        box(col_c, cys[6], w, h, "UiOut ← Action::push", ["view 只能把动作放进 UiOut"], "widgets")

        # arrows: A chain
        def down(x, y1, y2, color=EDGE_OWN):
            arrow(ax, [(x, y1), (x, y2)], color, lw=1.8, ms=12)

        for i in (0, 1, 2):
            down(A, ys[i], ys[i + 1] + h)
        down(A, ys[3], ys[4] + h)
        # A3 -> A6 (ui) via left margin (two phase entry points)
        ax.plot([col_a, 22, 22, col_a], [ys[2] + h / 2, ys[2] + h / 2,
                                         ys[5] + h / 2, ys[5] + h / 2],
                color=EDGE_OWN, lw=1.8, solid_capstyle="round")
        arrow(ax, [(22, ys[5] + h / 2), (col_a, ys[5] + h / 2)], EDGE_OWN, lw=1.8)
        down(A, ys[5], ys[6] + h)
        # A7 build_ui -> B1 shell (horizontal, same row)
        arrow(ax, [(col_a + w, ys[6] + h / 2), (col_b, ys[6] + h / 2)],
              EDGE_OWN, lw=1.8, ms=12)
        down(B, ys[6], ys[7] + h)
        down(B, ys[7], ys_b3 + h)

        # B3 PanelHost::draw -> C1 egui::Panel (elbow between B and C)
        arrow(ax, [(col_b + w, ys_b3 + h / 2), (884, ys_b3 + h / 2),
                   (884, cys[0] + h / 2), (col_c, cys[0] + h / 2)],
              EDGE_OWN, lw=1.8, ms=12)
        # C chain
        for i in range(6):
            down(C, cys[i], cys[i + 1] + h)

        # notes
        def note(x, ytop, w2, lines):
            hh = 24 + 17 * len(lines)
            y = ytop - hh
            rbox(ax, x, y, w2, hh, c("note")["fill"], c("note")["edge"],
                 lw=1.2, radius=7)
            txt(ax, x + 12, ytop - 18, lines[0], size=10.3, weight="bold",
                color="#8A6D1A")
            for i, line in enumerate(lines[1:]):
                txt(ax, x + 12, ytop - 35 - i * 16, line, size=9.8,
                    color="#6B5A2A")

        note(1340, cys[0] + h - 6, 344, [
            "只有 active panel 被绘制",
            "每个 slot 画一个 side panel；",
            "TabBar 报告点击，但只画 active 的 body"])
        note(1340, cys[1] + h - 6, 344, [
            "tab 点击 → Action",
            "TabBar::show 只报告 rect / 点击；",
            "PanelHost 把点击变成 LayoutAction 放进 UiOut"])
        note(1340, cys[3] + h - 6, 344, [
            "这就是答案",
            "app shell 从不调用具体 panel；唯一调用点：",
            "draw_panel_body → <dyn Panel>::draw"])
        note(1340, cys[5] + h - 6, 344, [
            "新增面板不改 host",
            "实现 Panel + PanelRegistry.register 即可；",
            "PanelHost / app shell 代码不变"])

        # bottom legend
        txt(ax, 40, 70,
            "调用方向：main → eframe → EditorApp（每帧 logic/ui）→ build_ui → PanelHost::draw → draw_slot → draw_panel_body → Panel::draw",
            mono=True, size=10.2, color=MUTED)
        txt(ax, 40, 44,
            "关键：app shell 不 import 任何具体 panel 类型；它只认识 PanelHost 和 Panel trait。",
            size=10.8, color=c("components")["edge"])

    render_mpl("04-draw-path", H / 100.0, draw)


# ==========================================================================
# 05 -- interaction / message path: one cycle + three examples
# ==========================================================================
def diag_05():
    H = 1000

    def draw(ax):
        bw, bh, gap = 250, 78, 55
        x0 = 60

        def box(x, y, title, desc, area, w=bw):
            rbox(ax, x, y, w, bh, c(area)["fill"], c(area)["edge"], lw=1.6,
                 radius=10, shadow=True)
            txt(ax, x + 14, y + bh - 22, title, mono=True, size=12,
                weight="bold", color=c(area)["edge"])
            txt(ax, x + 14, y + bh - 44, desc, size=10, color=MUTED)

        def right(x, y):
            return (x + bw, y + bh / 2)

        r1 = 792
        r2 = 648
        r3 = 504
        cols = [x0 + i * (bw + gap) for i in range(5)]
        # row 1 (left -> right)
        box(cols[0], r1, "egui 输入", "键盘 / 鼠标 / 滚轮", "extern")
        box(cols[1], r1, "面板 / 控件绘制", "读 Frame，画 UI", "components")
        box(cols[2], r1, "Action 构造", "FocusPanel / Orbit / Pick", "core")
        box(cols[3], r1, "UiOut", "Vec<Action>（本 pass）", "widgets")
        box(cols[4], r1, "EditorApp::pump", "take actions", "app")
        for i in range(4):
            arrow(ax, [right(cols[i], r1), (cols[i + 1], r1 + bh / 2)],
                  EDGE_OWN, lw=1.7, ms=12)
        # row 2 (right -> left)
        xr = cols[4]
        box(xr, r2, "App::dispatch(action)", "-> Vec<Effect>", "core")
        box(cols[3], r2, "纯 reducer", "只有这里写 state", "core")
        box(cols[2], r2, "Vec<Effect>", "host 工作 = 纯数据", "core")
        box(cols[1], r2, "apply_effect", "GPU / FS / window / dialog", "app")
        box(cols[0], r2, "Host", "ViewportHost / 文件 / 窗口", "viewport")
        arrow(ax, [(cols[4] + bw / 2, r1), (cols[4] + bw / 2, r2 + bh)],
              EDGE_OWN, lw=1.7, ms=12)
        for i in (4, 3, 2, 1):
            arrow(ax, [(cols[i], r2 + bh / 2),
                       (cols[i - 1] + bw, r2 + bh / 2)], EDGE_OWN, lw=1.7, ms=12)
        # row 3 (left -> right)
        box(cols[0], r3, "EffectOutcome", "完成结果（同步 / 异步）", "host")
        box(cols[1], r3, "App::complete(ticket)", "按 ticket 路由 completion", "core")
        box(cols[2], r3, "reducer 更新", "EditorState 变化", "core")
        box(cols[3], r3, "下一帧 Frame", "面板读到新 state", "components")
        box(cols[4], r3, "回到 egui", "present / repaint", "extern")
        arrow(ax, [(cols[0] + bw / 2, r2), (cols[0] + bw / 2, r3 + bh)],
              EDGE_DONE, lw=1.7, ms=12, dashed=True)
        for i in range(3):
            arrow(ax, [(cols[i] + bw, r3 + bh / 2),
                       (cols[i + 1], r3 + bh / 2)], EDGE_DONE, lw=1.7, ms=12)
        # loop back: 回到 egui (row 3 right) -> up right margin -> top -> egui 输入
        loop = [(cols[4] + bw, r3 + bh / 2), (1695, r3 + bh / 2),
                (1695, 968), (cols[0] + bw / 2, 968), (cols[0] + bw / 2, r1 + bh)]
        ax.plot([p[0] for p in loop[:-1]], [p[1] for p in loop[:-1]],
                color="#B6C2D1", lw=1.5, zorder=1, solid_capstyle="round",
                linestyle=(0, (4, 4)))
        arrow(ax, loop[-2:], "#B6C2D1", lw=1.5, ms=12)
        num = [0]

        def badge(x, y, label, color):
            num[0] += 1
            ax.add_patch(Circle((x, y), 13, facecolor=color, edgecolor=WHITE,
                                lw=1.2, zorder=9))
            txt(ax, x, y - 1, str(num[0]), mono=True, size=10.5, color=WHITE,
                ha="center", va="center", zorder=10)

        txt(ax, x0, 912, "① 通用循环：每帧一次单向数据流", size=15,
            weight="bold", color=INK)
        txt(ax, x0 + 460, 912, "所有交互都走同一条路；UI 永远不直接改 state、不直接碰 wgpu",
            size=11.5, color=MUTED)

        # principle banner
        rbox(ax, 60, 452, 1600, 36, c("danger")["fill"], c("danger")["edge"],
             lw=1.2, radius=8)
        txt(ax, 860, 470, "UI 永远不直接改 state / 不直接碰 wgpu —— 只 push Action，由 reducer 决定一切",
            size=12.5, weight="bold", color=c("danger")["edge"], ha="center")

        # examples
        ex = [
            ("a", "点击 tab（布局焦点）", "widgets",
             "PanelHost::draw_slot → LayoutAction::FocusPanel → layout reducer → 下一帧 active tab 生效"),
            ("b", "拖拽 orbit（相机输入）", "viewport",
             "ViewportPanel::handle_input → ViewportAction::Orbit [radians] → viewport reducer → Effect::ViewportCameraInput → ViewportHost::apply_camera_input → 下一帧渲染"),
            ("c", "点击拾取（异步完成）", "core",
             "ViewportAction::Pick(ndc) → RequestKind::PickSceneObject → Effect::PickViewport → host.pick → App::complete(SceneObjectPicked) → selection.selected_scene_object → Inspector"),
        ]
        ey = 390
        for key, title, area, chain in ex:
            pal = c(area)
            rbox(ax, 60, ey - 82, 1600, 94, pal["fill"], pal["edge"], lw=1.5,
                 radius=10)
            ax.add_patch(Circle((92, ey - 35), 15, facecolor=pal["edge"],
                                edgecolor=WHITE, lw=1.2, zorder=9))
            txt(ax, 92, ey - 36, key, mono=True, size=12, color=WHITE,
                ha="center", va="center", zorder=10)
            txt(ax, 120, ey - 22, title, size=13.5, weight="bold",
                color=pal["edge"])
            txt(ax, 120, ey - 56, chain, mono=True, size=10.4, color=INK)
            ey -= 118

        txt(ax, 60, 60,
            "同步完成：apply_effect 里直接调用 App::complete，因此本帧内 state 就更新；异步任务则走 EffectTicket + 下一帧 completion。",
            size=10.8, color=MUTED)

    render_mpl("05-message-path", H / 100.0, draw)


# ==========================================================================
# matplotlib helpers (sequence / annotated diagrams)
# ==========================================================================
def render_mpl(name, fig_h_in, draw_fn):
    """Draw on a 1720 x (fig_h_in*100) canvas, render 2x, then compose."""
    fig = plt.figure(figsize=(CONTENT_W / 100.0, fig_h_in), dpi=200)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.set_xlim(0, CONTENT_W)
    ax.set_ylim(0, fig_h_in * 100)
    ax.axis("off")
    draw_fn(ax)
    raw = OUT / (".%s.raw.png" % name)
    fig.savefig(raw, dpi=200, facecolor=WHITE)
    plt.close(fig)
    try:
        compose(name, raw)
    finally:
        try:
            raw.unlink()
        except OSError:
            pass


def txt(ax, x, y, s, mono=False, size=11, color=INK, weight="normal",
        ha="left", va="center", zorder=5, rotation=0):
    ax.text(x, y, s, family=(F_MONO if mono else F_CJK), fontsize=size,
            color=color, fontweight=weight, ha=ha, va=va, zorder=zorder,
            rotation=rotation)


def rbox(ax, x, y, w, h, fill, edge, lw=1.6, radius=10, zorder=2,
         linestyle="-", shadow=False):
    if shadow:
        sh = FancyBboxPatch((x + 4, y - 4), w, h,
                            boxstyle="round,pad=0,rounding_size=%d" % radius,
                            linewidth=0, facecolor="#E8EAED", zorder=zorder - 1)
        ax.add_patch(sh)
    p = FancyBboxPatch((x, y), w, h,
                       boxstyle="round,pad=0,rounding_size=%d" % radius,
                       linewidth=lw, edgecolor=edge, facecolor=fill,
                       zorder=zorder, linestyle=linestyle)
    ax.add_patch(p)
    return p


def arrow(ax, pts, color, lw=1.8, dashed=False, ms=13, zorder=4):
    ls = (0, (5, 3)) if dashed else "-"
    if len(pts) > 2:
        ax.plot([p[0] for p in pts[:-1]], [p[1] for p in pts[:-1]], color=color,
                lw=lw, zorder=zorder, solid_capstyle="round", linestyle=ls)
    ax.annotate("", xy=pts[-1], xytext=pts[-2],
                arrowprops=dict(arrowstyle="-|>", color=color, lw=lw,
                                shrinkA=0, shrinkB=0, mutation_scale=ms,
                                linestyle=ls),
                zorder=zorder)


def curve_arrow(ax, p1, p2, color, rad=0.1, lw=1.8, ms=13, zorder=3,
                arrowhead=True):
    fa = FancyArrowPatch(p1, p2, connectionstyle="arc3,rad=%s" % rad,
                         arrowstyle="-|>" if arrowhead else "-",
                         mutation_scale=ms, color=color, lw=lw, zorder=zorder)
    ax.add_patch(fa)


def tag(ax, x, y, text, fill, edge, size=10.5, mono=False, color=INK,
        ha="center", va="center", pad=10, zorder=6):
    """Small pill label; returns its width."""
    t = ax.text(x, y, text, family=(F_MONO if mono else F_CJK), fontsize=size,
                color=color, ha=ha, va=va, zorder=zorder + 1)
    bb = t.get_window_extent(renderer=ax.figure.canvas.get_renderer())
    # convert display pixels -> data units (2 render px per data unit, savefig dpi=200)
    f = ax.figure.dpi / 200.0
    wpx = bb.width * f / 2.0
    hpx = bb.height * f / 2.0
    x0 = x - wpx - pad / 2 if ha == "center" else x - pad / 2
    y0 = y - hpx / 2 - pad / 4
    rbox(ax, x0, y0, wpx + pad, hpx + pad / 2, fill, edge, lw=1.2, radius=6,
         zorder=zorder)
    t.set_zorder(zorder + 1)
    return wpx


# ==========================================================================
# 07 -- code map: "where do I look for X?" (mindmap)
# ==========================================================================
def diag_07():
    H = 1020

    cards = [
        # left column -- code structure / how it runs
        dict(col="L", area="app", title="1 · 帧循环 / 入口", rows=[
            ("editor-app/src/app/mod.rs", True, False),
            ("impl eframe::App：logic() / ui()", True, False),
            ("editor-app/src/app/pipeline.rs", True, False),
            ("render_viewports · build_ui · pump", False, True),
        ]),
        dict(col="L", area="components", title="2 · 面板怎么画", rows=[
            ("editor-components/src/panel/host.rs", True, False),
            ("PanelHost::draw / draw_slot", True, False),
            ("PanelHost::draw_panel_body", True, False),
            ("唯一调用 Panel::draw 的地方", False, True),
        ]),
        dict(col="L", area="components", title="3 · 新增一个面板", rows=[
            ("EditorApp::new（app/mod.rs）", True, False),
            ("PanelRegistry.register(kind, slot, factory)", True, False),
            ("新文件 *_panel.rs", True, False),
            ("impl Panel：kind / title / draw", False, True),
        ]),
        dict(col="L", area="widgets", title="4 · 交互 / Action", rows=[
            ("具体面板 *_panel.rs", True, False),
            ("editor-components/src/frame.rs", True, False),
            ("Frame：只读 state · UiOut：收 Action", False, True),
            ("event 只能变成 Action → reducer", False, True),
        ]),
        # right column -- data, GPU, design docs
        dict(col="R", area="core", title="5 · 状态 / reducer / effect", rows=[
            ("editor-core/src/state.rs", True, False),
            ("action.rs · effect.rs", True, False),
            ("editor-core/src/reduce/", True, False),
            ("reducer 是唯一写 state 的地方", False, True),
        ]),
        dict(col="R", area="viewport", title="6 · 3D 视口 / GPU", rows=[
            ("editor-viewport/src/host.rs", True, False),
            ("ViewportHost：bind / render / pick", True, False),
            ("renderer.rs · camera.rs · pick.rs", True, False),
            ("wgpu 只在这里；UI 只见 TextureId", False, True),
        ]),
        dict(col="R", area="note", title="7 · 为什么这样设计", rows=[
            ("docs/adr/0008-editor-ui-architecture.md", True, False),
            ("docs/adr/0009-editor-panel-host-and-viewport.md", True, False),
            ("docs/plan/editor-ui-architecture/", True, False),
            ("panel-viewport-design.md：更细的设计草案", False, True),
        ]),
        dict(col="R", area="host", title="8 · 全流程导读", rows=[
            ("prototypes/editor-ui/docs/architecture-tour.md", True, False),
            ("按顺序读完即懂：入口 → 面板 → 视口", False, True),
            ("配合本目录 01-07 图一起看", False, True),
        ]),
    ]

    def draw(ax):
        # --- geometry
        root_x, root_w, root_h = 560, 600, 92
        root_y = H - 30 - root_h
        col_w, col_gap = 640, 40
        left_x = 110
        right_x = left_x + col_w + col_gap + 220   # 1010
        card_h, card_gap = 180, 18
        top_y = root_y - 50
        trunk_x = 860

        # --- root
        rbox(ax, root_x, root_y, root_w, root_h, c("highlight")["fill"],
             c("highlight")["edge"], lw=2.2, radius=14, shadow=True)
        txt(ax, root_x + 28, root_y + root_h - 33, "我要找 X，看哪里？",
            size=22, weight="bold")
        txt(ax, root_x + 28, root_y + 26, "按问题分支 → 文件 / 函数（从八个入口查）",
            size=12, color=MUTED)

        # --- trunk + branch cards
        centers = []
        for col in ("L", "R"):
            for i, card in enumerate([cd for cd in cards if cd["col"] == col]):
                cy = top_y - i * (card_h + card_gap) - card_h
                cx = left_x if col == "L" else right_x
                centers.append((card, cx, cy))

        # trunk (from root bottom to lowest branch point)
        low_y = min(cy + card_h / 2 for _cd, _cx, cy in centers)
        ax.plot([trunk_x, trunk_x], [root_y, low_y], color="#B6C2D1", lw=2.4,
                zorder=1, solid_capstyle="round")

        for card, cx, cy in centers:
            pal = c(card["area"])
            edge_x = cx + col_w if card["col"] == "L" else cx
            cy_c = cy + card_h / 2
            # branch line from trunk to card edge
            ax.plot([trunk_x, edge_x], [cy_c, cy_c], color=pal["edge"], lw=1.8,
                    zorder=1, solid_capstyle="round")
            ax.add_patch(Circle((trunk_x, cy_c), 5, facecolor=WHITE,
                                edgecolor=pal["edge"], lw=1.8, zorder=2))
            ax.annotate("", xy=(edge_x + (-2 if card["col"] == "L" else 2), cy_c),
                        xytext=(trunk_x + (10 if card["col"] == "L" else -10), cy_c),
                        arrowprops=dict(arrowstyle="-|>", color=pal["edge"],
                                        lw=1.8, shrinkA=0, shrinkB=0,
                                        mutation_scale=14), zorder=2)
            # card
            rbox(ax, cx, cy, col_w, card_h, WHITE, pal["edge"], lw=1.7,
                 radius=12, shadow=True)
            rbox(ax, cx + 5, cy + 8, 7, card_h - 16, pal["edge"], pal["edge"],
                 lw=0, radius=3)
            txt(ax, cx + 26, cy + card_h - 26, card["title"], size=16,
                weight="bold", color=pal["edge"])
            ty = cy + card_h - 62
            for text, mono, muted in card["rows"]:
                txt(ax, cx + 26, ty, text, mono=mono, size=12.2,
                    color=(MUTED if muted else INK))
                ty -= 27

        # --- footer note
        txt(ax, 110, 42,
            "读法：先看问题 → 打开文件 → 对照 01-06 图。代码标识符保持英文。",
            size=12, color=FAINT)

    render_mpl("07-code-map", H / 100.0, draw)


# ==========================================================================
# registry + main
# ==========================================================================
DOT_DIAGRAMS = {
    "01-crates": diag_01,
    "02-objects": diag_02,
    "06-viewport-lifecycle": diag_06,
}
MPL_DIAGRAMS = {
    "03-frame-phases": diag_03,
    "04-draw-path": diag_04,
    "05-message-path": diag_05,
    "07-code-map": diag_07,
}
ALL_NAMES = [
    "01-crates",
    "02-objects",
    "03-frame-phases",
    "04-draw-path",
    "05-message-path",
    "06-viewport-lifecycle",
    "07-code-map",
]


def main(argv):
    OUT.mkdir(parents=True, exist_ok=True)
    wanted = set(argv[1:]) if len(argv) > 1 else set(ALL_NAMES)
    unknown = wanted - set(ALL_NAMES)
    if unknown:
        print("unknown diagram(s): " + ", ".join(sorted(unknown)))
        print("known: " + ", ".join(ALL_NAMES))
        return 2
    for name in ALL_NAMES:
        if name not in wanted:
            continue
        print("[%s] rendering ..." % name)
        if name in DOT_DIAGRAMS:
            render_dot(name, DOT_DIAGRAMS[name]())
        else:
            MPL_DIAGRAMS[name]()
    print("done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
