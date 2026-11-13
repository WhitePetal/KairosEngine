# Tuanjie / Unity editor window icons — dock tab headers

Extracted **2026-11-13** from the Tuanjie editor installed on this machine, for the
Kairos editor's dock tab headers.

* Editor: **Tuanjie 1.8.4** (Unity 2022.3.62t6 fork) —
  `/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app`
* Source asset: `Contents/Resources/tuanjie editor resources` (312 MB serialized
  resource file; the stock Unity 2022.3.62f1 bundle contains the same assets under
  `Contents/Resources/unity editor resources`).
* Skin: dark + light variants are separate textures. The dark skin uses the
  `d_` prefix and is already pre-coloured **#C4C4C4** (196,196,196), which is
  exactly what the running editor draws in its tab strip; light-skin textures are
  dark grey. The dock therefore paints these with a `WHITE` tint for both skins.
* Size: every icon has a **16×16** texture and a **`@2x` 32×32** texture. The
  runtime files ship the 32 px artwork and paint it into a 16 pt tab icon slot,
  so hi-dpi displays get 1 device pixel per source pixel.

## Extracted icons

`icons/` holds the original PNGs under their asset names (`dark` = `d_` texture,
`light` = unprefixed texture). These files are provenance/reference only; the
editor loads the runtime copies below.

| Window | Unity asset (dark / light) | Runtime file | Shown in |
|---|---|---|---|
| Scene | `d_UnityEditor.SceneView` / `UnityEditor.SceneView` | `Preferences/Textures/tab_icons/scene.png` | Scene tab |
| Hierarchy | `d_UnityEditor.SceneHierarchyWindow` / `UnityEditor.SceneHierarchyWindow` | `Preferences/Textures/tab_icons/hierarchy.png` | Hierarchy tab |
| Inspector | `d_UnityEditor.InspectorWindow` / `UnityEditor.InspectorWindow` | `Preferences/Textures/tab_icons/inspector.png` | Inspector tab |
| Console | `d_UnityEditor.ConsoleWindow` / `UnityEditor.ConsoleWindow` | `Preferences/Textures/tab_icons/console.png` | Console tab |
| Project | `d_Project` / `Project` | `Preferences/Textures/tab_icons/project.png` | Project tab |

Light-skin runtime copies live under `Preferences/Textures/tab_icons/light/` with
the same file names. Each panel names its own file from `Panel::icon`: the Console
reads `ConsoleStyle::tab_icon` (serialized in `Preferences/Styles/ConsoleStyle.ron`),
and the four not-yet-styled demo panels keep a module-local `TAB_ICON` const beside
their title.

Notes:

* `UnityEditor.HierarchyWindow` is the obsolete standalone Hierarchy window icon
  (16×14); the hierarchy window Unity 2022 actually docks is
  `SceneHierarchyWindow`, so that is the icon used.
* The Project tab icon is a plain folder; `Project`/`d_Project` is the window
  icon texture (there is no `UnityEditor.ProjectBrowser` texture).
* A `@2x` variant exists for every icon used above; the runtime copies are the
  `@2x` PNGs copied byte-for-byte.

## Extraction recipe

```bash
python3.10 -m pip install --target /tmp/unitypy UnityPy   # 1.25.x
PYTHONPATH=/tmp/unitypy python3.10 - <<'PY'
import UnityPy
env = UnityPy.load(
    "/Applications/Tuanjie/Hub/Editor/2022.3.62t6/"
    "Tuanjie.app/Contents/Resources/tuanjie editor resources"
)
wanted = {
    "d_UnityEditor.SceneView@2x", "UnityEditor.SceneView@2x",
    "d_UnityEditor.SceneHierarchyWindow@2x", "UnityEditor.SceneHierarchyWindow@2x",
    "d_UnityEditor.InspectorWindow@2x", "UnityEditor.InspectorWindow@2x",
    "d_UnityEditor.ConsoleWindow@2x", "UnityEditor.ConsoleWindow@2x",
    "d_Project@2x", "Project@2x",
}
for obj in env.objects:
    if obj.type.name != "Texture2D":
        continue
    name = obj.peek_name()
    if name in wanted:
        obj.read().image.save(f"out/{name}.png")
PY
```

On macOS the pip-installed native wheels may need a Python binary whose hardened
runtime accepts them; the extraction above was run with `/usr/local/bin/python3.10`
(a framework Python) because the DSH-bundled Python rejects the pre-signed wheels.
The project is only read, never modified.

The 16×16 textures (`d_...` without `@2x`) are captured in `icons/` too so the
mapping can be re-checked without re-running the extraction.

## Placement in the editor

Each panel owns its tab icon via `Panel::icon` (Console from its style, the demo
panels from a local const) and preloads it in `Panel::init`. `PanelHostViewer`
converts that panel-owned path to a `file://` URI and hands an
`egui::ImageSource` to the docking crate, which paints it 16×16 to the left of
the tab title (see the shared tab-header helpers in
`kairos_editor_docking/src/docker.rs`). There is deliberately no host-side icon
table: the panel is the single owner of its tab identity, and the same seam
covers per-instance icons such as document tabs.

## Implemented result

[`kairos-tab-icons-implemented.png`](kairos-tab-icons-implemented.png) — the Kairos
editor with the icons wired in: Hierarchy, Scene, Inspector and Console tabs show
their Tuanjie window icons at 16 pt (captured at 2× backing scale, 2026-11-13).
The Project panel returns its icon through the same `Panel::icon` seam; it appears
when the Project panel is opened.

Tab widths grow with the icon slot now: icon left padding 8 px, icon → title gap
6 px (`TabStyle::horizontal_padding` / `icon_spacing`), and the icon is tinted
with the tab's per-state `icon_tint` (the dark artwork is already #C4C4C4, so the
default white tint leaves it untouched).

