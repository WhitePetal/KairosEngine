# UE World Outliner Research

> Sources: Unreal Engine 5 official documentation (dev.epicgames.com), UE source code architecture knowledge, community resources.

---

## 1. How the World Outliner Works

### 1.1 Data Structure — The Actor Hierarchy

The World Outliner is driven by Unreal's **actor–component hierarchy**, not a separate tree data structure. Every Actor placed in a level lives in `ULevel::Actors` (a `TArray<AActor*>`). The hierarchical tree view is formed by the attachment relationships between **Scene Components**:

```
UWorld
 └── ULevel (Persistent Level)
      ├── ULevel::Actors[]               ← flat list of all actors in the level
      │    ├── AActor (Light)
      │    ├── AActor (StaticMeshActor)
      │    │    └── RootComponent (USceneComponent)
      │    │         ├── ChildSceneComponent
      │    │         └── ChildSceneComponent
      │    └── AActor (BlueprintActor)
      │         └── RootComponent
      │              └── ...
      └── ULevel::Actors[] (streaming sub-levels each have their own array)
```

**Key fact:** Actors themselves do **not** store transform data (location, rotation, scale). An Actor's world transform is derived from its **Root Component** (`AActor::RootComponent`), which is a `USceneComponent`. The root component can be attached to another Actor's component, which creates the **parent–child relationship** displayed in the Outliner.

```cpp
// Simplified: how attachment creates the Outliner tree
ActorA->GetRootComponent()->SetupAttachment(ActorB->GetRootComponent());
// ActorA is now a child of ActorB in the Outliner
```

The attachment forms a directed tree (no cycles allowed):
- A `USceneComponent` can have any number of children.
- A `USceneComponent` can have at most one parent (or be attached directly to the world).
- Child transforms are relative to their parent.

### 1.2 Folders vs. Actor Nesting

UE provides two distinct organization mechanisms that both appear in the Outliner tree:

| Feature | Mechanism | Effect |
|---|---|---|
| **Actor Nesting** (Attachment) | `SetupAttachment()` / `AttachToComponent()` on the Root Component | Creates a real transform parent–child relationship. Child moves with parent. Affects gameplay. |
| **Folders** | `AActor::SetFolderPath()` — stored as metadata (`FFolder` / `ActorFolder` property) | Purely organizational. No transform relationship. No gameplay effect. Actors in the same folder retain world-space transforms. |

**Folders are cosmetic organization**: They are stored as an `FName` or `FFolder` property on each actor. The Outliner groups actors with the same folder path under a virtual folder node. This is rendered by `SceneOutliner` as a folder tree item, not a real actor.

**Actor nesting (attachment) is structural**: The Outliner uses the component attachment tree to render nesting. When ActorA's root is attached to ActorB's component, ActorA appears indented under ActorB.

### 1.3 Selection Sync Between Outliner and Viewport

Selection is synchronized through a global editor selection system:

```
┌─────────────────┐        ┌──────────────────────┐
│  World Outliner  │◄──────►│  GEditor->GetSelected │
│  (SSceneOutliner)│        │  Actors()             │
└─────────────────┘        └──────────┬───────────┘
                                      │
                             ┌────────▼───────────┐
                             │   Level Viewport    │
                             └────────────────────┘
```

- **GEditor** (`UUnrealEdEngine`) maintains the global actor selection set via `USelection`.
- When the user clicks an actor in the Outliner → `GEditor->SelectActor()` is called → selection event fires → Viewport highlights the actor and shows its transform gizmo.
- When the user clicks in the Viewport → `GEditor->SelectActor()` is called → selection change notification → Outliner scrolls to and highlights the entry.
- **"Always Frame Selection"** (Outliner Settings): When toggled on, selecting an actor in the Viewport auto-scrolls the Outliner to that actor. Can be disabled.

### 1.4 Drag-and-Drop Re-parenting (AttachTo)

Dragging an actor entry in the Outliner onto another actor entry triggers attachment:

