# Bevy & Godot Hierarchy / Scene Panel Research

> Research for KairosEngine editor hierarchy design.
> Focus: how each engine maps "editor scene data ↔ runtime world",
> and how the Inspector connects to the scene hierarchy.

---

## Table of Contents

1. [Bevy](#bevy)
   - [1. Hierarchy System (ECS Relationships)](#1-hierarchy-system-ecs-relationships)
   - [2. Editor Options](#2-editor-options)
   - [3. Mapping ECS to Hierarchy Panel](#3-mapping-ecs-to-hierarchy-panel)
   - [4. Selection → Inspector Propagation](#4-selection--inspector-propagation)
   - [5. Component Display in Inspector](#5-component-display-in-inspector)
   - [6. Scene Loading / Saving](#6-scene-loading--saving)
   - [7. Editor-Play Transition](#7-editor-play-transition)
2. [Godot](#godot)
   - [1. Scene Dock (Tree View)](#1-scene-dock-tree-view)
   - [2. Tree Structure (Node Tree + Inheritance)](#2-tree-structure-node-tree--inheritance)
   - [3. Instancing (PackedScene)](#3-instancing-packedscene)
   - [4. Inspector → Node Properties](#4-inspector--node-properties)
   - [5. Drag-and-Drop Re-parenting](#5-drag-and-drop-re-parenting)
   - [6. Editor-Play Transition](#6-editor-play-transition)
   - [7. .tscn Format & Scene Dock Relationship](#7-tscn-format--scene-dock-relationship)
3. [Architectural Comparison](#architectural-comparison)
   - [Data Model Comparison](#data-model-comparison)
   - [Editor ↔ Runtime Mapping](#editor--runtime-mapping)
   - [Key Design Decisions for KairosEngine](#key-design-decisions-for-kairosengine)

---

## Bevy

### 1. Hierarchy System (ECS Relationships)

**Bevy 0.19** replaced the old `Parent`/`Children` component pair with a generalized **Relationship** system:

```rust
// Old (pre-0.19):
#[derive(Component)] struct Parent(pub Entity);
#[derive(Component)] struct Children(pub SmallVec<[Entity; 8]>);

// New (0.19+): uses the Relationship/RelationshipTarget trait system
// ChildOf: a Relationship component on the child entity
// Children: a RelationshipTarget component on the parent entity
```

**Key types in `bevy::ecs::hierarchy`:**

| Type | Role |
|------|------|
| `ChildOf` | `Relationship` — stored on child, points to parent `Entity` |
| `Children` | `RelationshipTarget` — stored on parent, collects child entities |
| `ChildSpawner` | Type alias for `RelatedSpawner<ChildOf>` — spawns entities with a `ChildOf` relationship |
| `ChildSpawnerCommands` | Same via `Commands` |
| `AncestorIter` | Iterator over ancestors of an entity |
| `DescendantIter` | Iterator over descendants (breadth-first) |
| `DescendantDepthFirstIter` | Iterator over descendants (depth-first) |

**The generalized `Relationship` trait system in `bevy::ecs::relationship`:**

```rust
pub trait Relationship: Component {
    type RelationshipTarget: RelationshipTarget;
    // ...
}

pub trait RelationshipTarget: Component {
    type Collection: RelationshipSourceCollection;
    type Relationship: Relationship;
    // ...
}
```

This means hierarchies are just _one_ kind of relationship. The same mechanism supports
arbitrary entity-to-entity links (e.g. `Follows`, `Targets`, `OwnedBy`, etc.).

**Transform propagation:**
Bevy's `Transform` plugin queries `ChildOf` + `Children` to compute `GlobalTransform`
from local `Transform` values in a dedicated system (`propagate_transforms`).

### 2. Editor Options

Bevy has **no official built-in editor** as of 0.19. The ecosystem includes:

| Editor | Status | Approach |
|--------|--------|----------|
| `bevy_editor_pls` | Community, pre-0.19 | Immediate-mode `egui` panels; hierarchy is a simple recursive tree walk of `Children` |
| `space_editor` | Community | More feature-rich; uses `bevy_egui`; hierarchy panel queries `Children` relationships |
| New Bevy Editor (WIP) | Official, in development | Expected to be `egui`-based; will likely build on the Relationship system |

All current editors work by:
1. Querying `Entity` + `Children` + `Name` components
2. Building a tree from the recursive `Children` traversal
3. Displaying names (or fallback entity IDs) in a tree widget

### 3. Mapping ECS to Hierarchy Panel

The mapping is **direct** — every `Entity` in the `World` that has a `ChildOf` relationship
(or a parent) appears in the hierarchy tree.

```text
World (flat ECS storage)
  Entity A (Children: [B, C])
  Entity B (ChildOf(A))
  Entity C (ChildOf(A), Children: [D])
  Entity D (ChildOf(C))
  Entity E (no relationships — root entity)

        ↓ hierarchy panel renders as ↓

  └ A
    ├ B
    └ C
      └ D
  └ E
```

**Key observation:** There is no separate "editor scene data" — the hierarchy panel
**directly reflects the ECS world**. There is no intermediate `EditorScene` struct;
the `World` IS the scene. This is both powerful (no synchronization) and challenging
(no concept of "unsaved changes" separate from World state).

### 4. Selection → Inspector Propagation

Third-party Bevy editors use a **selection resource**:

```rust
#[derive(Resource)]
struct EditorSelection {
    selected: Option<Entity>,
}
```

The flow:
1. **Hierarchy panel** detects a click on a tree item → writes `Entity` to `EditorSelection`
2. **Inspector panel** reads `EditorSelection`, then queries the `World` for all components on that entity
3. **Inspector panel** iterates over components, rendering each one using `bevy_reflect` for introspection
4. **On change**, the inspector writes back to the component via `Reflect` trait's `apply`/`set` methods

Bevy's `Reflect` trait provides:
- `type_name()`, `type_info()`, `reflect_hash()`, `reflect_partial_eq()`
- Field iteration via `Struct`/`TupleStruct`/`Enum`/`List`/`Map` reflect subtraits
- `FromReflect` for cloning/deserializing
- `Typed` for type-erased manipulation

This is how a generic Inspector can display and edit **any** component without
knowing its concrete type at compile time.

### 5. Component Display in Inspector

Since Bevy is ECS-based, the Inspector shows a **flat list of all components** on the
selected entity. There is no inheritance hierarchy to display (unlike Godot).

Each component is rendered as a collapsible section showing its fields,
using `bevy_reflect` to enumerate fields and their values.

```text
Inspector Panel
┌─────────────────────────────┐
│ Entity: 42v0                │
│ Name: "Player"              │
├─────────────────────────────┤
│ ▼ Transform                 │
│   translation: [0, 5, 0]   │
│   rotation: [0, 0, 0, 1]   │
│   scale: [1, 1, 1]         │
├─────────────────────────────┤
│ ▼ Sprite                    │
│   color: #FFFFFF            │
│   image: "player.png"       │
│   ...                       │
├─────────────────────────────┤
│ ▼ Health                    │
│   current: 100              │
│   max: 100                  │
├─────────────────────────────┤
│ ▶ ChildOf                   │
│ ▶ Children                  │
├─────────────────────────────┤
│ [+ Add Component]           │
└─────────────────────────────┘
```

### 6. Scene Loading / Saving

**Old system (Bevy < 0.19):**
- `DynamicScene` — a reflection-based serialized scene
- `.scn.ron` format (RON — Rusty Object Notation)
- Used `bevy_reflect` to serialize/deserialize component data
- `SceneSpawner` system to instantiate scenes into the World

**Old `.scn.ron` example:**
```ron
(
  resources: {},
  entities: [
    (
      entity: 0,
      components: {
        "bevy_transform::components::transform::Transform": (
          translation: (x: 0.0, y: 5.0, z: 0.0),
          rotation: (x: 0.0, y: 0.0, z: 0.0, w: 1.0),
          scale: (x: 1.0, y: 1.0, z: 1.0),
        ),
        "bevy_core::name::Name": ("Player"),
      },
    ),
    // ... more entities with Parent components for hierarchy
  ],
)
```

**New system (Bevy 0.19+): BSN (Bevy Scene Notation)**
- Macro-based: `bsn! { ... }` for inline scene definitions
- `Scene` trait — describes what a spawned entity should look like
- `SceneList` — list of scenes, each producing one Entity
- `Template` — "superpowered ECS-aware constructor" that can access World/AssetServer
- `FromTemplate` — automatically implemented for `Default + Clone` types; manual impl for Handle<>, Entity, etc.
- **Patching** — scenes compose by patching: later entries override fields of earlier entries
- `SceneComponent` — a component with an associated scene, similar to Godot's PackedScene concept
- `.bsn` file format — planned but not yet shipped

**BSN example (hierarchical):**
```rust
commands.spawn_scene(bsn! {
    #Player
    Transform::from_xyz(0.0, 5.0, 0.0)
    Health { current: 100, max: 100 }
    Children [
        (#Sword Mesh3d(sword_mesh)),
        (#Shield Mesh3d(shield_mesh)),
    ]
});
```

**Key architectural differences old → new:**
- Old: serializes the entire World as a flat entity list with serialized components
- New: describes entities declaratively; composition via patching; asset handles resolved automatically
- Old required `Reflect` on everything; new requires `FromTemplate` or `Default + Clone`
- New supports named entity references within the same `bsn!` scope
- New supports `SceneComponent` which bundles a component with its scene (like Godot PackedScene)
- New has queued spawning with automatic dependency resolution

### 7. Editor-Play Transition

Since Bevy has no official editor, the editor-play transition pattern is defined by
third-party tools. Common approaches:

1. **Clone the World:**
   - On "Play", clone the current World state
   - Optionally strip editor-only entities/components
   - Run the clone in a sub-app or replace the main World
   - On "Stop", discard the play World, restore editor state

2. **Sub-App approach:**
   - Run editor and game in separate Bevy `App` instances
   - Editor app controls the game app's schedule
   - Game runs in a separate window/viewport

3. **Entity tagging:**
   - Editor entities/components marked with `EditorOnly` marker component
   - On play, despawn all EditorOnly entities
   - On stop, reload the scene from saved state

The core challenge for any editor is: **the World IS the scene**.
There's no separate "editor data model" to diff against. Saving means
serializing all non-editor entities, and reloading means deserializing
back into the World.

---

## Godot

### 1. Scene Dock (Tree View)

Godot's Scene dock is built on Godot's own `Tree` UI control (`class_Tree`).

The dock displays the **currently edited scene's node tree**.
It shows:
- Node name (from `Node.name` property)
- Node type icon
- Visibility toggle (eye icon)
- Lock toggle (lock icon)
- Connection/groups indicator
- Script indicator

The tree is a **direct visual representation** of the `Node` tree for the
currently open scene.

**Editor API:**
- `EditorInterface.get_selection()` returns the `EditorSelection` singleton
- `EditorSelection.get_selected_nodes()` returns the currently selected nodes
- Signal `EditorSelection.selection_changed` emitted on selection change

### 2. Tree Structure (Node Tree + Inheritance)

Godot uses **classical OOP inheritance** for its node hierarchy:

```
Object
  └ Node — base class for all scene tree nodes
       ├ CanvasItem — base for 2D and UI nodes
       │    ├ Node2D — 2D game objects
       │    │    ├ Sprite2D
       │    │    ├ Area2D
       │    │    ├ RigidBody2D
       │    │    └ ...
       │    └ Control — UI widgets
       │         ├ Button
       │         ├ Label
       │         └ ...
       └ Node3D — 3D game objects
            ├ MeshInstance3D
            ├ Camera3D
            ├ Area3D
            └ ...
```

The scene tree reflects both:
1. **Runtime hierarchy** (parent-child spatial/nesting relationships)
2. **Inheritance hierarchy** (Node type determines available properties)

Each node knows its children through `Node.get_children()` and its parent through
`Node.get_parent()`. The scene tree is traversed **pre-order** for `_process`,
input, and rendering, and **post-order** for `_ready` (children ready before parents).

### 3. Instancing (PackedScene)

**PackedScene** is Godot's solution for reusable scene templates:

```gdscript
# Loading and instancing
var scene = preload("res://player.tscn")  # loads PackedScene resource
var instance = scene.instantiate()         # creates Node hierarchy
add_child(instance)                        # adds to current scene tree
```

**PackedScene internals:**
- `PackedScene.pack(node)` — serializes a node and all **owned** sub-nodes
- `PackedScene.get_state()` — returns `SceneState`, a read-only snapshot of serialized data
- `PackedScene.instantiate(edit_state)` — creates a new node hierarchy from the scene state
- `Node.owner` property — determines which nodes are "owned" by the scene root (and thus serialized)

**Owner concept:**
```gdscript
var node = Node2D.new()
var body = RigidBody2D.new()
var collision = CollisionShape2D.new()

body.add_child(collision)
node.add_child(body)

body.owner = node  # Only 'node' and 'body' will be packed
# 'collision' is NOT owned by 'node', so it will NOT be serialized
```

This is a **critical design choice**: the `owner` property creates a distinction
between "the scene's authored content" and "runtime-spawned/dynamically-added content."
Only owned nodes are saved to `.tscn`.

**GenEditState enum** controls instantiation behavior:
- `GEN_EDIT_STATE_DISABLED` (0) — blocks edits, used at runtime
- `GEN_EDIT_STATE_INSTANCE` (1) — editor-only, provides local scene resources
- `GEN_EDIT_STATE_MAIN` (2) — for the main scene being edited
- `GEN_EDIT_STATE_MAIN_INHERITED` (3) — for inherited scenes

### 4. Inspector → Node Properties

Godot's Inspector is an `EditorInspector` control that displays properties of the
currently selected node(s).

**How it works:**

1. `EditorSelection.selection_changed` signal fires
2. `EditorInspector` queries `EditorSelection.get_selected_nodes()`
3. For each selected node, the inspector:
   - Gets the node's **class** via `Object.get_class()`
   - Enumerates **properties** via `Object.get_property_list()` (accounts for inheritance chain)
   - Creates appropriate `EditorProperty` widgets for each property type
4. Property changes are applied via `Object.set(property, value)` → triggers `_set()`/setters
5. All changes are recorded in `EditorUndoRedoManager` for undo/redo

**Property display order:**
1. Script variables (`@export` vars from attached script)
2. Inherited properties (from base classes, grouped by class)
3. Each class group is collapsible

**Custom Inspector plugins:**
`EditorInspectorPlugin` allows adding custom controls, property widgets, or entire
custom sections to the inspector for specific types.

**Key difference from Bevy:** Godot's Inspector shows properties grouped by
**inheritance chain**, not a flat component list. This is because Godot uses
single-inheritance OOP rather than composition-based ECS.

### 5. Drag-and-Drop Re-parenting

Godot's Scene dock supports drag-and-drop re-parenting natively through the `Tree` control.

**Implementation:**
1. The Scene dock's `Tree` sets `drag_mode_enabled = true`
2. When a node is dragged:
   - `Tree` emits `item_mouse_selected` or handles drag internally
   - A drag preview (node name/icon) follows the cursor
   - Valid drop targets are highlighted
3. On drop:
   - Source node is **reparented**: `source.reparent(new_parent)`
   - This calls `old_parent.remove_child(source)` and `new_parent.add_child(source)`
   - If the source was a scene root (owner = self), the owner is updated
   - **Undo/Redo** is recorded via `EditorUndoRedoManager`
4. Constraints:
   - Cannot parent a node to itself or its descendants
   - Cannot parent outside the scene (unless "Editable Children" is enabled for inherited scenes)
   - Some node types restrict accepted children (e.g. `Viewport`)

**The `Node.reparent()` method:**
```cpp
// C++ internals (simplified):
void Node::reparent(Node* p_new_parent, bool p_keep_global_transform) {
    // 1. Store global transform if keep_global_transform
    // 2. Remove from old parent
    // 3. Add to new parent
    // 4. Restore global transform if needed
    // 5. Update owner if scene root
    // 6. Emit tree_changed signal
}
```

### 6. Editor-Play Transition

**"The Godot editor is a Godot game"** — this is a core design philosophy.
The editor itself runs on the Godot engine.

**How the play transition works:**

1. **Editor state (edit mode):**
   - The scene being edited lives as a child of the root `Viewport`
   - `SceneTree.edited_scene_root` points to the edited scene's root node
   - Editor-only nodes (gizmos, grids, etc.) are children of `editor_node`
   - The editor uses `@tool` scripts and `Engine.is_editor_hint()` to distinguish modes

2. **On "Play" button press:**
   - The current scene is **saved** (if dirty) or a copy is made
   - A **new SceneTree** or a new scene instance is created
   - The edited scene instance is swapped out, and the "play" instance is added
   - Physics, input, and rendering continue in the new scene
   - `SceneTree.paused` can be toggled for debugging

3. **On "Stop":**
   - The play scene is unloaded
   - The editor scene is reloaded/re-instantiated
   - Editor state is restored

4. **Key properties:**
   - `SceneTree.edited_scene_root` — null at runtime, points to edited scene in editor
   - `Engine.is_editor_hint()` — returns `true` when running inside the editor
   - `Node.NOTIFICATION_EDITOR_PRE_SAVE` — sent before save
   - `Node.NOTIFICATION_EDITOR_POST_SAVE` — sent after save

**Game embedding (Godot 4.x+):**
The editor can embed the running game in a sub-window:
- `EditorInterface.play_main_scene()` starts the game
- The game can run in a separate window or embedded in the editor viewport
- Hot-reloading of scripts while game is running is supported

### 7. .tscn Format & Scene Dock Relationship

The `.tscn` file format is a text-based representation of a `PackedScene`.

**Structure:**
```ini
[gd_scene load_steps=4 format=3 uid="uid://abc123"]

[ext_resource type="Script" path="res://player.gd" id="1_abc"]
[ext_resource type="Texture2D" path="res://icon.png" id="2_def"]

[sub_resource type="CircleShape2D" id="Circle_ghi"]
radius = 20.0

[node name="Player" type="CharacterBody2D"]
script = ExtResource("1_abc")
position = Vector2(100, 200)

[node name="CollisionShape2D" type="CollisionShape2D" parent="."]
shape = SubResource("Circle_ghi")

[node name="Sprite2D" type="Sprite2D" parent="."]
texture = ExtResource("2_def")
```

**Sections:**
1. **Header** `[gd_scene ...]` — format version, UID, load steps
2. **`[ext_resource ...]`** — references to external files (scripts, textures, etc.)
3. **`[sub_resource ...]`** — inline resources defined within this scene file
4. **`[node ...]`** — each node in the hierarchy, with `parent="."` indicating the previous node

**Relationship to Scene Dock:**
- The Scene dock displays the node tree defined by the `.tscn` file
- Edits in the dock are reflected in the `.tscn` file on save
- The `.tscn` is loaded into a `PackedScene`, which is `instantiate()`-d to create the editable node tree
- `SceneState` (from `PackedScene.get_state()`) provides a read-only view of serialized data
- Scene inheritance (`.tscn` extending another `.tscn`) is supported through `ext_resource` references

**Key design points:**
- `.tscn` is a **resource** (not a scene directly) — it's a `PackedScene` resource
- The format stores a **flat list of nodes** with parent references, not a tree structure
- Resources are **deduplicated** (shared resources referenced by ID)
- The uid system provides stable references even when files are moved
- Format is designed to be **human-readable** and **VCS-friendly**

---

## Architectural Comparison

### Data Model Comparison

| Aspect | Bevy (ECS) | Godot (OOP) |
|--------|-----------|-------------|
| **Core entity type** | `Entity` (u32/u64 ID) — just an identifier | `Node` (Object) — class instance with methods + data |
| **Composition** | Components attached to entities via `World` | Properties on node; script variables |
| **Hierarchy** | `ChildOf` relationship (component on child) + `Children` (component on parent) | `Node.get_parent()` / `Node.get_children()` — linked list of pointers |
| **Type system** | No inheritance — type is the set of components | Single-inheritance OOP tree |
| **Transform** | `Transform` component + `GlobalTransform` (computed) | `Node2D.position` or `Node3D.position` (built into base class) |
| **Scene persistence** | BSN macros (inline) or `.scn.ron` / planned `.bsn` | `.tscn` text format + `PackedScene` resource |
| **Reflection** | `bevy_reflect` (trait-based, per-type) | `Object.get_property_list()` (built into Object base class) |
| **Ownership** | No distinction — all entities in World are equal | `Node.owner` marks "authored" vs "runtime" nodes |

### Editor ↔ Runtime Mapping

**Bevy approach (idealized):**
```
World (flat, all entities)
  │
  ├── Hierarchy view: filter entities with ChildOf/Children; build tree
  │
  ├── Inspector: for selected entity, iterate all components via Reflect
  │
  └── Scene save: serialize entity graph via Reflect into RON/BSN
```

**Godot approach:**
```
PackedScene (serialized .tscn on disk)
  │
  ├── instantiate() → Node tree (runtime object graph)
  │
  ├── Scene Dock: display Node.get_children() tree
  │
  ├── Inspector: Object.get_property_list() → EditorProperty widgets
  │
  └── Scene save: Node tree → serialize owned nodes → PackedScene → .tscn
```

**Key difference:**
- Bevy: the World **IS** the scene. The hierarchy panel is a **view** of World state.
- Godot: the PackedScene is a **separate data model** from the runtime Node tree.
  The editor edits the PackedScene's instantiated nodes, and saving serializes them back.

### Key Design Decisions for KairosEngine

#### 1. Separate Editor Scene Model?

**Godot's approach** (separate `PackedScene`/`SceneState` from runtime nodes):
- ✅ Clear separation: "this is authored" vs "this is runtime"
- ✅ Undo/redo works on the scene model independently
- ✅ Can diff between saved and current state ("unsaved changes")
- ✅ Scene inheritance and scene variants are natural
- ❌ Two parallel data structures to keep in sync
- ❌ More code to write and maintain

**Bevy's approach** (World IS the scene):
- ✅ Simple — no synchronization needed
- ✅ Direct manipulation of runtime state
- ✅ Less code
- ❌ No clear concept of "unsaved changes" — need to diff against serialized state
- ❌ Editor-only entities/components pollute the World (mitigated by marker components)
- ❌ Undo/redo must work at the ECS level (component insert/remove/mutate)

#### 2. Component Display in Inspector

**Godot-style (inheritance-based):**
- Properties grouped by class in inheritance chain
- Script variables in dedicated section
- Makes sense for OOP with deep hierarchies

**Bevy-style (flat component list):**
- All components shown as collapsible sections
- Order is arbitrary or user-defined
- Makes sense for composition-based ECS

**For KairosEngine (Rust/ECS-based):**
Recommend Bevy-style flat component list, since KairosEngine uses Bevy's ECS.
Components are the natural unit of composition.

#### 3. Hierarchy Representation

**Godot-style:**
- Each node has a **class type** that determines its capabilities
- Inherited scenes show an icon indicating they're external
- Editable children toggle for scene instances

**Bevy-style:**
- Each entity has a **set of components** that determine its capabilities
- No inherent type — an entity is what its components make it
- Scene instances are just entities with components from a scene template

**For KairosEngine:**
The hierarchy tree should show entity names (via `Name` component) and optionally
display component icons next to entities to indicate their "type" (e.g., if an entity
has `Camera`, show a camera icon).

#### 4. Scene Ownership / Scene Instance Boundaries

Godot's `Node.owner` concept is valuable for an editor:
- Distinguishes "this node was authored in this scene" from "this was spawned at runtime"
- Determines what gets saved to `.tscn`
- Creates clear "scene instance" boundaries in the hierarchy view

For KairosEngine, consider a marker component approach:
```rust
#[derive(Component)]
struct SceneRoot; // marks the root entity of a scene instance

#[derive(Component)]
struct SceneOwned; // marks entities that should be saved with the scene
```

#### 5. Editor-Play Transition

**Recommendation for KairosEngine:**
Use Bevy's **Sub-App** pattern:
```
Main App
├── Editor Sub-App (always runs)
│   ├── Editor camera, gizmos, UI panels
│   └── Reads/writes game World via shared resource
└── Game Sub-App (runs during "Play")
    ├── Game camera, physics, gameplay systems
    └── Operates on cloned World or separate World
```

Benefits:
- Editor state (selection, undo history, UI) is never part of the game World
- Easy to implement "play in editor" (run game sub-app with editor overlay)
- Clean reset on "Stop" (just drop the game sub-app)
- No need to save/reload or tag/detag entities

#### 6. Scene Format

For KairosEngine, both the old Bevy `.scn.ron` and new BSN approaches have lessons:

| Old `.scn.ron` | New BSN |
|---|---|
| RON format, human-readable | Custom DSL in Rust macros (and future `.bsn` files) |
| Flat entity list with component data | Declarative scene descriptions |
| Requires `Reflect` on all components | Requires `FromTemplate` or `Default + Clone` |
| Serializes exact World state | Describes what to construct; supports composition/patching |
| No template/parameterization | Supports `SceneComponent` with props |

For KairosEngine, a human-readable, VCS-friendly text format (like `.tscn` or `.scn.ron`) is desirable for an editor. The format should:
- Store the entity hierarchy (parent-child relationships)
- Store component values per entity
- Reference external assets/resources
- Support scene composition/inheritance

---

## Summary Table

| Feature | Bevy | Godot |
|---------|------|-------|
| **Base entity** | `Entity` (ID) + components | `Node` (Object subclass) |
| **Hierarchy data** | `ChildOf` + `Children` components | Linked list in Node |
| **Editor** | None official; `bevy_editor_pls` | Built-in, full-featured |
| **Inspector** | Reflect-based, flat component list | Property-based, grouped by inheritance |
| **Scene save** | BSN macro / planned `.bsn` / old `.scn.ron` | `.tscn` text format |
| **Scene load** | `commands.spawn_scene(bsn!{...})` | `PackedScene.instantiate()` |
| **Instancing** | Scene macro composition | PackedScene with owner concept |
| **Selection** | `EditorSelection` resource | `EditorSelection` singleton |
| **Undo/Redo** | Not built-in (third-party adds) | `EditorUndoRedoManager` |
| **Re-parenting** | Insert/remove `ChildOf` component | `Node.reparent()` method |
| **Play transition** | Clone World or Sub-App | Swap scene tree; hot-reload |
| **Transform** | `Transform` + `GlobalTransform` components | Built-in `.position`/`.rotation`/`.scale` per node type |
| **Type system** | Composition (set of components) | Inheritance (class hierarchy) |
