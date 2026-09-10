---
name: implement-editor-inspector
description: Implement a new editor Inspector in KairosEngine. Use when the user wants to build an inspector for a new resource type or component.
disable-model-invocation: true
---

# Implement an Editor Inspector

Follow these patterns exactly. The reference implementations are
`TomlTableInspector` (state mutation) and `AudioInspector` (asset-system
integration). Read both before starting.

## 1. Understand the resource

Identify:
- What serialized form exists (TOML / binary / both)?
- What runtime form exists (asset system's `AssetType`)?
- What editor composite is needed (cf. `AudioExt` for audio, `TextureExt` for textures)?

If the resource doesn't have an editor composite yet, create one as a
struct bundling the serialized settings, runtime handles, and any cached
original data needed for inspector operations.

## 2. Create the editor asset (if needed)

Put the composite struct and its asset system under
`kairos_editor/editor_assets/<name>.rs`. The struct holds:

- Serialized form (modifiable by the inspector)
- Handles to runtime assets (for preview / dependent data)
- Any cached original data (for re-computation on Apply)

The asset system boilerplate follows the `AudioExtAssetsSystem` template
exactly — `LoadedEvent`, `DropEvent`, `Loader` (async, uses
`DependencyLoadRequest` for dependencies, `spawn_blocking` for heavy I/O),
the assets system struct, and all trait impls.

Add `pub use` re-exports in `editor_assets.rs` (the module root).
Add capacity constants in `kairos_editor/consts.rs`.
Add `pub mod editor_assets;` in `kairos_editor.rs` if not already present.

The system auto-registers on first `assets_server.load::<T>(&path)` — no
manual registration needed.

## 3. Create the style config

Add a TOML file under `Preferences/Styles/Inspectors/<Name>.toml`.
Add a `PATH_*_INSPECTOR_STYLE` constant in `ui/paths.rs`.
Define a `*InspectorStyle` struct in the inspector file (derive
`Serialize, Deserialize`), load it in `new()` via `toml::from_slice`.

## 4. Implement the Inspector

### Model

```rust
struct XxxInspectorModel {
    style: XxxInspectorStyle,
    /// Path to the primary asset file (for Apply writes).
    asset_path: PathBuf,
    /// Handle to the editor runtime resource.
    handle: Arc<AssetHandle<XxxExtAssetsSystem>>,
    /// Mutable cache — populated on first draw, modified by the inspector,
    /// consumed by the save handler on Apply.
    ext: Arc<Mutex<Option<XxxExt>>>,
}
```

### Inspector struct

```rust
pub struct XxxInspector {
    model: XxxInspectorModel,
    /// Any frame-cached data (egui textures, etc.) use Mutex / Cell.
    preview: Mutex<Option<TextureHandle>>,
    dirty: Cell<bool>,
}
```

### Inspector::create()

- Load style
- `assets_server.load::<XxxExtAssetsSystem>(&path)` — no blocking I/O
- Return immediately with empty mutex cache

### Inspector::draw()

```
1. Lock ext mutex
2. If empty, try to clone from assets_server.get(&handle)
3. If still empty → "Loading..." return early
4. Also wait for any dependent assets (preview data, etc.)
5. Draw UI using TableBuilder:
   - Striped, two columns (Column::auto + Column::remainder)
   - Each property row: label in col 0, value/control in col 1
   - Modify ext fields directly through the mutex guard
   - Set dirty flag on changes
6. Apply button outside the table
7. Preview panel below
```

### Message

```rust
// In ui.rs Message enum:
XxxInspectorApply(
    PathBuf,                                    // asset path
    Arc<AssetHandle<XxxExtAssetsSystem>>,       // ext handle
    Arc<Mutex<Option<XxxExt>>>,                // shared mutable state
),
```

### on_exit()

If dirty, show `ConfirmDialogWindow` with the same Message variant.
Compute target values from cached local state (Cells populated during draw).

### Handler (in ui.rs Context::handle())

```rust
Message::XxxInspectorApply(path, handle, ext) => {
    // 1. Reset inspector state (get inspector_mut, call apply() to reset dirty/preview)
    // 2. Call static save method: XxxInspector::save_xxx(&mut assets_server, &path, handle, ext)
}
```

### Save method

```rust
pub fn save_xxx(assets_server, path, handle, ext: Arc<Mutex<Option<XxxExt>>>) {
    let mut guard = ext.lock();
    let Some(ext) = guard.take() else { return; };

    // 1. Process the modified ext (resize, encode, etc.)
    // 2. Write files (delegate to ext.serialized.save_to_file() if possible)
    // 3. Update in-memory assets via assets_server.get_mut()
    // 4. Write back the ext: assets_server.get_mut(&handle) = ext
}
```

## 5. Wire up the inspector

- In `ui/inspector/creater.rs`: add the `AssetKind` match arm
- In `ui/inspector.rs`: add `pub mod xxx;`
- In `ui.rs`: add imports, Message variant, handler
- In `project_window.rs` (if needed): ensure correct path passed to
  `InspectorCreater::create_from_asseet_kind` (use `asset_path` for
  imported types)

## Checklist

- [ ] Read `TomlTableInspector` and `AudioInspector` source
- [ ] Editor composite struct + asset system in `editor_assets/`
- [ ] Style TOML + constant in `paths.rs`
- [ ] Model uses `Arc<Mutex<Option<Ext>>>`
- [ ] `create()` does zero blocking I/O
- [ ] `draw()` clones from asset server on first frame, "Loading..." otherwise
- [ ] UI uses `TableBuilder` (striped, two columns)
- [ ] Message carries `Arc<Mutex<Option<Ext>>>`, not computed output
- [ ] Handler calls inspector_mut for state reset, then static save method
- [ ] Save method delegates to existing `save_to_file()` where possible
- [ ] `on_exit()` shows ConfirmDialogWindow
- [ ] No `OnceLock`, no `TextureSourceInfo`-style lazy init structs, no
  blocking loops in create