1. **Drag detected** → Outliner identifies the dragged actor(s) via tree item hit testing.
2. **Drop target resolved** → The Outliner determines the recipient actor/component (supports dropping onto the actor itself, or onto a specific component if the tree is expanded).
3. **Attachment executed** → Calls `AActor::AttachToActor()` or `USceneComponent::AttachToComponent()`, passing:
   - The parent actor/component
   - Attachment rules (`FAttachmentTransformRules`): Keep World, Keep Relative, or Snap to Target
4. **Transaction created** → The attachment change is wrapped in an `FScopedTransaction` for undo/redo.
5. **Outliner refreshes** → Tree re-sorts, indentation updates, world transforms may change.

**Constraints:**
- Cannot create cycles (engine validates before attaching).
- Only `USceneComponent` (and subclasses) can be attachment parents.
- Attaching to a non-scene component is rejected.

### 1.5 Right-Click Context Menu

The Outliner's context menu is the **same** `FLevelEditorActionCallbacks` / `FActorContextMenu` as the Viewport context menu. It is built dynamically based on the current selection:

**Standard operations:**
| Category | Actions |
|---|---|
| Edit | Cut, Copy, Paste, Duplicate (`Ctrl+D`), Delete (`Delete`), Rename (`F2`) |
| Transform | Snap/Align, Transform to Coordinate System |
| Visibility | Hide Selected (`H`), Show All (`Ctrl+H`) |
| Actor | Convert Actor (to Static Mesh, to Blueprint, etc.), Replace Selected Actors |
| Grouping | Group (`Ctrl+G`), Ungroup (`Shift+G`), Lock/Unlock, Add to Group, Remove from Group |
| Level | Move to Current Level, Create Level from Selection |
| Attach | Attach To... (opens an actor picker), Detach |
| Selection | Select Matching (by class, by mesh), Select All Descendants, Select Immediate Children |
| Asset Actions | Browse to Asset, Edit Asset, Find in Content Browser |
| Miscellaneous | Merge Actors, Bake Materials, Export to FBX |

The menu includes **actor-type-specific** entries (e.g., "Edit Blueprint" for Blueprint actors, "Edit Particle System" for particle actors).

### 1.6 Search and Filter

**Search bar** (`SSceneOutlinerFilterBar`):

- **Default behavior**: Partial text match against actor name/label.
- **Multi-term search**: Space-separated terms → AND logic (actor must match all terms).
- **Advanced operators**:

| Operator | Action | Example |
|---|---|---|
| `-` | **Exclude** actors matching the term | `-Sky` hides all Sky-related actors |
| `+` | **Exact match** (disables partial match) | `+Sky` matches only `Sky`, not `Skylight` |
| `"..."` | **Phrase match** | `"point light"` matches exactly that string |
| `type:` | **Filter by class/type** | `type:Light` shows only Light actors |

- **Column-wide matching**: Search matches all columns (visible or hidden), including Type, Layer, Level, ID Name.

**Filter dropdown:**

- Predefined filters (Static Meshes, Lights, Blueprints, etc.).
- **Custom filters**: Save any search query as a named custom filter. These are stored globally per user (persist across projects and sessions).
- Filters combine with the search text (AND logic).

### 1.7 Sublevels / World Partition Integration

**Sublevels (Level Streaming):**

- The Outliner displays actors from **all currently loaded** levels (persistent + streaming sublevels).
- Each level's actors are grouped under a level header in the tree.
- The **Levels** column (when visible) shows which level each actor belongs to.
- Right-click → "Move to Current Level" to reassign actors between levels.
- World Composition (legacy, pre-UE5): Sublevels arranged on a 2D grid; Outliner reflects the currently loaded set.

**World Partition (UE5):**

- The entire world exists in a single persistent level, divided into a grid of cells.
- Actors are automatically assigned to cells based on their world position.
- Streaming is distance-based and automatic.
- The Outliner shows all actors in the persistent level. World Partition actors are not visually distinguished in the tree, but you can filter by Data Layer (the UE5 replacement for sublevel organization).

**Data Layers (UE5):**

- Successor to both Layers and sublevel-based organization.
- Actors belong to Data Layers; layers can be loaded/unloaded dynamically.
- The Outliner's "Data Layer" column shows each actor's layer assignment.

