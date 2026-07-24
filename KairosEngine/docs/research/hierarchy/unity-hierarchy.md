# Unity Hierarchy Window — Research Notes

> **Sources:** Unity 6.5 (6000.5) Official Manual & Scripting API Reference
>
> Last updated: 2026-07-24

---

## 1. Internal Data Model

### 1.1 Core Data Structure: Transform Hierarchy (Scene Graph)

Unity does **not** use a separate tree data structure for the Hierarchy window. The Hierarchy **directly reflects** the runtime `Transform` component hierarchy—it is a visualization of the scene graph itself.

```
Hierarchy Window  ←→  Transform.parent / Transform.GetChild() chain
                  ←→  Scene root GameObjects
```

- Every `GameObject` has exactly one `Transform` (mandatory, cannot be removed).
- `Transform.parent` establishes the parent-child link.
- `Transform.childCount` and `Transform.GetChild(index)` enumerate children.
- `Transform.root` returns the topmost Transform in the hierarchy.
- `Transform.hierarchyCount` / `Transform.hierarchyCapacity` track the internal data structure capacity.

**Key implication:** There is no separate "editor tree model." The Hierarchy window walks the `Transform` parent-child pointers in real time. Changes made in the Hierarchy (drag re-parent, create, delete) directly mutate the same Transform relationships that the runtime uses.

### 1.2 Scene-Level Root Nodes

The Hierarchy window organizes content under **Scene root nodes**. Each loaded scene appears as a top-level root node in the Hierarchy:

- **Single-scene setup:** One scene root node named after the `.unity` scene file.
- **Multi-scene editing:** Multiple scene root nodes appear as siblings in the Hierarchy.
  - `EditorSceneManager.OpenScene(path, OpenSceneMode.Additive)` opens additional scenes.
  - `EditorSceneManager.CloseScene(scene)` removes a scene.
  - `EditorSceneManager.RestoreSceneManagerSetup(SceneSetup[])` restores a saved set of scenes.
  - Scene order in the Hierarchy can be changed via `EditorSceneManager.MoveSceneBefore()` / `MoveSceneAfter()`.
- Each scene is a `Scene` struct obtained via `SceneManager.GetSceneAt(index)`.
- `Scene.GetRootGameObjects()` returns the top-level GameObjects in the scene—these become the top-level nodes under the scene root in the Hierarchy.

### 1.3 Transform Ordering

- Default ordering is **creation order** — newest at the bottom.
- Reordering is done by **dragging** GameObjects up/down within the same parent.
- Under the hood: `Transform.SetSiblingIndex(index)` / `Transform.SetAsFirstSibling()` / `Transform.SetAsLastSibling()`.
- Undo for reorder: `Undo.RegisterChildrenOrderUndo(transform)` records the children order before mutation.

---

## 2. Selection State

### 2.1 The `Selection` Class (`UnityEditor`)

The entire selection system is centralized in the static `Selection` class:

| Property | Description |
|---|---|
| `Selection.activeGameObject` | The active GameObject (shown in Inspector) |
| `Selection.activeTransform` | The active Transform |
| `Selection.activeObject` | Actual object selection (includes Prefabs) |
| `Selection.gameObjects` | All selected GameObjects (unfiltered) |
| `Selection.transforms` | Top-level selection, excluding Prefabs |
| `Selection.objects` | All selected objects (unfiltered) |
| `Selection.count` | Number of objects in the selection |
| `Selection.assetGUIDs` | GUIDs of selected assets |
| `Selection.selectionChanged` | Delegate callback triggered on selection change |

Key methods:
- `Selection.Contains(object)` — test if an object is selected.
- `Selection.GetFiltered<T>(SelectionMode)` — filtered selection by type/mode.

### 2.2 Selection → Inspector Propagation

- The **Inspector window** subscribes to `Selection.selectionChanged` (or equivalent internal mechanism).
- When any item is selected in the Hierarchy, the `Selection` class is updated.
- The Inspector reads `Selection.activeGameObject` (or `Selection.activeTransform`, `Selection.activeObject`) and renders its components.
- The Inspector supports **locking** (via the lock icon) to keep inspecting a specific object even when selection changes.
- Multiple Inspector windows can be open, each locked to a different object.

