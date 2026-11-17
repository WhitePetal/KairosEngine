# Tuanjie / Unity Profiler — frame transport icons

The four buttons the Profiler window draws between **Play Mode** and **Frame: n / m**:
the record toggle plus three frame-navigation buttons. Extracted **2026-11-17** from the
editors installed on this machine, as source material for the Kairos Profiler panel
toolbar.

* Editor: **Tuanjie 1.8.4** (Unity 2022.3.62t6 fork) —
  `/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app`
* Containers: `Contents/Resources/tuanjie editor resources` (312,058,888 bytes, 6,334
  `Texture2D` assets) and, for the stock editors,
  `Contents/Resources/unity editor resources` (Unity 2022.3.62f1 / 2021.3.9f1 / 6000.0.37f1).
* The icons are **not** loose PNGs: each is a `Texture2D` inside that serialized resource
  file, extracted with UnityPy (recipe below).

## Which assets the Profiler actually asks for

The toolbar does **not** use the `Profiler.*` textures (those are the stopwatch and the
plain-triangle module icons). Decompiling `UnityEditor.ProfilerWindow+Styles` in
`UnityEditor.CoreModule.dll` with ilspycmd 11.2 shows the real names:

```csharp
profilerRecordOff = EditorGUIUtility.TrIconContent("Record Off", "Record profiling information");
profilerRecordOn  = EditorGUIUtility.TrIconContent("Record On",  "Record profiling information");
prevFrame    = EditorGUIUtility.TrIconContent("Animation.PrevKey", "Previous frame");
nextFrame    = EditorGUIUtility.TrIconContent("Animation.NextKey", "Next frame");
currentFrame = EditorGUIUtility.TrIconContent("Animation.LastKey", "Current frame");
```

`ProfilerWindow.DrawMainToolbar` paints the record toggle
(`GUILayout.Toggle(m_Recording, m_Recording ? profilerRecordOn : profilerRecordOff, EditorStyles.toolbarButton)`)
and `ProfilerWindow.FrameNavigationControls` paints the other three (`GUILayout.Button(Styles.prevFrame, …)`,
`GUILayout.Button(Styles.nextFrame, …)`, `GUILayout.Toggle(…, Styles.currentFrame, …)`), each in an
`EditorGUI.DisabledScope` while there is no frame in that direction.

| Button | Tooltip | Unity asset (dark / light) | Runtime file | Size (1× / @2x) |
|---|---|---|---|---|
| record toggle, idle | Record profiling information | `d_Record Off` / `Record Off` | `Preferences/Textures/profiler/record_off.png` | 16×16 / 32×32 |
| record toggle, recording | Record profiling information | `d_Record On` / `Record On` | `Preferences/Textures/profiler/record_on.png` | 16×16 / 32×32 |
| previous frame (bar + ◀) | Previous frame | `d_Animation.PrevKey` / `Animation.PrevKey` | `Preferences/Textures/profiler/prev_frame.png` | 16×16 / 32×32 |
| next frame (▶ + bar) | Next frame | `d_Animation.NextKey` / `Animation.NextKey` | `Preferences/Textures/profiler/next_frame.png` | 16×16 / 32×32 |
| current-frame toggle (▶▶ + bar) | Current frame | `d_Animation.LastKey` / `Animation.LastKey` | `Preferences/Textures/profiler/current_frame.png` | 16×16 / 32×32 |

Light-skin copies live in `Preferences/Textures/profiler/light/` under the same file names,
mirroring `Preferences/Textures/console/light/` and `Preferences/Textures/tab_icons/light/`.
The runtime copies are the **@2x** PNGs copied byte-for-byte, like the dock tab icons (they
are painted into a 16 pt slot, so a 2× display gets one source pixel per device pixel).

[profiler-transport-icons.png](profiler-transport-icons.png) — the five runtime icons in both
skins, at 4×.

## Notes

* **Colours.** Dark-skin glyphs are `#C4C4C4`; light-skin glyphs are `#555555`. `Record On`
  keeps the same ring colour and puts the red dot `#FF534A` inside it (light skin: same red).
  Because the artwork is already pre-coloured for its skin, the editor paints it with a
  `WHITE` tint, exactly like the tab icons.
* **Alpha.** Transparent pixels are `(0, 0, 0, 0)` — no white matte — so the PNGs tint without
  haloing.
* **`icons/`** holds the untouched extractions for provenance: both skins, 1× and @2x, named
  after the Unity asset (spaces replaced by `_`).
* The stock Unity bundles ship the same five assets at the same sizes; Unity 2022.3.62f1 was
  verified byte-identical to the Tuanjie textures.

## Extraction recipe

```bash
python3.10 -m pip install --target /tmp/unitypy UnityPy   # 1.25.4
PYTHONPATH=/tmp/unitypy python3.10 - <<'PY'
import UnityPy, os
out = "out"
os.makedirs(out, exist_ok=True)
base = ["Record Off", "Record On", "Animation.PrevKey", "Animation.NextKey", "Animation.LastKey"]
wanted = {skin + b + suf for b in base for skin in ("d_", "") for suf in ("", "@2x")}
env = UnityPy.load(
    "/Applications/Tuanjie/Hub/Editor/2022.3.62t6/"
    "Tuanjie.app/Contents/Resources/tuanjie editor resources"
)
for obj in env.objects:
    if obj.type.name != "Texture2D":
        continue
    name = obj.peek_name()
    if name in wanted:
        obj.read().image.save(os.path.join(out, name.replace(" ", "_") + ".png"))
PY
```

To confirm the asset names on a different editor build:

```bash
dotnet tool install --tool-path /tmp/ilspy ilspycmd     # 11.2.0
/tmp/ilspy/ilspycmd -t UnityEditor.ProfilerWindow \
  "/Applications/Unity/Hub/Editor/2022.3.62f1/Unity.app/Contents/Managed/UnityEngine/UnityEditor.CoreModule.dll" \
  | grep -n "IconContent"
```

## Status in Kairos

Nothing consumes these files yet. `Preferences/Styles/Profiler.ron` still names only the tab
icon (`ProfilerIconPaths { tab }`), and `Profiler::ui` in
`kairos_editor_component/src/panels/profiler.rs:231–237` still draws a placeholder `"Recorde"`
button whose icon path is the literal string `"path"`. A toolbar that mirrors Unity's would use:

```text
record toggle (off / on)  Preferences/Textures/profiler/record_off.png | record_on.png
previous frame            Preferences/Textures/profiler/prev_frame.png
next frame                Preferences/Textures/profiler/next_frame.png
current-frame toggle      Preferences/Textures/profiler/current_frame.png
```

## Licence

Local-reference assets extracted from the installed editor, not project assets. Same caveat
as [ADR 0010](../../adr/0010-runtime-logging.md): do not redistribute the Unity-extracted PNGs
without checking Unity's terms.
