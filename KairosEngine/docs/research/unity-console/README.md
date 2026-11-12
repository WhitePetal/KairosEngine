# Tuanjie / Unity Console — visual ground truth for the Kairos editor

Captured **2026-11-12** from a running Tuanjie editor on this machine, to give the Kairos
editor rewrite an exact, measurable reference for rebuilding its Console panel.

* Editor: **Tuanjie 1.8.4** (Unity 2022.3.62t6 fork) — `/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app`
* Project open at capture time: `/Users/baiaoxiang/wxgardenagrown_human/Unity` (only the Console window was inspected — **no project asset was changed**)
* Skin: **dark theme**, display **BenQ PD2500Q, 2560×1440 @ 1× (non-Retina)** → every pixel in the captures is **1 UI px = 1 px**. All sizes below are device px at 1×.
* Console was **docked** in the bottom dock area of the editor, next to *Project / Animation / Animator / Timeline / Profiler*.
* Reference shot: [`00_tuanjie_editor_console_window_raw.png`](00_tuanjie_editor_console_window_raw.png) — full 2560×1415 window capture (window id 67020, `screencapture -x -o -l 67020`).

Console state in the reference shot (this is what the measurements describe):

* 8 visible log entries, list scrolled to the bottom; counts **39 info / 303 warning / 5 error**
* **Collapse OFF**, **Show Timestamp ON**, **Log Entry = 2 Lines**, **Error Pause OFF**,
  **Clear on Play / on Build / on Recompile all checked**
* Row 4 selected (blue) → the selected entry's message + full stack trace is rendered in the
  **bottom detail pane** (new-style split console; see §3.4)

---

## 1. Screenshot index