### 2.3 Multi-Select

- Multi-select works via standard OS conventions:
  - **Ctrl+Click** (Cmd+Click on macOS): add/remove individual items.
  - **Shift+Click**: select a contiguous range.
  - **Ctrl+A** (Cmd+A): select all top-level items.
- `Selection.gameObjects` returns all selected GameObjects.
- When multiple objects are selected, the Inspector shows **common components** with multi-edit support (editing a value applies to all selected objects).
- Scene visibility/picking commands (H to hide, L to toggle pickability) work on the entire multi-selection.

---

## 3. Drag-and-Drop Re-Parenting

### 3.1 How It Works

- Drag a GameObject in the Hierarchy and drop it onto another GameObject → the dragged object becomes a **child** of the target.
- Internally calls `Transform.SetParent(newParent, worldPositionStays: true)`.
- The dragged GameObject maintains its world position (unless dropped with modifier keys).
- **Undo support:** `Undo.SetTransformParent(transform, newParent, "Set new parent")`.
- The editor also registers children order undo via `Undo.RegisterChildrenOrderUndo(parentTransform)`.

### 3.2 Visual Feedback During Drag

- A blue insertion line appears between items when dragging between siblings (for reorder).
- A highlight appears on the target GameObject when hovering over a valid parent.
- Invalid operations (e.g., making an object its own descendant) are blocked with visual feedback.

### 3.3 Related Operations

- **"Create Empty Parent"** (Ctrl+Shift+G): Wraps selected GameObjects in a new parent GameObject. Creates the new parent at the average position of the selection, then reparents.
- **"Paste as Child"** (Ctrl+Shift+V): Pastes a cut/copied GameObject as a child of the selected GameObject. Maintains world position.
- **"Set as Default Parent":** A designated default parent receives all new GameObjects created in the scene. Its name is displayed in **bold** in the Hierarchy. Only one default parent per scene.

---

## 4. Right-Click Context Menu

### 4.1 Create Operations

The context menu provides hierarchical creation menus:
- **3D Object** (Cube, Sphere, Capsule, Cylinder, Plane, Quad, etc.)
- **2D Object** (Sprite, Tilemap, etc.)
- **Effects** (Particle System, Line Renderer, etc.)
- **Light** (Directional, Point, Spot, Area)
- **Audio** (Audio Source, Audio Reverb Zone)
- **Video** (Video Player)
- **UI** (Canvas, Text, Button, Image, etc.)
- **Camera**
- **Create Empty** (or Ctrl+Shift+N)

Newly created GameObjects enter **rename mode** by default (configurable via More menu → "Rename New Objects").

### 4.2 Edit Operations

| Operation | Shortcut | Behavior |
|---|---|---|
| **Duplicate** | Ctrl+D | Clones the selected GameObject with same parent, sibling index + 1 |
| **Cut** | Ctrl+X | Removes GameObject, stores on clipboard |
| **Copy** | Ctrl+C | Copies GameObject to clipboard |
| **Paste** | Ctrl+V | Pastes as sibling |
| **Paste as Child** | Ctrl+Shift+V | Pastes as child of selection |
| **Delete** | Delete / Backspace | Destroys the GameObject |
| **Rename** | F2 | Enter rename mode |
| **Create Empty Parent** | Ctrl+Shift+G | Creates new parent for selection |

### 4.3 Prefab Operations

| Operation | Description |
|---|---|
| **Create Prefab Asset** | Creates a new Prefab from the GameObject |
| **Unpack Prefab** | Reverts prefab instance to a regular GameObject |
| **Unpack Prefab Completely** | Recursively unpacks all nested prefabs |
| **Select Prefab Asset** | Selects the source Prefab in the Project window |
| **Open Prefab** | Opens Prefab Mode for editing |
| **Override / Apply / Revert** | Manage prefab overrides |