---

## 2. Features

### 2.1 Column View

Right-click any column header to show/hide columns:

| Column | Description |
|---|---|
| **Label** | Actor name/label (always visible, editable via `F2`) |
| **Type** | C++/Blueprint class of the actor |
| **Layer** | Legacy Layer assignment |
| **Level** | Which level (persistent or sublevel) the actor lives in |
| **Data Layer** | UE5 Data Layer assignment |
| **ID Name** | Unique editor identifier |
| **Visibility** | Eye icon toggle |
| **Lock** | Lock icon toggle |

- Columns are resizable (drag the edge of a header).
- Up to **four independent Outliner instances** can be open (Window → Outliner → Outliner 1/2/3/4), each with its own column configuration and filter state.

### 2.2 Locking and Visibility Toggles

Each actor row has inline toggle icons:

| Icon | Property | Effect |
|---|---|---|
| 👁 (Eye) | `AActor::bHidden` | Hides the actor in the Viewport. Hidden actors are grayed out in the Outliner. |
| 🔒 (Lock) | `AActor::bLocked` | Prevents the actor from being selected or transformed in the Viewport. Still selectable in the Outliner. |

- **Bulk toggle**: Multi-select actors, then toggle visibility/lock for all at once.
- Right-click → Visibility → "Hide Selected" (`H`), "Show All" (`Ctrl+H`), "Show Only Selected".

### 2.3 Labels and Groups

**Labels:**

- Each actor has a **Label** (display name), editable by `F2` or slow double-click in the Outliner.
- Labels are distinct from the actor's C++ class name.
- Duplicate actors get `_2`, `_3`, etc. appended automatically.

**Groups:**

- Groups are a special `AGroupActor` that acts as a container.
- Groups appear as nodes in the Outliner with the actors nested underneath.
- Green brackets = locked group (selecting any member selects the whole group; transforms affect all).
- Red brackets = unlocked group (individual selection and transformation allowed).
- Groups are **flat** — actors inside a group do not get a transform parent (unlike attachment).
- Groups can be created across actors from different levels (though moving an actor to another level removes it from the group).

### 2.4 Multi-Select Editing

- **Add to selection**: `Ctrl+Click`
- **Range selection**: Click first, then `Shift+Click` last
- **Marquee selection**: Click and drag in the Viewport
- **Select All**: `Ctrl+A` (in Outliner) or Edit → Select All

When multiple actors are selected:
- The **Details** panel shows properties common to all selected types.
- Properties with differing values display "Multiple Values".
- Editing a property applies the new value to **all** selected actors.
- Transform edits apply relative offsets (not absolute values) to preserve relative positioning.

### 2.5 Undo / Redo

Unreal Editor uses a **transaction system** (`UTransactor` / `UTransBuffer`) for undo/redo:

- **`Ctrl+Z`** = Undo
- **`Ctrl+Y`** = Redo

Transactions track changes to `RF_Transactional` objects (actors, components, levels). Each Outliner operation that mutates state creates a named transaction:

| Operation | Transaction name (example) |
|---|---|
| Delete actor(s) | "Delete Actors" |
| Attach/Detach | "Attach Actor" / "Detach Actor" |
| Rename | "Set Actor Label" |
| Move between levels | "Move Actor to Level" |
| Property edit via Details | "Edit Property" |
| Group/Ungroup | "Group Actors" / "Ungroup Actors" |
| Visibility toggle | "Toggle Visibility" |

The transaction system captures the object's **entire state before and after** the operation, enabling multi-step undo. It is deeply integrated with the serialization system — changed objects are serialized into the transaction buffer.

---

## 3. Editor vs. PIE (Play In Editor) Transition

### 3.1 What Happens When PIE Starts

```
┌──────────────────────┐          ┌──────────────────────┐
│   Editor World        │   copy   │   Play World          │
│  (UWorld::EditorWorld)│─────────►│ (UWorld::PlayWorld)   │
│                       │          │                       │
│  Editor Outliner      │          │  PIE Outliner         │
│  shows EditorWorld    │          │  (Debug → World       │
│  actors               │          │   Outliner)           │
└──────────────────────┘          └──────────────────────┘
```