| # | File | Contents |
|---|------|----------|
| 00 | [`00_tuanjie_editor_console_window_raw.png`](00_tuanjie_editor_console_window_raw.png) | **Canonical reference** — whole Tuanjie editor window, 2560×1415, 1× |
| 01 | [`01_console_panel_full.png`](01_console_panel_full.png) | Whole Console panel: tab strip + toolbar + counts + 8 log rows + detail pane (1601×481 crop of 00, 1×) |
| 02 | [`02_tabbar_toolbar_2x.png`](02_tabbar_toolbar_2x.png) | Tab strip + toolbar, 2× nearest-neighbour |
| 02b | [`02b_tabbar_toolbar_firstrows_2x.png`](02b_tabbar_toolbar_firstrows_2x.png) | Tab strip + toolbar + first rows, 2× |
| 03 | [`03_toolbar_buttons_4x.png`](03_toolbar_buttons_4x.png) | Clear ▾ / Collapse / Error Pause / Editor ▾ buttons, 4× |
| 04 | [`04_toolbar_search_counts_4x.png`](04_toolbar_search_counts_4x.png) | Search field + the 3 level-count toggle buttons, 4× |
| 05 | [`05_rows_info_error_2x.png`](05_rows_info_error_2x.png) | Info row, error row, info rows, selected info row, 2× |
| 05b | [`05_row_types_info_error_warning_2x.png`](05_row_types_info_error_warning_2x.png) | One info, one error and one warning row stacked, labelled, 2× |
| 06 | [`06_warning_row_and_stack_pane_2x.png`](06_warning_row_and_stack_pane_2x.png) | Warning row (2 wrapped lines) + the stack-trace detail pane, 2× |
| 07 | [`07_selected_row_stack_pane.png`](07_selected_row_stack_pane.png) | **Selected row (blue) + stack-trace detail pane** with blue file links, 1× |
| 08 | [`08_console_icons_10x.png`](08_console_icons_10x.png) | The three list icons at 10× (info / error / warning) |
| 09 | [`09_tabbar_3x.png`](09_tabbar_3x.png) | Dock tab strip at 3× (Project … Console, own tab icon, ⋮ menu) |
| 10 | [`10_editor_status_bar.png`](10_editor_status_bar.png) | Editor status bar below the console (warning-tinted) |
| 11 | [`11_console_tab_context_menu.png`](11_console_tab_context_menu.png) | **⋮ panel menu**: Open Player Log / Open Editor Log / ✓Show Timestamp / Log Entry ▸ / Use Monospace font / Strip logging callstack / Stack Trace Logging ▸ / Maximize / Close Tab / Add Tab / UI Toolkit … |
| 12 | [`12_menu_clear_options.png`](12_menu_clear_options.png) | **Clear ▾ menu**: ✓Clear on Play / ✓Clear on Build / ✓Clear on Recompile |
| 13 | [`13_menu_log_entry_lines.png`](13_menu_log_entry_lines.png) | **Log Entry ▸ submenu**: 1 Line … 10 Lines, ✓2 Lines |
| 14 | [`14_menu_stack_trace_logging.png`](14_menu_stack_trace_logging.png) | **Stack Trace Logging ▸ submenu**: Error, Assert, Warning, Log, Exception, All ▸ |
| 15 | [`15_menu_player_logging.png`](15_menu_player_logging.png) | **Editor ▾** = *Player Logging* target selector (Player Logging, Full Log, target table, Editor, Direct Connection) |
| 16 | [`16_collapse_on_repeat_badges.png`](16_collapse_on_repeat_badges.png) | **Collapse ON**: per-row repeat-count badges at the right edge; counts become 38 / 174 / 5 |
| 17 | [`17_collapse_badge_8x.png`](17_collapse_badge_8x.png) | Collapse repeat badge, 8× |
| 18 | [`18_toolbar_collapse_toggled_on_4x.png`](18_toolbar_collapse_toggled_on_4x.png) | Toolbar with **Collapse toggled ON** (pressed background #464646), 4× |
| 20 | [`20_console_geometry_annotated.png`](20_console_geometry_annotated.png) | **Annotated geometry diagram** of the whole console panel |
| — | [`icons/`](icons/) | **Extracted real editor icon assets** (PNG, dark + light, 1×/@2x/16/32/64 px) + `_contact_sheet_dark.png` |

---

## 2. Console panel anatomy (window coordinates of shot 00)

```
y = 915..916   2 px  #181818   top border of the docked panel
y = 917..934  18 px  #242424   tab strip (Project | Animation | Animator | Timeline | Profiler | [Console])
y = 935        1 px  #202020   separator
y = 936..955  20 px  #353535   toolbar (Clear ▾ | Collapse | Error Pause | Editor ▾ … search … 3 count toggles)
y = 956        1 px  #202020   separator
y = 957..1260 304 px           log list — 8 rows × 38 px (2-line entries), alternating background
y = 1261       1 px  #202020   separator (list / detail split)
y = 1262..1391 130 px #313131  stack-trace DETAIL PANE for the selected entry
y = 1392       1 px  #202020   separator
y = 1393..1414 22 px #181818   editor-wide status bar (last log line, warning-tinted) — not part of Console
x = 0..1587                    list / toolbar content
x = 1588       1 px  #2A2A2A   scrollbar edge
x = 1589..1600 12 px #2F2F2F   vertical scrollbars (list + detail pane), thumb #545454
x = 1601..1602  2 px #181818   dock splitter
```

Tab strip detail: selected tab rect **x 444..529 (86×18)** with background **#353535** and 1 px **#2E2E2E** edges,
inactive tabs **#242424**; Console tab icon 16×16 at x 452..467, label x 470..511; the ⋮ panel-menu
button sits at the far right (ink x 1587..1588, y 921..930).

---

## 3. Layout spec tables

### 3.1 Tab strip (dock chrome)

| Element | Size | Position | Color |
|---|---|---|---|
| tab strip background | 1601×18 | y 917..934 | `#242424` |
| top border | 1601×2 | y 915..916 | `#181818` |
| bottom separator | 1601×1 | y 935 | `#202020` |
| selected tab (Console) | 86×18 | x 444..529 | `#353535` + 1 px `#2E2E2E` border |
| tab icon | 16×16 | icon left edge = tab x + 8 | `#C4C4C4` (asset `UnityEditor.ConsoleWindow`) |
| tab label | 12 px Inter Regular | icon + 2 px gap | inactive `#BDBDBD`, active `#C0C0C0` |
| tab width | fits label, no fixed width (Console = 86 px) | ink runs: Project 7..62, Animation 89..159, Animator 189..252, Timeline 279..341, Profiler 368..424, Console 452..511 | — |
| ⋮ panel menu | 3 dots, 2 px wide, 10 px tall | x 1587..1588, y 921..930 | light grey `#B5B5B5` (dots render `#989898`/`#B5B5B5`/`#BCBCBC` top→bottom) |

### 3.2 Toolbar

| Element | Size | Position | Color |
|---|---|---|---|
| toolbar background | 1601×20 | y 936..955 | `#353535` |
| **Clear** button (split) | 55×20 | x 1..55 | text `#B1B1B1` 12 px; label ink x 5..33, caret ▼ ink x 46..49 |
| **Collapse** toggle | 58×20 | x 57..114 | text `#B1B1B1`; **ON** background `#464646` (whole button rect) |
| **Error Pause** toggle | 73×20 | x 116..188 | text `#B1B1B1`; ON background `#464646` (same style as Collapse) |
| **Editor ▾** dropdown | 50×20 | x 190..239 | text `#B1B1B1`; caret ink x 230..233 |
| button separators | 1 px | x 56, 115, 189 | toolbar bg `#353535` (no drawn line) |
| search field | 302×17 (r≈3) | x 1174..1475, y 938..954 | fill `#252525`, 1 px edge `#1D1D1D`; magnifier glyph 7×7 at (1179,943); **no placeholder text** |
| count toggle – info | 40×20 | x 1477..1516, y 936..955 | flat **square-cornered** bg `#464646` (1 px `#202020` separator at x 1476), icon 13×13 at (1482,939), number right of icon |
| count toggle – warning | 49×20 | x 1518..1566 | bg `#464646`, icon 13×13 at (1523,939) |
| count toggle – error | 32×20 | x 1568..1599 | bg `#464646`, icon 13×13 at (1573,939) |
| count numbers | 12 px | icon + 5 px gap | `#B1B1B1` (observed `39`, `303`, `5`) |
| count toggle gaps | 1 px | x 1517, 1567 | toolbar bg |

### 3.3 Log list rows

| Element | Value | Notes |
|---|---|---|
| row height | **38 px** (2 lines) | row height scales with the **Log Entry** setting (1–10 lines); 2 lines ⇒ 38 px, i.e. 19 px per line |
| row pitch | 38 px | 8 rows exactly fill 304 px |
| row background | **alternating** `#373737` / `#313131` | verified by pixel runs; the editor exposes an *AlternatingRowBackground* option |
| selected row background | **`#27537C`** | full row width, covers both lines |
| selected row text | **`#F1F5F7`** | slightly blue-tinted near-white |
| normal row text | **`#C0C0C0`** | message, inline stack line and `[HH:mm:ss]` timestamp are all the same color |
| severity icon | **26×26 px** | x = 10, y = rowTop + 6 (vertically centred: 6 px top/bottom) |
| message text left edge | **x = 41** | = 10 (pad) + 26 (icon) + 5 (gap) |
| line 1 text ink | rowTop + 6 … rowTop + 16 | 12 px Inter, cap height 8–9 px |
| line 2 text ink | rowTop + 21 … rowTop + 31 | **line pitch 15 px** |
| text clipping | message wraps, entry clipped to N lines | a 2-line entry shows the message (wrapped) and, if room remains, the first stack frame line; long messages consume both lines (see row 8) |
| timestamp format | `[HH:mm:ss] ` prefix | same color as message; toggled by ⋮ → **Show Timestamp** |
| horizontal scroll | none | rows do not scroll horizontally; long text is clipped at x = 1587 |
| hover | **no visual change** | pointer-over rows produced zero pixel difference (no hover fill) |
| collapse repeat badge | 20×20 round plate, tinted `#545454` | circular badge at the row's right edge (≈ x 1565..1584), number `#C0C0C0`, only when **Collapse** is ON (asset `ConsoleCountBadge`, base `#525252`) |

### 3.4 Stack-trace detail pane (new split view)

| Element | Value |
|---|---|
| rect | x 0..1600, y 1262..1391 → **130 px tall** |
| background | `#313131` |
| borders | 1 px `#202020` above (y 1261) and below (y 1392) |
| text left padding | **3 px** (first glyph ink starts at x = 3) |
| first text line ink | y 1267 (≈ 5 px from pane top) |
| line pitch | **15 px** (same as list) |
| font | 12 px Inter Regular |
| plain text color | `#C0C0C0` |
| file-path links | **`#416EEE`** (e.g. `(at Assets/Scripts/.../DefaultLogHelper.cs:32)`) |
| content | selected entry's **message (no timestamp, no icon) + full stack trace**, unchanged line order |
| own scrollbar | 12 px wide, track `#2F2F2F`, thumb `#545454` (observed thumb y 1279..1365) |

### 3.5 Scrollbars / splitter

| Element | Value |
|---|---|
| scrollbar width | 12 px (x 1589..1600), 1 px edge `#2A2A2A` at x 1588 |
| track | `#2F2F2F` |
| thumb | `#545454`, rounded caps; list thumb observed y 1223..1244 (22 px) |
| pane splitter | 2 px `#181818` at x 1601..1602 between the console dock and the right panel (`#2B2B2B`) |

### 3.6 Editor status bar (below the console, editor-wide — context only)

| Element | Value |
|---|---|
| rect | y 1393..1414 → 22 px tall, full window width |
| background | `#181818` |
| text | 12–14 px, ink y 1397..1411, left padding 3 px |
| color | **`#E0A705`** — the last console message is tinted by log level (warning here); shows the most recent log line |

---

## 4. Color palette (all values sampled from shot 00 unless noted)

| Token | Hex | Where |
|---|---|---|
| `console.row.odd` | `#373737` | odd log rows |
| `console.row.even` | `#313131` | even log rows |
| `console.row.selected` | `#27537C` | selected row background |
| `console.text` | `#C0C0C0` | row message / timestamp / inline stack line |
| `console.text.selected` | `#F1F5F7` | text on the selected row |
| `console.link` | `#416EEE` | clickable file paths in the stack pane |
| `tabbar.bg` | `#242424` | dock tab strip |
| `tab.selected.bg` | `#353535` | selected tab |
| `tab.selected.edge` | `#2E2E2E` | 1 px edge of selected tab |
| `tab.text.inactive` | `#BDBDBD` | inactive tab labels |
| `tab.text.active` | `#C0C0C0` | active tab label |
| `toolbar.bg` | `#353535` | toolbar background |
| `toolbar.text` | `#B1B1B1` | toolbar button labels, count numbers |
| `toolbar.toggle.on` | `#464646` | pressed toggle / count-button background |
| `search.fill` | `#252525` | search input |
| `search.edge` | `#1D1D1D` | search input border |
| `separator` | `#202020` | 1 px separators (tab strip, toolbar, list, pane) |
| `panel.border` | `#181818` | 2 px dock border / splitter |
| `detail.bg` | `#313131` | stack-trace detail pane |
| `scrollbar.track` | `#2F2F2F` | scrollbar track |
| `scrollbar.edge` | `#2A2A2A` | 1 px scrollbar edge |
| `scrollbar.thumb` | `#545454` | scrollbar thumb, collapse badge tint |
| `statusbar.bg` | `#181818` | editor status bar |
| `statusbar.warning` | `#E0A705` | warning-level status text |
| `icon.info` | asset `#F0F0F0` / rendered `#EEEEEE` | info icon fill |
| `icon.warn` | asset `#FFC107` / rendered `#FFB909` | warning icon fill |
| `icon.error` | asset `#FF534A` / rendered `#FF4941` | error icon fill |

> The “asset” values are the raw pixel values of the PNGs in `icons/`; the “rendered” values are what the
> editor actually draws on screen (sRGB/gamma conversion). **Use the rendered values when matching pixels,
> use the assets when drawing.**

---

## 5. Typography

* UI font: **Inter** — `/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app/Contents/Resources/Fonts/Inter-Regular.otf`
  (Medium / SemiBold / Bold also shipped). Monospace option (⋮ → *Use Monospace font*) uses `RobotoMono-Regular.ttf`.
* Console list / detail pane / toolbar / tab labels: **12 px, Regular** (cap height 8–9 px, digit height 9 px).
* Line pitch inside a 2-line log entry: **15 px**; entry height **19 px per line** (2 lines ⇒ 38 px).
* No bold weight is used in the log list; the selected row is distinguished by color only.

---

## 6. Icon inventory — the real assets are extractable ✅

The console icons are **not** loose PNG files in the bundle; they are `Texture2D` assets inside the serialized
resource file. They **were extracted successfully** with UnityPy 1.25.4 and copied here.

**Source (absolute):**
`/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app/Contents/Resources/tuanjie editor resources`
(312,058,888 bytes; 7,919 objects, 50 console-related `Texture2D` assets).
The equivalent stock-Unity bundle also contains them:
`/Applications/Unity/Hub/Editor/2022.3.62f1/Unity.app/Contents/Resources/unity editor resources`.

**Extraction recipe** (for reproducability):

```bash
python3 -m pip install --target /tmp/pylibs UnityPy
PYTHONPATH=/tmp/pylibs python3 - <<'PY'
import UnityPy
env = UnityPy.load("/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app/Contents/Resources/tuanjie editor resources")
for obj in env.objects:
    if obj.type.name != "Texture2D": continue
    name = obj.peek_name()          # e.g. d_console.infoicon
    if "console" in (name or "").lower():
        d = obj.read(); d.image.save(f"out/{name}.png")
PY
```

### 6.1 Copied files (all in [`icons/`](icons/), PNG with alpha, sRGB)

| File | Source asset name | Asset size | Use |
|---|---|---|---|
| `console.infoicon.dark.32x32.png` | `d_console.infoicon` | 32×32 | list row icon (drawn at 26×26) |
| `console.infoicon.dark.64x64.png` | `d_console.infoicon@2x` | 64×64 | hi-dpi list icon |
| `console.warnicon.dark.32x32.png` | `d_console.warnicon` | 32×32 | list row icon |
| `console.warnicon.dark.64x64.png` | `d_console.warnicon@2x` | 64×64 | hi-dpi |
| `console.erroricon.dark.32x32.png` | `d_console.erroricon` | 32×32 | list row icon |
| `console.erroricon.dark.64x64.png` | `d_console.erroricon@2x` | 64×64 | hi-dpi |
| `console.infoicon.sml.dark.16x16.png` | `d_console.infoicon.sml` | 16×16 | count toggle (drawn at 13×13) |
| `console.infoicon.sml.dark.32x32.png` | `d_console.infoicon.sml@2x` | 32×32 | hi-dpi count toggle |
| `console.warnicon.sml.dark.16x16.png` | `d_console.warnicon.sml` | 16×16 | count toggle |
| `console.warnicon.sml.dark.32x32.png` | `d_console.warnicon.sml@2x` | 32×32 | hi-dpi |
| `console.erroricon.sml.dark.16x16.png` | `d_console.erroricon.sml` | 16×16 | count toggle |
| `console.erroricon.sml.dark.32x32.png` | `d_console.erroricon.sml@2x` | 32×32 | hi-dpi |
| `console.*icon.inactive.sml.dark.16x16.png` | `d_console.*icon.inactive.sml` | 16×16 | count toggles **off** (dimmed variant) |
| `ConsoleCountBadge.20x20.png` | `ConsoleCountBadge` | 20×20 | **collapse repeat-count badge** (round 20×20 plate) |
| `ConsoleCountBadge.40x40.png` | `ConsoleCountBadge@2x` | 40×40 | hi-dpi |
| `UnityEditor.ConsoleWindow.dark.16x16.png` | `d_UnityEditor.ConsoleWindow` | 16×16 | Console tab icon |
| `UnityEditor.ConsoleWindow.dark.32x32.png` | `d_UnityEditor.ConsoleWindow@2x` | 32×32 | hi-dpi tab icon |
| `console.infoicon.light.32x32.png` | `console.infoicon` | 32×32 | light skin (reference) |
| `console.warnicon.light.32x32.png` | `console.warnicon` | 32×32 | light skin (reference) |
| `console.erroricon.light.32x32.png` | `console.erroricon` | 32×32 | light skin (reference) |
| `_contact_sheet_dark.png` | — | — | contact sheet of the dark-theme set |

Original Unity asset names follow `d_*` = **dark skin**, `@2x` = hi-dpi, `.sml` = small variant,
`.inactive` = dimmed state (used by the count filters when a level is switched off).

### 6.2 Icon shapes (so they can be redrawn as vectors if preferred)

All three list icons share one design language: a solid coloured silhouette with a **punched-out
exclamation mark** (a rounded vertical bar ≈ 9 % × 45 % of the icon plus a round dot below it) that
lets the row background show through. No outline, no shadow.

* **info** — white/very light rounded **speech bubble**: a circle of ~80 % of the icon width with a
  small triangular tail at the **bottom-right** (tail points down-right, ~8 % of width). Fill `#F0F0F0`
  (rendered `#EEEEEE`). *Note: the mark inside is an exclamation “!”, not an “i”.*
* **warning** — **amber equilateral triangle**, apex up, rounded corners (corner radius ≈ 8 %),
  base spans the full icon width. Fill `#FFC107` (rendered `#FFB909`).
* **error** — **red regular octagon** (flat top/bottom/left/right, the corner chamfers cut ≈ 22 %).
  Fill `#FF534A` (rendered `#FF4941`).
* **collapse count badge** — a `ConsoleCountBadge` 20×20 fully-rounded (circular) plate, base fill
  `#525252`, drawn tinted `#545454` in the list; the number inside is `#C0C0C0`.
  Note: the **toolbar count toggles are NOT drawn with this texture** — they are plain square-cornered
  `#464646` rects, full toolbar height (20 px), separated by 1 px toolbar-bg gaps.

---

## 7. Behaviour notes for the Kairos implementation

Everything below was **observed in the running editor** unless marked *(standard Unity)*.

### 7.1 Toolbar
* **Clear** is a split-style button (55×20): the text part clears the console; the **▼ caret**
  opens a dropdown with three checkable options — **Clear on Play**, **Clear on Build**,
  **Clear on Recompile** (all checked in the reference state, see shot 12). Tooltip of Clear:
  *“Clear console entries”*.
* **Collapse** toggle (58×20): groups identical entries; tooltip *“Collapse identical entries”*.
  ON state = button rect filled `#464646`. While ON, each row shows a circular repeat-count badge
  (`#545454`, 20×20) at its right edge (shot 16/17), and the toolbar counts change to the number of
  *displayed* entries (observed 39/303/5 → **38/174/5**).
* **Error Pause** toggle (73×20): tooltip *“Pause Play Mode on error”*. ON state uses the same
  `#464646` pressed background (shot 18 shows the pressed style on Collapse).
* **Editor ▾** (50×20) is Tuanjie's **Player Logging** target selector (shot 15): *Player Logging*,
  *Full Log [Developer Mode Only]*, a target table (Player Name / Product Name / IP / Port),
  *Editor* (connected), *Direct Connection ▸* (<Enter IP>, 10.30.28.29), *Troubleshoot Connection Issues*.
  Kairos probably only needs this as a generic dropdown-button visual.
* **Search** (302×17, rounded, `#252525`): live filter; empty state shows only the magnifier glyph
  (no “Search…” placeholder). *(Filtering behaviour is standard Unity — typing filters the list live. Not
  exercised in this capture to avoid changing the user's console.)*
* **Level counts** (three 20 px-tall buttons, `#464646`, right-aligned): each is a **filter toggle**
  with icon + count. Info / warning / error counts are the number of entries of that level in the list.
  Disabled state uses the `.inactive` icon variants (extracted in `icons/`).

### 7.2 ⋮ panel menu (the console's settings menu — shot 11)
`Open Player Log` · `Open Editor Log` · `✓ Show Timestamp` · `Log Entry ▸` · `Use Monospace font` ·
`Strip logging callstack` · `Stack Trace Logging ▸` · ― · `Maximize` · `Close Tab` · ― · `Add Tab ▸` ·
― · `UI Toolkit Live Reload` · `UI Toolkit Debugger  ⌥⌘F5`.

* **Log Entry ▸** = **1 Line … 10 Lines**, check-mark on the current value (2 Lines in the reference
  state). This controls the row height: 2 Lines ⇒ 38 px (verified); 1 Line would be 19 px by linearity
  (not directly captured — the setting was left at 2 Lines).
* **Stack Trace Logging ▸** = per-level stack-trace display type: Error, Assert, Warning, Log,
  Exception, All — each opens a submenu with Unity's `StackTraceLogType` values
  (None / ScriptOnly / Full; these are the standard Unity options, not individually re-captured).
* **Show Timestamp** adds the `[HH:mm:ss] ` prefix (verified by toggling it off and on: the prefix
  disappears / reappears, colour unchanged).
* Matching localisation strings confirm the same feature set in
  `/Applications/Tuanjie/Hub/Editor/2022.3.62t6/Tuanjie.app/Contents/Localization/zh-hans.po`
  (`Editor/Mono/ConsoleWindow.cs:60–69, 1063–1066`):
  `Clear`/`Clear console entries`, `Clear on Play`, `Clear on Build`, `Clear on Recompile`,
  `Collapse`/`Collapse identical entries`, `Error Pause`/`Pause Play Mode on error`,
  `Stop for Assert`, `Stop for Error`, `Use Monospace font`, `Strip logging callstack`,
  `Open Player Log`, `Open Editor Log`, `Show Timestamp`.
  `AlternatingRowBackground` also appears in `UnityEditor.dll`, confirming the alternating-row feature.

### 7.3 Log list
* One log entry = stacked lines, height `LogEntryLines × 19 px`; with **2 Lines** the entry shows the
  message (wrapped) and, when the message fits on one line, the **first stack-trace line** below it
  (e.g. `UnityEngine.Debug.Log (object)`), same `#C0C0C0` colour, no indent.
* Multi-line messages consume the available lines (row 1 *Timings*: 2 message lines; row 8 warning:
  2 wrapped message lines, no stack line).
* **Alternating row backgrounds** `#373737` / `#313131`; the alternation follows the row index (38 px
  blocks), not the log level.
* **Selection**: click a row → background `#27537C`, all text `#F1F5F7`. The selected entry is
  *expanded into the bottom detail pane*, which renders its message (without timestamp) followed by the
  **full stack trace**, file paths as blue `#416EEE` links, with its own scrollbar.
  The list itself does **not** expand the selected row in place.
* **No hover highlight** on rows (verified by pixel diffing pointer-over vs. baseline).
* **Collapse ON** adds the circular repeat-count badge at the row's right edge.
* **Double-click a row** opens the reported source location (standard Unity behaviour — *not exercised
  here*, because it would launch the user's external IDE).
* Timestamps: `[HH:mm:ss]` in the same colour as the message.

### 7.4 Scrollbars / layout
* The list and the detail pane each have their **own 12 px scrollbar** in the same right-hand column;
  the list's thumb is at y 1223..1244 in the reference shot (list scrolled to the bottom).
* The dock tab strip (Project/Animation/…) and the ⋮ button belong to the **dock**, not the Console
  window — a standalone Kairos Console only needs the toolbar + list + detail pane.

---

## 8. Implementation notes / caveats

1. **Scale**: all values are 1× (the capture display is non-Retina). On a 2× display Unity would use the
   `@2x` textures and double the pixel metrics; the layout ratios stay the same.
2. **Row icons are drawn at 26×26 from 32×32 assets** (non-integer scale → the slight blur visible in the
   screenshot). Toolbar count icons are drawn at **13×13 from 16×16 assets**. If Kairos draws icons at the
   same sizes, use the provided 32/64 or 16/32 PNGs and let the renderer downscale (or render at 2× for crispness).
3. **Text is 12 px Inter** with a **15 px line pitch**; the entry box is **19 px per line**. Easy to get
   wrong: the row is 38 px but the two text baselines are only 15 px apart — the remaining space is padding
   (6 px above the first line, 6 px below the last).
4. The reference editor is the **Tuanjie fork** (Unity 2022.3.62t6). Its console adds a **bottom stack-trace
   detail pane** and the **Log Entry line-count setting**; stock Unity 2022.3 may differ in those two
   behaviours. The colours, icons, toolbar and row anatomy are the stock Unity 2022 dark-skin values
   (identical icon assets are present in the stock Unity 2022.3.62f1 bundle).
5. **Nothing in the user's project was modified.** Console settings were only toggled during the session to
   capture evidence, and were restored afterwards (**Show Timestamp ON, Log Entry = 2 Lines, Collapse OFF** —
   re-verified against the baseline capture: counts 39/303/5, timestamps visible, row 4 selected).
   The only side-effect is that the Console dock content sits **2 px lower** than at the very first capture
   (dock focus/layout re-flow), which does not change any measurement above.

---

## 9. Blockers / gaps encountered

* **AppleScript/System Events deadlocked mid-session** (another process had a stuck `osascript` querying
  System Events). Activation and input were therefore done with **CGEvent (Swift)** instead of
  Accessibility APIs; `screencapture -x -o -l <windowid>` was used to capture the window without activating
  it (window id from `CGWindowListCopyWindowInfo`).
* The Console was captured **docked** (its normal state). The dock was **not** maximised and no floating
  Console window was opened, to avoid leaving the user's editor layout changed.
* Not exercised (deliberately, to avoid side effects): double-click-to-open-source, search-text filtering,
  Collapse-ON stack details for a selected entry, and the Stack-Trace-Logging submenu values per level.
  These are standard Unity behaviours described in §7.