These map to `PrefabUtility` methods: `SaveAsPrefabAssetAndConnect()`, `UnpackPrefabInstance()`, `ApplyPrefabInstance()`, `RevertPrefabInstance()`.

### 4.4 Scene Operations

- **Set as Default Parent** / **Clear Default Parent**
- **Move GameObject to Scene** → submenu of open scenes (`Undo.MoveGameObjectToScene()`)

---

## 5. Search / Filter

### 5.1 Search Bar

The Hierarchy window has a **search bar** at the top that filters the displayed tree. While Unity does not extensively document its internal implementation, observed behavior includes:

- **Name-based filtering:** Type a name to show only matching GameObjects.
- **Type-based filtering:** Type `t:ComponentName` to filter by component type. E.g., `t:Light` shows all objects with a Light component.
- **Tag/Layer filtering:** Known to support `tag:`, `layer:`, etc.
- **Matching:** Case-insensitive substring match.

### 5.2 Filter Behavior

- Filtering is **non-destructive** — it only affects the visual display.
- The tree structure is preserved: matching objects are shown with their full parent chain (ancestors that don't match are still shown to maintain tree context, typically dimmed).
- Clearing the search restores the full unfiltered hierarchy.

---

## 6. Prefab Instance Handling

### 6.1 Visual Indicators in the Hierarchy

| Visual | Meaning |
|---|---|
| **Blue text** | The GameObject is part of a Prefab instance |
| **Blue cube icon** (prefab icon) | Default prefab icon next to the name |
| **► Arrow indicator** | Prefab instance has overrides (different from the source) |
| **Gray prefab icon** | Prefab variant or model prefab |
| **Bold text** | Default parent GameObject |

### 6.2 `PrefabUtility` — The Key API (`UnityEditor`)

The `PrefabUtility` class provides all prefab-related introspection:

**Status checks:**
- `IsPartOfPrefabInstance(obj)` — is this object part of any Prefab instance?
- `IsPartOfPrefabAsset(obj)` — is this part of a Prefab Asset?
- `IsAnyPrefabInstanceRoot(obj)` — is this object the root of a Prefab instance?
- `IsOutermostPrefabInstanceRoot(obj)` — root of the outermost Prefab (important for nesting).
- `IsPartOfModelPrefab(obj)` — is it a Model Prefab (imported FBX, etc.)?
- `IsPartOfVariantPrefab(obj)` — is it a Prefab Variant?
- `IsPrefabAssetMissing(obj)` — is the source asset gone?

**Navigation:**
- `GetNearestPrefabInstanceRoot(obj)` — nearest Prefab root going up the hierarchy.
- `GetOutermostPrefabInstanceRoot(obj)` — outermost root (for nested prefabs).
- `GetCorrespondingObjectFromSource(obj)` — get the source Prefab Asset object.
- `GetCorrespondingObjectFromOriginalSource(obj)` — get the ultimate source (for variants).
- `GetPrefabAssetPathOfNearestInstanceRoot(obj)` — file path of the source asset.

**Overrides:**
- `GetObjectOverrides(instanceRoot)` — list all overrides on the instance.
- `GetPropertyModifications(instanceRoot)` — property-level modifications.
- `GetAddedComponents(instanceRoot)` — components added to the instance.
- `GetAddedGameObjects(instanceRoot)` — child GameObjects added to the instance.
- `GetRemovedComponents(instanceRoot)` — components removed from the instance.
- `HasPrefabInstanceAnyOverrides(instanceRoot)` — quick check.

### 6.3 Content Caching Architecture (Inferred)

The Hierarchy window likely maintains an internal cache/data structure that:
1. Walks the Transform hierarchy.
2. For each GameObject, queries `PrefabUtility` to determine prefab connection status.
3. Applies styling (blue text, icons, override indicators) based on the prefab status.
4. Monitors prefab events: `PrefabUtility.prefabInstanceUpdated`, `prefabInstanceApplied`, `prefabInstanceReverted`, `prefabInstanceUnpacked`.

---

## 7. Visibility & Picking (Eye and Hand Icons)

### 7.1 Scene Visibility (Eye Icon)

| State | Icon (Hover) | Description |
|---|---|---|
| **Visible** | Eye icon (visible on hover) | GameObject and all children visible |
| **Hidden** | Closed eye | GameObject and all children hidden |
| **Mixed (visible, some children hidden)** | Eye with dot | GameObject visible, some children hidden |
| **Mixed (hidden, some children visible)** | Eye with line | GameObject hidden, some children visible |

- **Click** eye icon: toggle visibility for the object **and all descendants**.
- **Alt+Click** eye icon: toggle visibility for the object **only** (children retain their state).
- **H** key: toggle visibility for selected object(s).
- **Shift+H**: isolate selection (hide everything else, exit with Shift+H again).
- Visibility settings are **persistent across sessions** — stored in `Library/SceneVisibilityState.asset`.
- The global Scene visibility toggle in the Scene View toolbar can override all per-object settings (mute/unmute).

### 7.2 Scene Picking (Hand Icon)

| State | Description |
|---|---|
| **Pickable** | Object can be selected in Scene View |
| **Not pickable** | Object is ignored by Scene View click-selection |

- **L** key: toggle pickability for selected object(s).
- Objects not pickable can still be selected via the Hierarchy window.

### 7.3 Script API

`SceneVisibilityManager` (a `ScriptableSingleton`):
- `Hide(gameObject)` / `Show(gameObject)` — manage visibility.
- `DisablePicking(gameObject)` / `EnablePicking(gameObject)` — manage pickability.
- `ToggleVisibility(gameObject)` / `TogglePicking(gameObject)` — toggle.
- `Isolate(gameObject)` / `ExitIsolation()` — isolation mode.
- `IsHidden(gameObject)` / `IsPickingDisabled(gameObject)` — query state.
- Events: `visibilityChanged`, `pickingChanged`.

---

## 8. Copy / Paste

### 8.1 Clipboard Model

- **Copy (Ctrl+C):** Serializes the selected GameObject(s) to an internal editor clipboard (not the system clipboard). The serialization includes all components, property values, and child hierarchy.
- **Cut (Ctrl+X):** Copies and marks the original for deletion.
- **Paste (Ctrl+V):** Deserializes and instantiates the clipboard content as a **sibling** of the selection (at the same hierarchy level).
- **Paste as Child (Ctrl+Shift+V):** Instantiates as a **child** of the selection, preserving **world position**.

### 8.2 Undo Integration

- Cut: registered as a destroy operation via `Undo.DestroyObjectImmediate()`.
- Paste: registered as a create operation via `Undo.RegisterCreatedObjectUndo()`.
- This makes copy/paste fully undoable.

---

## 9. Undo / Redo System

### 9.1 The `Undo` Class (`UnityEditor`)

The Undo system stores **delta changes** in an undo stack. Key characteristics:

- **Automatic grouping:** Undo operations within a mouse-down/mouse-up cycle are automatically grouped.
- **Manual grouping:** `Undo.IncrementCurrentGroup()` starts a new group.
- **Naming:** `Undo.SetCurrentGroupName("name")` sets the display name in the Edit > Undo menu.
- **Group ID:** `Undo.GetCurrentGroup()` returns the current group index.

### 9.2 Key Undo Methods for Hierarchy Operations

| Method | Use Case |
|---|---|
| `Undo.RecordObject(obj, name)` | Record property changes (e.g., Transform position) |
| `Undo.RegisterCreatedObjectUndo(obj, name)` | Undo creation of a new GameObject |
| `Undo.DestroyObjectImmediate(obj)` | Undo destruction of a GameObject |
| `Undo.AddComponent<T>(gameObject)` | Undo adding a component |
| `Undo.SetTransformParent(transform, newParent, name)` | Undo re-parenting |
| `Undo.RegisterChildrenOrderUndo(transform)` | Undo sibling reorder |
| `Undo.RegisterFullObjectHierarchyUndo(obj, name)` | Undo full hierarchy state |
| `Undo.RegisterCompleteObjectUndo(obj, name)` | Undo complete object state (serialized copy) |

### 9.3 Undo Events

- `Undo.undoRedoPerformed` — callback after any undo/redo.
- `Undo.undoRedoEvent` — callback with event details.
- `Undo.willFlushUndoRecord` — before flush.
- `Undo.postprocessModifications` — after property modifications.

---

## 10. Editor ↔ Runtime Transition (Play Mode)

### 10.1 Play Mode State Machine

```
Edit Mode ──ExitingEditMode──→ Pre-Play ──EnteredPlayMode──→ Play Mode
                                                        │
Play Mode ──ExitingPlayMode──→ Pre-Edit ──EnteredEditMode──→ Edit Mode
```

`PlayModeStateChange` enum:
- `ExitingEditMode` — about to enter Play mode.
- `EnteredPlayMode` — now in Play mode.
- `ExitingPlayMode` — about to exit Play mode.
- `EnteredEditMode` — back in Edit mode.

### 10.2 What Happens to the Hierarchy

When you press Play:

1. **Scene serialization:** The edit-mode scene is serialized to memory.
2. **Scene reload:** The scene is reloaded/re-initialized for runtime (Awake/Start called, physics initialized).
3. **Hierarchy window:** Continues to display the **runtime scene**. It does NOT show a different "copy" — the Hierarchy always reflects the currently loaded Scene objects. Now those objects are running in Play mode.
4. **Runtime-created objects:** GameObjects created via `Instantiate()` or `new GameObject()` at runtime **appear in the Hierarchy** in real time. They are visually distinguishable (typically dimmed or with a different tint in some Unity versions to indicate they're runtime-only).

When you press Stop:

1. **Scene teardown:** The runtime scene is destroyed.
2. **Scene restoration:** The serialized edit-mode scene is deserialized and restored.
3. **Hierarchy window:** Returns to showing the original edit-mode state. All runtime-created objects disappear.
4. **Transform state:** Any changes made during Play mode to Transform positions, etc., are reverted unless marked with `[RuntimeInitializeOnLoadMethod]` or similar patterns designed to persist.

### 10.3 Key Distinction: Editor vs. Runtime Scene API

| Context | Scene API | Hierarchy Visible? |
|---|---|---|
| **Edit mode** | `EditorSceneManager.OpenScene()` / `CloseScene()` | Yes, all open scenes shown |
| **Play mode** | `SceneManager.LoadScene()` / `UnloadSceneAsync()` | Yes, loaded scenes shown |
| **Editor-only preview scenes** | `EditorSceneManager.NewPreviewScene()` | **No**, not shown in Hierarchy |

---

## 11. Additional Features

### 11.1 Default Parent

- Any GameObject can be set as the **default parent** via right-click → "Set as Default Parent".
- Its name is rendered in **bold** in the Hierarchy.
- When set, all newly created GameObjects and drag-and-drops from the Project window are automatically parented to it.
- Only one default parent per scene.

### 11.2 Rename Mode

- New GameObjects enter rename mode by default (F2 to rename existing).
- Can be disabled via More menu (⋮) → deselect "Rename New Objects".

### 11.3 Expand/Collapse

- **Click ►:** Expand a single level.
- **Alt+Click ►:** Expand/collapse all descendants recursively.
- **Click ▼:** Collapse all descendants.

### 11.4 Color Coding / Labels

Unity's core Hierarchy does **not** natively support color coding or custom labels per GameObject. However:
- The "default parent" gets **bold text**.
- Prefab instances get **blue text**.
- **Workaround / Community packages:** Tools like "Rainbow Hierarchy" or "Hierarchy 2" (Unity Asset Store) provide color coding by adding a component that draws colored backgrounds and icons via custom Editor code.

### 11.5 Sorting

The Hierarchy window does **not** have a "sort by name" button in the default interface. The ordering is:
- Manual (drag to reorder).
- Creation order (default).
- The More menu (⋮) may expose additional options in newer versions.

---

## 12. Architecture Summary (Inferred)

Based on the public API surface, the likely internal architecture is:

```
┌──────────────────────────────────────────────────────┐
│                  Hierarchy Window (IMGUI/UI Toolkit) │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │
│  │ Search   │  │ Tree     │  │ Visibility/      │   │
│  │ Filter   │  │ Renderer │  │ Picking Icons    │   │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘   │
│       │              │                │              │
│       └──────────────┼────────────────┘              │
│                      │                               │
│           ┌──────────▼──────────┐                    │
│           │   Tree Model /      │                    │
│           │   Data Source       │                    │
│           │   (walks Transform  │                    │
│           │    .parent /        │                    │
│           │    .GetChild())     │                    │
│           └──────────┬──────────┘                    │
└──────────────────────┼──────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
┌───────▼──────┐ ┌─────▼──────┐ ┌────▼──────────┐
│  Selection   │ │ Prefab     │ │ Undo System   │
│  (static)    │ │ Utility    │ │ (Undo class)  │
│  ┌─────────┐ │ │ ┌───────┐  │ │ ┌───────────┐ │
│  │objects[]│ │ │ │IsPart │  │ │ │RecordObj  │ │
│  │activeGO │ │ │ │OfPref-│  │ │ │CreateUndo │ │
│  │callback │ │ │ │ab()   │  │ │ │DestroyIm- │ │
│  └─────────┘ │ │ │...    │  │ │ │mediate()  │ │
└──────┬───────┘ │ └───────┘  │ │ │SetParent  │ │
       │         └─────┬──────┘ │ └───────────┘ │
       │               │        └───────────────┘
       │               │
       │    ┌──────────▼──────────┐
       │    │    Transform Chain   │
       │    │  (Scene Graph)      │
       │    │                     │
       │    │  Scene              │
       │    │   └── GameObject A  │
       │    │         ├── GO B    │
       │    │         └── GO C    │
       │    │            └── GO D │
       │    │  Scene2             │
       │    │   └── GO E          │
       │    └─────────────────────┘
       │
       ▼
┌──────────────┐
│  Inspector   │
│  Window      │
│  (reads      │
│   Selection) │
└──────────────┘
```

**Key architectural insights:**

1. **No separate editor tree model.** The Hierarchy directly visualizes the `Transform.parent`/`Transform.GetChild()` graph. This means the "tree" is the runtime scene graph itself.

2. **Selection is global and decoupled.** The `Selection` class is a static singleton. The Hierarchy updates it on click; the Inspector reads it. This decoupling means multiple windows can observe selection independently.

3. **Visibility state is persisted separately.** Scene visibility/picking settings are stored in `Library/SceneVisibilityState.asset`, NOT in the scene file itself. This avoids version control conflicts.

4. **Prefab status is computed on-the-fly.** The Hierarchy queries `PrefabUtility` for each GameObject to determine whether to render it in blue text with a prefab icon. There's likely caching for performance.

5. **Undo wraps mutations, not the tree model.** Every Hierarchy operation (create, delete, reparent, reorder) wraps the underlying mutation in an `Undo.*` call. The undo system stores serialized state snapshots or property deltas.

6. **Multi-scene is a first-class concept.** The Hierarchy natively handles multiple scene root nodes. `EditorSceneManager` provides the scene lifecycle; the Hierarchy simply iterates all open scenes.

---

## 13. Key Differences for KairosEngine

When designing a Hierarchy-equivalent for KairosEngine, the following Unity design choices are worth noting:

| Aspect | Unity's Approach | Implication for KairosEngine |
|---|---|---|
| **Data model** | Transform hierarchy is the source of truth | Could use ECS hierarchy / `Parent` component chains |
| **Selection** | Global static `Selection` class | Need a resource/event-based selection system |
| **Undo** | Command pattern via `Undo` class | Could implement as a command queue with snapshot support |
| **Visibility** | Stored in Library folder, separate from scene | Could persist in editor state, not in scene data |
| **Prefab indicators** | Computed on-the-fly, keyed by asset path | Could use entity metadata / archetype references |
| **Multi-scene** | Scene root nodes as top-level entries | Could use world/sub-world root nodes |
| **Play mode** | Scene serialized, reloaded, restored | Could support hot-reload of component data |