1. UE **duplicates** the editor world into a separate `PlayWorld` (`UWorld::CreatePlayWorld()`).
2. Editor-only actors (`bIsEditorOnlyActor = true`) are **stripped** during the copy — they do not exist in the PlayWorld.
3. The **Editor Outliner** continues to show the frozen editor world. Actors in it are not ticking, not rendering gameplay visuals.
4. A **separate PIE Outliner** (accessible via the play-in-editor dropdown or Window → Developer Tools → World Outliner (PIE)) shows the live PlayWorld's actors.

### 3.2 Runtime-Spawned Actors

- Actors spawned by gameplay code during PIE (`UWorld::SpawnActor()`) appear in the **PIE Outliner**, not the editor Outliner.
- When PIE ends, the entire PlayWorld is destroyed, so runtime-spawned actors are gone.
- The editor Outliner returns to showing the original editor world's actor set (unchanged).

### 3.3 Editor-Only Actors

- Marked with `bIsEditorOnlyActor = true` (settable in Details panel).
- **In the Editor Outliner**: Visible and selectable/normal.
- **In PIE**: They are never copied to the PlayWorld and do not exist at runtime.
- **In packaged builds**: They are completely stripped (`WITH_EDITORONLY_DATA`).
- Common examples: NavMesh bounds volumes, Lightmass importance volumes, editor helper meshes.

### 3.4 Visualization Components

A related concept: **Visualization Components** (`SetIsVisualizationComponent(true)`) are Components that exist only in the editor (e.g., the camera frustum wireframe). They are wrapped in `#if WITH_EDITORONLY_DATA` and are automatically excluded from PIE and packaged builds.

---

## 4. UE Actor–Component Model vs. Unity GameObject–Component Model

This is one of the most fundamental architectural differences between the two engines.

### 4.1 Side-by-Side Comparison

| Aspect | Unreal Engine | Unity |
|---|---|---|
| **Core entity** | `AActor` — a C++ class you **derive from** | `GameObject` — a generic, typeless container |
| **Identity** | Defined by **class type** (e.g., `ACharacter`, `APawn`, `AStaticMeshActor`). The class determines what the thing *is*. | Defined by **component composition**. A GameObject is whatever its components make it. |
| **Design philosophy** | **Inheritance-heavy** + components for modularity | **Composition-heavy** — almost all behavior comes from components |
| **Transform** | Actor has **no transform**. The Root Component (`USceneComponent`) provides position/rotation/scale. | Every GameObject **always** has a `Transform` component (mandatory, cannot be removed). |
| **Component types** | `UActorComponent` (no transform), `USceneComponent` (has transform), `UPrimitiveComponent` (has geometry/rendering) | `MonoBehaviour` (behavior scripts), `Renderer` (visual), `Collider` (physics shape), etc. |
| **Root** | Optional: `RootComponent` is a `USceneComponent*`. If null, actor has no world position. | Always exists: `Transform` is the implicit root. |
| **Attachment** | Only `USceneComponent` subclasses can attach. Non-scene components (`UActorComponent`) live "flat" on the Actor. | Any `GameObject` can be parented to any other (through Transform hierarchy). |
| **Replication** | Actor is the atomic replication unit. Components replicate *through* their owning Actor. | `NetworkBehaviour` / `NetworkIdentity` component controls networking per-GameObject. |
| **Spawning** | `UWorld::SpawnActor<T>()` — must specify a class. Template function. | `Instantiate(prefab)` — clones a pre-configured GameObject template (prefab). |
| **Lifecycle** | Actor lifecycle: `BeginPlay()`, `Tick()`, `EndPlay()`, `Destroy()`. Garbage collected. | `Awake()`, `Start()`, `Update()`, `OnDestroy()`. Destroyed via `Destroy()`. |
| **Blueprints / Prefabs** | **Blueprint** = a class asset derived from an Actor class, containing a component hierarchy and script graph. | **Prefab** = a serialized GameObject+component template. Not a class, no inheritance. |
| **Component ownership** | Components are **sub-objects** of the Actor, created in the Actor's constructor or via `NewObject<T>(this)`. | Components are added via `AddComponent<T>()` and are owned by the GameObject. |
| **Typical workflow** | Subclass `AActor` / `APawn` / `ACharacter`, add components in Blueprint or C++ constructor, implement behavior in the class. | Create empty `GameObject`, add components until it does what you want. |

### 4.2 Visual Comparison

**Unreal Engine:**

```
ACharacter (C++ class: "HeroCharacter")
├── Root: UCapsuleComponent           ← transform origin
│   ├── USkeletalMeshComponent        ← visual
│   │   └── UWeaponComponent (child socket)
│   ├── UCharacterMovementComponent   ← movement logic (no transform)
│   ├── UHealthComponent              ← inventory/health (no transform)
│   └── UCameraComponent              ← player camera
```

- The **class** (`ACharacter`) defines the core identity and built-in capabilities (movement, replication, collision).
- Components add modular sub-capabilities (mesh, health, inventory).
- Non-transform components (`UActorComponent`) live on the actor but have no spatial relationship.

**Unity:**

```
GameObject "Hero"              ← typeless container
├── Transform                   ← mandatory, implicit root
├── Rigidbody                   ← physics
├── CapsuleCollider             ← collision shape
├── Animator                    ← animation controller
├── SkinnedMeshRenderer         ← visual
├── HeroMovement (MonoBehaviour) ← custom movement script
├── Health (MonoBehaviour)      ← custom health script
└── Camera                      ← child GameObject with Camera component
```

- The GameObject has **no intrinsic type**. Everything, including movement, health, and physics, comes from components.
- Script components (`MonoBehaviour`) are the primary way to add behavior.
- Child GameObjects are used for hierarchy (like the camera), each with their own mandatory Transform.

### 4.3 Implications for Tooling (Outliner / Hierarchy Panel)

| Implication | UE Outliner | Unity Hierarchy |
|---|---|---|
| **What nodes represent** | Actors (class instances). Components are shown in the Details panel, not the Outliner (unless you expand an actor to see attached child actors). | GameObjects. Components are shown as children of the GameObject in the Inspector but not in the Hierarchy. |
| **Type filtering** | Natural — filter by class (e.g., "show only Lights"). | Less natural — filter by component presence (e.g., "show GameObjects with a Light component"). Requires tag/component-based search. |
| **Inheritance in the tree** | Not directly visible (class hierarchy is separate from scene hierarchy). | GameObjects are typeless, so class hierarchy doesn't apply. Prefab variants create overrides. |
| **Empty/utility nodes** | Actors without RootComponent can exist (rare, mostly for manager/utility actors). | Every GameObject MUST have a Transform — no truly "empty" spatial nodes except zeroed transforms. |
| **Prefab workflows** | Blueprint editing opens a separate editor. Actor instances in the world can diverge from Blueprint via instance data. | Prefab editing can be done in context (prefab mode) or in isolation. Overrides are tracked per-property. |

---

## 5. Architectural Summary for Kairos Engine

When designing a hierarchy panel (Outliner equivalent) for Kairos Engine, the key architectural decisions from UE to consider:

1. **One selection authority**: Both Outliner and Viewport go through a single selection manager, avoiding desync bugs.
2. **Transaction system**: Undo/redo must be deeply integrated, not bolted on later. Every mutation should create a named transaction.
3. **Separate editor and play worlds**: The editor hierarchy panel shows the frozen editor world during play mode. A debug hierarchy panel can show the runtime world.
4. **Attachment vs. folders**: Keep these as separate concepts. Attachment affects transforms; folders are cosmetic organization.
5. **Column customization**: Plan for an extensible column system from the start. Each column is a "view" of actor properties.
6. **Search syntax**: +, -, type: operators are table stakes for productive use with large scenes.
7. **Entity identity**: Decide early whether entities have intrinsic types (like UE Actors) or are typeless containers (like Unity GameObjects). This affects filtering, spawning, and the entire tooling UX.
