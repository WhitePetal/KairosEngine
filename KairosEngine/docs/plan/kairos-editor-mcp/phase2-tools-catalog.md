# Phase 2: MCP Tool Catalog — `kairos_editor_mcp`

> **Wayfinder D2 resolution** — Research output from studying the MCP spec, egui_mcp tool catalog (see `docs/research/egui-mcp-analysis.md`), and the KairosEngine editor module source code.
>
> This document specifies the complete MCP Tools and Resources that `kairos_editor_mcp` should expose, categorized by priority and module origin. It will be refined when D1 (requirements) is resolved.

## 1. Tool Design Principles

Derived from the [MCP Tools spec](https://modelcontextprotocol.io/docs/concepts/tools) and egui_mcp patterns:

| Principle | Application |
|-----------|-------------|
| **Model-controlled** | LLM discovers tools via `tools/list`, invokes via `tools/call`. Each tool has a clear `description` so the model understands when to use it. |
| **Structured input/output** | Use JSON Schema (`inputSchema`, `outputSchema`) for type-safe parameters. `structuredContent` for programmatic consumption; `content` (text) for human readability. |
| **Error safety** | Protocol errors (unknown tool, invalid args) use JSON-RPC error codes. Execution errors use `isError: true` in the result. Never crash the editor on a bad MCP call. |
| **Idempotent where possible** | Query tools (`get_*`) are read-only. Action tools (`select_*`, `create_*`) should be safe to retry. |
| **Semantic over generic** | Editor tools map to domain concepts (asset, inspector, camera) rather than generic widget coordinates, while still inheriting egui_mcp's generic tools as an escape hatch. |
| **Resources for state** | Read-only editor state (project tree, camera, logs) is exposed as MCP Resources with stable URIs, supporting `resources/list`, `resources/read`, and optional `resources/subscribe`. |

### 1.1 What kairos_editor_mcp adds beyond egui_mcp

| Layer | egui_mcp (generic) | kairos_editor_mcp (semantic) |
|-------|-------------------|------------------------------|
| **Targeting** | AccessKit id, role, text match | Asset path/GUID, camera target, entity name |
| **State queries** | AccessKit tree (labels, values, bounds) | Project tree, asset registry, scene camera, console logs |
| **Commands** | click, type, scroll, drag, key press | select_asset, create_asset, orbit_camera, inspect_asset |
| **Scene view** | Screenshot only (pixels) | Camera control (orbit/zoom/fly), scene state query |
| **File ops** | AccessKit tree traversal | Project path graph operations (create, delete, rename, navigate) |

---

## 2. Summary Table: All Tools with Priority

### 2.1 Generic egui Tools (inherited from `UiServer`)

These tools come from `egui_mcp` and provide the generic widget-level interaction surface. Included in kairos_editor_mcp via Design A (separate server reusing `UiServer`).

| Tool | Priority | Description |
|------|----------|-------------|
| `attach` | P0 | Connect to running KairosEngine instance |
| `disconnect` | P0 | Disconnect from attached instance |
| `status` | P0 | Query connection state |
| `query_tree` | P0 | Walk AccessKit tree; discover widgets by role/label/value |
| `get_node` | P0 | Lookup single widget by id |
| `click` | P0 | Click a widget (id / query match / pos) |
| `hover` | P0 | Hover over a widget |
| `scroll` | P0 | Scroll a widget |
| `drag` | P0 | Drag from one widget to another |
| `type_text` | P0 | Focus a widget and type text |
| `press_key` | P0 | Send key press event |
| `screenshot` | P0 | Capture rendered frame as PNG |
| `resize` | P1 | Resize viewport dimensions |
| `wait_for` | P1 | Poll tree until condition holds |
| `batch` | P1 | Execute multiple tools in one round-trip |

### 2.2 Editor-Specific Tools

| # | Tool Name | Module Origin | Priority | Category |
|---|-----------|---------------|----------|----------|
| 1 | `get_project_tree` | `project_path_tree` | **P0** | Project/Asset Query |
| 2 | `get_asset_info` | `asset_registry` | **P0** | Project/Asset Query |
| 3 | `get_asset_registry` | `asset_registry` | **P0** | Project/Asset Query |
| 4 | `select_asset` | `project_window` | **P0** | Project/Asset Action |
| 5 | `open_asset` | `project_window` | **P0** | Project/Asset Action |
| 6 | `create_asset` | `project_window` | **P0** | Project/Asset Action |
| 7 | `delete_asset` | `project_window` | **P0** | Project/Asset Action |
| 8 | `rename_asset` | `project_window` | **P0** | Project/Asset Action |
| 9 | `inspect_asset` | `inspector_window` | **P0** | Inspector |
| 10 | `get_inspector_state` | `inspector_window` | **P0** | Inspector Query |
| 11 | `get_scene_camera` | `scene_window` | **P0** | Scene View Query |
| 12 | `camera_orbit` | `scene_window` | **P0** | Scene View Action |
| 13 | `camera_zoom` | `scene_window` | **P0** | Scene View Action |
| 14 | `camera_fly` | `scene_window` | **P0** | Scene View Action |
| 15 | `scene_screenshot` | `scene_window` | **P0** | Scene View Action |
| 16 | `get_game_state` | `game_window` | **P0** | Game View Query |
| 17 | `game_screenshot` | `game_window` | **P0** | Game View Action |
| 18 | `get_console_logs` | `console_window` | **P0** | Console Query |
| 19 | `clear_console` | `console_window` | **P0** | Console Action |
| 20 | `get_editor_state` | `KairosEngine` | **P0** | Editor Control Query |
| 21 | `refresh_project` | `project_window` | P1 | Project/Asset Action |
| 22 | `duplicate_asset` | `project_window` | P1 | Project/Asset Action |
| 23 | `set_asset_field` | `inspector_window` | P1 | Inspector Action |
| 24 | `save_asset` | `inspector_window` | P1 | Inspector Action |
| 25 | `close_tab` | `docking_tab` | P1 | Layout Action |
| 26 | `open_tab` | `docking_tab` | P1 | Layout Action |
| 27 | `get_dock_layout` | `docking_tab` | P1 | Layout Query |
| 28 | `search_assets` | `project_window` | P2 | Project/Asset Query |
| 29 | `get_editor_preferences` | `preferences_window` | P2 | Settings Query |
| 30 | `set_editor_preference` | `preferences_window` | P2 | Settings Action |
| 31 | `audio_preview_play` | `inspector/audio` | P2 | Inspector Action |
| 32 | `audio_preview_pause` | `inspector/audio` | P2 | Inspector Action |
| 33 | `audio_preview_seek` | `inspector/audio` | P2 | Inspector Action |
| 34 | `material_set_shader` | `inspector/material` | P2 | Inspector Action |
| 35 | `material_set_texture` | `inspector/material` | P2 | Inspector Action |
| 36 | `batch_edit` | *(cross-module)* | P2 | Meta Tool |

---

## 3. Detailed Spec: P0 Tools

### 3.1 Project / Asset Query Tools

#### `get_project_tree`

Query the project file tree. Returns the hierarchical asset tree built from `ProjectPathGraph`.

| Field | Value |
|-------|-------|
| **Name** | `get_project_tree` |
| **Description** | Query the project asset tree. Returns a hierarchical listing of all project files and directories with their asset types, GUIDs, and paths. Optionally filter by path prefix or asset kind. |
| **Resource** | Also exposed as `editor://project/tree` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path prefix to filter by. e.g. 'assets/textures/' to get only that subtree. Omit or set to '/' for the entire tree."
    },
    "kind": {
      "type": "string",
      "enum": ["Directory", "Texture", "Mesh", "Material", "Audio", "Shader", "Script", "Document", "Toml", "Font"],
      "description": "Filter by asset kind. Omit to include all kinds."
    },
    "depth": {
      "type": "integer",
      "minimum": 0,
      "maximum": 10,
      "default": 2,
      "description": "Maximum recursion depth. 0 = only the requested node."
    }
  }
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "root": {
      "type": "object",
      "properties": {
        "guid": { "type": "string", "description": "Unique GUID of this node" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" },
        "children": {
          "type": "array",
          "items": { "$ref": "#/properties/root" }
        }
      },
      "required": ["guid", "name", "path", "kind"]
    }
  }
}
```

---

#### `get_asset_info`

Get detailed information about a specific asset, identified by path or GUID.

| Field | Value |
|-------|-------|
| **Name** | `get_asset_info` |
| **Description** | Look up a single asset by its filesystem path or GUID. Returns the asset's GUID, path, kind, and any inspector-relevant metadata. |
| **Resource** | `editor://assets/{guid}` or `editor://assets/{path}` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": {
      "type": "string",
      "description": "Asset GUID. Mutually exclusive with 'path'."
    },
    "path": {
      "type": "string",
      "description": "Project-relative path to the asset. Mutually exclusive with 'guid'."
    }
  },
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "name": { "type": "string" },
    "path": { "type": "string" },
    "asset_path": { "type": "string", "description": "Canonical asset file path (e.g. .texture vs .png)" },
    "kind": { "type": "string" },
    "parent_path": { "type": "string" },
    "children": {
      "type": "array",
      "items": { "type": "object", "properties": { "name": { "type": "string" }, "kind": { "type": "string" }, "path": { "type": "string" } } }
    }
  },
  "required": ["guid", "name", "path", "kind"]
}
```

---

#### `get_asset_registry`

List all registered assets from `AssetRegistry` (the GUID → path mapping).

| Field | Value |
|-------|-------|
| **Name** | `get_asset_registry` |
| **Description** | List all registered assets from the persistent asset registry. Returns a flat list of all {guid, path, kind} entries. |
| **Resource** | `editor://assets` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "kind": {
      "type": "string",
      "enum": ["Directory", "Texture", "Mesh", "Material", "Audio", "Shader", "Script", "Document", "Toml", "Font"],
      "description": "Filter by asset kind. Omit to include all."
    },
    "search": {
      "type": "string",
      "description": "Substring filter on path/name."
    },
    "limit": {
      "type": "integer",
      "default": 200,
      "description": "Max entries to return."
    }
  }
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "assets": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "guid": { "type": "string" },
          "path": { "type": "string" },
          "kind": { "type": "string" }
        },
        "required": ["guid", "path", "kind"]
      }
    },
    "total": { "type": "integer" }
  }
}
```

---

### 3.2 Project / Asset Action Tools

#### `select_asset`

| Field | Value |
|-------|-------|
| **Name** | `select_asset` |
| **Description** | Select an asset in the project window by its path or GUID. This highlights the asset in both the hierarchy panel and the content panel. Does NOT open the inspector (use `inspect_asset` for that). |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "path": { "type": "string" }
  },
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "selected": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "error": { "type": "string" }
  }
}
```

---

#### `open_asset`

| Field | Value |
|-------|-------|
| **Name** | `open_asset` |
| **Description** | Open an asset by path or GUID. Behavior depends on asset kind: Directory → navigate into it; Texture/Mesh/Material/Audio → select + open inspector (when implemented); Shader/Script/Document/Toml/Font → open in external IDE. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "path": { "type": "string" }
  },
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "action": { "type": "string", "description": "What happened: 'navigated', 'inspected', 'opened_in_ide'" },
    "node": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    }
  }
}
```

---

#### `create_asset`

| Field | Value |
|-------|-------|
| **Name** | `create_asset` |
| **Description** | Create a new asset in the project. The asset is created as a child of the specified parent directory node. Supported kinds: Directory, Texture, Mesh, Material, Audio, Shader, Script, Document, Toml. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "parent_path": {
      "type": "string",
      "description": "Path of the parent directory to create the asset in. Use '/' for project root."
    },
    "name": {
      "type": "string",
      "description": "Name of the new asset (without extension — the extension is derived from kind)."
    },
    "kind": {
      "type": "string",
      "enum": ["Directory", "Texture", "Mesh", "Material", "Audio", "Shader", "Script", "Document", "Toml"],
      "description": "Kind of asset to create. Font creation is not supported (fonts are imported)."
    }
  },
  "required": ["parent_path", "name", "kind"]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "created": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "error": { "type": "string" }
  }
}
```

---

#### `delete_asset`

| Field | Value |
|-------|-------|
| **Name** | `delete_asset` |
| **Description** | Delete an asset from the project by path or GUID. Deletes the file from disk and removes it from the asset registry. Directories are deleted recursively. **This is a destructive operation** — clients SHOULD prompt for confirmation. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "path": { "type": "string" }
  },
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "deleted": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "error": { "type": "string" }
  }
}
```

**Security:**
```json
{
  "annotations": {
    "destructive": true,
    "readOnlyHint": false
  }
}
```

---

#### `rename_asset`

| Field | Value |
|-------|-------|
| **Name** | `rename_asset` |
| **Description** | Rename an asset. The provided name should be the stem only (no extension); the extension is automatically derived from the asset kind. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "path": { "type": "string" },
    "new_name": {
      "type": "string",
      "description": "New name (stem only, without file extension)."
    }
  },
  "required": ["new_name"],
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "renamed": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "old_name": { "type": "string" },
        "new_name": { "type": "string" },
        "new_path": { "type": "string" }
      }
    },
    "error": { "type": "string" }
  }
}
```

---

### 3.3 Inspector Tools

#### `inspect_asset`

| Field | Value |
|-------|-------|
| **Name** | `inspect_asset` |
| **Description** | Select an asset and open the inspector panel for it. This is equivalent to `select_asset` + opening the Inspector tab. Returns the InspectorNodeInfo including the asset's kind-specific inspector data. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "guid": { "type": "string" },
    "path": { "type": "string" }
  },
  "oneOf": [
    { "required": ["guid"] },
    { "required": ["path"] }
  ]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "asset": {
      "type": "object",
      "properties": {
        "guid": { "type": "string" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "inspector_kind": {
      "type": "string",
      "enum": ["texture", "mesh", "material", "audio", "shader", "script", "document", "toml", "font", "directory", "unknown"],
      "description": "Which inspector type is active."
    }
  }
}
```

---

#### `get_inspector_state`

| Field | Value |
|-------|-------|
| **Name** | `get_inspector_state` |
| **Description** | Read the current state of the inspector window — which asset is selected and which inspector type is active. Returns null if nothing is selected. |
| **Priority** | P0 |
| **Resource** | `editor://inspector/selection` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "selected": {
      "type": ["object", "null"],
      "properties": {
        "guid": { "type": "string" },
        "name": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "inspector_kind": { "type": "string" }
  }
}
```

---

### 3.4 Scene View Tools

#### `get_scene_camera`

| Field | Value |
|-------|-------|
| **Name** | `get_scene_camera` |
| **Description** | Read the current state of the scene editor camera (an orbit camera). Returns position, pivot (look-at target), field of view, near/far planes, yaw, pitch, distance, and sensitivity parameters. |
| **Priority** | P0 |
| **Resource** | `editor://scene/camera` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "position": {
      "type": "object",
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" } },
      "description": "World-space camera position."
    },
    "pivot": {
      "type": "object",
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" } },
      "description": "Orbit pivot point (look-at target)."
    },
    "fov": { "type": "number", "description": "Vertical field of view in degrees." },
    "aspect": { "type": "number" },
    "near": { "type": "number" },
    "far": { "type": "number" },
    "yaw": { "type": "number", "description": "Horizontal rotation in radians." },
    "pitch": { "type": "number", "description": "Vertical rotation in radians." },
    "distance": { "type": "number", "description": "Distance from pivot to camera." },
    "viewport": {
      "type": "object",
      "properties": { "width": { "type": "integer" }, "height": { "type": "integer" } }
    }
  },
  "required": ["position", "pivot", "fov", "distance"]
}
```

---

#### `camera_orbit`

| Field | Value |
|-------|-------|
| **Name** | `camera_orbit` |
| **Description** | Rotate the scene camera around its pivot point. `dx` and `dy` are pixel-drag deltas; the tool scales them by orbit speed and delta time internally. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "dx": { "type": "number", "description": "Horizontal drag delta in screen pixels." },
    "dy": { "type": "number", "description": "Vertical drag delta in screen pixels." },
    "dt": {
      "type": "number",
      "default": 0.016,
      "description": "Delta time in seconds. Defaults to 1 frame at 60fps."
    }
  },
  "required": ["dx", "dy"]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "yaw": { "type": "number" },
    "pitch": { "type": "number" }
  }
}
```

---

#### `camera_zoom`

| Field | Value |
|-------|-------|
| **Name** | `camera_zoom` |
| **Description** | Zoom the scene camera in/out by adjusting distance from pivot. Positive `delta` = zoom in (closer), negative = zoom out. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "delta": { "type": "number", "description": "Scroll delta. Positive = zoom in." },
    "dt": {
      "type": "number",
      "default": 0.016,
      "description": "Delta time in seconds."
    }
  },
  "required": ["delta"]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "distance": { "type": "number" }
  }
}
```

---

#### `camera_fly`

| Field | Value |
|-------|-------|
| **Name** | `camera_fly` |
| **Description** | Fly the scene camera in world space (WASD-style movement). `right` and `forward` should be in [-1, 0, 1] representing input axes. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "right": {
      "type": "number",
      "description": "Right axis amount in [-1, 1]. Positive = right (D), Negative = left (A)."
    },
    "forward": {
      "type": "number",
      "description": "Forward axis amount in [-1, 1]. Positive = forward (W), Negative = back (S)."
    },
    "dt": {
      "type": "number",
      "default": 0.016
    }
  },
  "required": ["right", "forward"]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "pivot": {
      "type": "object",
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" } }
    }
  }
}
```

---

#### `scene_screenshot`

| Field | Value |
|-------|-------|
| **Name** | `scene_screenshot` |
| **Description** | Capture a screenshot of the scene editor viewport (3D view). Returns base64-encoded PNG. Note: the generic egui_mcp `screenshot` tool captures the entire editor window; this tool captures just the scene viewport render target. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "pixels_per_point": {
      "type": "number",
      "default": 1.0,
      "description": "Resolution multiplier. 2.0 = retina quality."
    },
    "save_path": {
      "type": "string",
      "description": "Optional file path to save the screenshot to (relative to project root)."
    }
  }
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "image": {
      "type": "object",
      "properties": {
        "data": { "type": "string", "contentMediaType": "image/png", "description": "Base64-encoded PNG data." },
        "width": { "type": "integer" },
        "height": { "type": "integer" }
      }
    }
  }
}
```

---

### 3.5 Game View Tools

#### `get_game_state`

| Field | Value |
|-------|-------|
| **Name** | `get_game_state` |
| **Description** | Read the current state of the game preview window, including viewport dimensions, display settings, and runtime status. |
| **Priority** | P0 |
| **Resource** | `editor://game/state` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "viewport": {
      "type": "object",
      "properties": { "width": { "type": "integer" }, "height": { "type": "integer" } }
    },
    "maximize_on_play": { "type": "boolean" },
    "stats_visible": { "type": "boolean" },
    "gizmos_visible": { "type": "boolean" },
    "scale": { "type": "number" },
    "is_running": { "type": "boolean", "description": "Whether the game is currently in play mode." }
  }
}
```

---

#### `game_screenshot`

| Field | Value |
|-------|-------|
| **Name** | `game_screenshot` |
| **Description** | Capture a screenshot of the game preview viewport. Returns base64-encoded PNG of the game's rendered output. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "pixels_per_point": {
      "type": "number",
      "default": 1.0
    },
    "save_path": {
      "type": "string",
      "description": "Optional file path to save the screenshot to."
    }
  }
}
```

**Output Schema:** Same as `scene_screenshot`.

---

### 3.6 Console Tools

#### `get_console_logs`

| Field | Value |
|-------|-------|
| **Name** | `get_console_logs` |
| **Description** | Query the editor console log buffer. Returns log entries matching optional level and text filters. The console captures `LogMessage` entries from the editor's `Log` struct (with level, message, caller location, and optional backtrace). |
| **Priority** | P0 |
| **Resource** | `editor://console/logs` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "level": {
      "type": "string",
      "enum": ["Info", "Warning", "Error"],
      "description": "Filter by log level."
    },
    "pattern": {
      "type": "string",
      "description": "Substring filter on the log message text."
    },
    "after": {
      "type": "integer",
      "default": 0,
      "description": "Skip the first N matching entries (for incremental reads)."
    },
    "limit": {
      "type": "integer",
      "default": 100,
      "maximum": 500,
      "description": "Maximum entries to return."
    }
  }
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "entries": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "level": { "type": "string" },
          "message": { "type": "string" },
          "caller": { "type": "string", "description": "file:line:column" }
        }
      }
    },
    "total": { "type": "integer", "description": "Total matching entries (regardless of limit)." }
  }
}
```

---

#### `clear_console`

| Field | Value |
|-------|-------|
| **Name** | `clear_console` |
| **Description** | Clear all entries from the console log buffer. |
| **Priority** | P0 |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "ok": { "type": "boolean" },
    "cleared": { "type": "integer", "description": "Number of entries cleared." }
  }
}
```

---

### 3.7 Editor Control

#### `get_editor_state`

| Field | Value |
|-------|-------|
| **Name** | `get_editor_state` |
| **Description** | Read the overall editor state: which tabs are open, what the current project root is, and connection metadata. |
| **Priority** | P0 |
| **Resource** | `editor://state` |

**Input Schema:**
```json
{
  "type": "object",
  "properties": {}
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "project_root": { "type": "string" },
    "open_tabs": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "title": { "type": "string" },
          "surface_id": { "type": "string" }
        }
      }
    },
    "selected_asset": {
      "type": ["object", "null"],
      "properties": {
        "guid": { "type": "string" },
        "path": { "type": "string" },
        "kind": { "type": "string" }
      }
    },
    "peer_info": {
      "type": "object",
      "properties": {
        "version": { "type": "string" },
        "uptime_seconds": { "type": "number" }
      }
    }
  }
}
```

---

## 4. Resources

MCP Resources expose read-only editor state through stable URIs. Resources support `resources/list`, `resources/read`, and optional `resources/subscribe` (for push updates when state changes).

### 4.1 Resource URI Catalog

| URI Pattern | Content Type | Description | Refresh |
|-------------|-------------|-------------|---------|
| `editor://state` | `application/json` | Overall editor status (open tabs, project root, selection) | On change |
| `editor://project/tree` | `application/json` | Full project tree (JSON format matching `get_project_tree` output) | On filesystem change |
| `editor://project/tree/{path}` | `application/json` | Subtree at `{path}` (URL-encoded project-relative path) | On filesystem change |
| `editor://assets` | `application/json` | Asset registry listing (all GUID→path→kind entries) | On asset create/delete/rename |
| `editor://assets/{guid}` | `application/json` | Single asset detail by GUID | On asset modification |
| `editor://inspector/selection` | `application/json` | Current inspector selection (null if nothing selected) | On selection change |
| `editor://scene/camera` | `application/json` | Scene camera state (position, pivot, fov, etc.) | Every frame while hovering |
| `editor://scene/viewport` | `application/json` | Scene viewport dimensions (width, height) | On resize |
| `editor://game/state` | `application/json` | Game window state and settings | On change |
| `editor://game/viewport` | `application/json` | Game viewport dimensions (width, height) | On resize |
| `editor://console/logs` | `application/json` | Console log buffer (all levels) | On new log entry |
| `editor://console/logs/{level}` | `application/json` | Console logs filtered by level (info/warning/error) | On new log entry |
| `editor://layout` | `application/json` | Current dock tab layout (surface/node/tab tree) | On layout change |
| `editor://preferences` | `application/json` | Editor preferences / style pages | On change |

### 4.2 Resource Implementation Notes

1. **URI encoding:** Path-based URIs (`{path}` segments) use URL-encoded project-relative paths. Example: `editor://project/tree/assets%2Ftextures%2F`.
2. **Subscription:** Resources marked "On change" SHOULD implement `resources/subscribe` so clients receive `notifications/resources/updated` when state changes. This enables reactive UIs and live dashboards.
3. **Resource content** should match the `OutputSchema` of the corresponding tool (e.g., `editor://scene/camera` returns the same shape as `get_scene_camera`'s output).

---

## 5. Tool Categorization

### 5.1 By Origin

```
Category A: Generic egui Tools (inherited from egui_mcp::UiServer)
├── attach, disconnect, status
├── query_tree, get_node
├── click, hover, scroll, drag, type_text, press_key
├── screenshot, resize, wait_for, batch
└── [Pass-through to AccessKit + egui event injection]

Category B: Editor-Specific Tools (kairos_editor_mcp::EditorTools)
├── Project/Asset: get_project_tree, get_asset_info, get_asset_registry,
│   select_asset, open_asset, create_asset, delete_asset, rename_asset
├── Inspector: inspect_asset, get_inspector_state
├── Scene View: get_scene_camera, camera_orbit, camera_zoom, camera_fly, scene_screenshot
├── Game View: get_game_state, game_screenshot
├── Console: get_console_logs, clear_console
└── Editor Control: get_editor_state
```

### 5.2 By Read/Write

```
Read-Only (Queries):
  get_project_tree, get_asset_info, get_asset_registry,
  get_inspector_state, get_scene_camera, get_game_state,
  get_console_logs, get_editor_state

Write (Actions):
  select_asset, open_asset, create_asset, delete_asset, rename_asset,
  inspect_asset, camera_orbit, camera_zoom, camera_fly,
  scene_screenshot, game_screenshot, clear_console
```

### 5.3 By Safety Profile

| Profile | Tools | Notes |
|---------|-------|-------|
| **Safe (read-only)** | All `get_*` tools, `*_screenshot` | No side effects. Safe to call repeatedly. |
| **Safe (UI state)** | `select_asset`, `camera_orbit`, `camera_zoom`, `camera_fly`, `clear_console` | Change UI state only; no filesystem impact. |
| **Caution (filesystem)** | `create_asset`, `delete_asset`, `rename_asset` | Modify files on disk. Clients SHOULD prompt for confirmation. `delete_asset` is marked `destructive: true`. |

---

## 6. Phase Breakdown

### Phase 1: MVP (P0 — 20 tools)

Focus: The minimal set needed for an LLM agent to understand and navigate a KairosEngine project.

| # | Tool |
|---|------|
| 1-3 | `attach`, `disconnect`, `status` (from egui_mcp) |
| 4 | `get_project_tree` |
| 5 | `get_asset_info` |
| 6 | `get_asset_registry` |
| 7 | `select_asset` |
| 8 | `open_asset` |
| 9 | `create_asset` |
| 10 | `delete_asset` |
| 11 | `rename_asset` |
| 12 | `inspect_asset` |
| 13 | `get_inspector_state` |
| 14 | `get_scene_camera` |
| 15 | `camera_orbit` |
| 16 | `camera_zoom` |
| 17 | `scene_screenshot` |
| 18 | `get_console_logs` |
| 19 | `clear_console` |
| 20 | `get_editor_state` |

**MVP Resources:** `editor://project/tree`, `editor://assets`, `editor://inspector/selection`, `editor://scene/camera`, `editor://console/logs`, `editor://state`

### Phase 2: Enhancement (P1 — 10 tools)

Focus: Richer interaction — modifying inspector fields, saving assets, layout management.

| # | Tool | Notes |
|---|------|-------|
| 21 | `camera_fly` | Scene navigation |
| 22 | `game_screenshot` | Game view capture |
| 23 | `get_game_state` | Game viewport/status |
| 24 | `refresh_project` | Rescan filesystem |
| 25 | `duplicate_asset` | Copy asset |
| 26 | `set_asset_field` | Modify inspector field values (depends on per-kind field schema) |
| 27 | `save_asset` | Persist inspector edits to disk |
| 28 | `close_tab` | Close an editor tab |
| 29 | `open_tab` | Open a specific tab (Inspector, Console, etc.) |
| 30 | `get_dock_layout` | Read tab layout |

**Enhancement Resources:** `editor://game/state`, `editor://layout`

### Phase 3: Complete (P2 — 8 tools)

Focus: Full editor control — preferences, search, audio preview, material editing, batch operations.

| # | Tool | Notes |
|---|------|-------|
| 31 | `search_assets` | Full-text search across project files |
| 32 | `get_editor_preferences` | Read Settings window state |
| 33 | `set_editor_preference` | Modify individual preference key |
| 34 | `audio_preview_play` | Start audio playback in inspector |
| 35 | `audio_preview_pause` | Stop audio playback |
| 36 | `audio_preview_seek` | Seek audio playback to position |
| 37 | `material_set_shader` | Change material's shader |
| 38 | `material_set_texture` | Assign texture to material |
| 39 | `batch_edit` | Batch multiple editor actions atomically |

**Complete Resources:** `editor://preferences`

---

## 7. Integration with egui_mcp

### Architecture (Design A reconfirmed)

```
┌──────────────────────────────────────────────────────┐
│ kairos_editor_mcp (binary)                           │
│                                                      │
│  Server (rmcp)                                       │
│  ├── lifecycle: attach/disconnect/status              │
│  │                                                   │
│  ├── UiServer (from egui_mcp)          ← generic     │
│  │   ├── query_tree, get_node                        │
│  │   ├── click, hover, scroll, drag                  │
│  │   ├── type_text, press_key                        │
│  │   ├── screenshot, resize                          │
│  │   └── wait_for, batch                             │
│  │                                                   │
│  └── EditorTools (from kairos_editor_mcp) ← semantic │
│      ├── project/asset queries + actions             │
│      ├── inspector queries                           │
│      ├── scene camera tools                          │
│      ├── game view tools                             │
│      ├── console tools                               │
│      └── editor control                              │
│                                                      │
│  Bridge → Transport → KairosEngine (TCP/in-process)   │
│  ├── egui_inspection channel (generic)               │
│  └── kairos_inspection channel (editor-specific)     │
└──────────────────────────────────────────────────────┘
```

### Tool Naming Convention

To avoid confusion between generic and editor tools:
- Generic tools keep their `egui_mcp` names (e.g., `screenshot` = full window screenshot)
- Editor tools use descriptive names (e.g., `scene_screenshot` = scene view only)
- Resources use `editor://` URI scheme

### Transport Design

Two channels multiplexed through a `KairosEditorTransport`:

```rust
struct KairosEditorTransport {
    egui_transport: FramedTransport,     // speaks egui_inspection protocol
    kairos_channel: mpsc::Sender<KairosRequest>,  // custom editor channel
}
```

The `kairos_channel` carries editor-specific requests (asset CRUD, camera control, console queries) that the generic AccessKit-based protocol cannot express.

---

## 8. Design Decisions & Rationale

| Decision | Rationale |
|----------|-----------|
| **P0 scope excludes `set_asset_field`** | Inspector field modification requires per-kind field schemas (Texture has size/format, Material has shader/texture/render state, etc.). The field schema design is complex enough to warrant its own ticket. P0 focuses on _navigation_ — the LLM can use generic egui tools to modify fields as a workaround. |
| **Separate `scene_screenshot` and `game_screenshot`** | These are distinct render targets in KairosEngine (scene view = editor camera, game view = game camera). The generic `screenshot` tool captures the whole window which is less useful. |
| **Resources expose data the tool returns** | Keeping resource and tool output schemas aligned simplifies implementation and reduces LLM confusion. |
| **Design A (separate server) confirmed** | The egui_mcp analysis (section 4.2) recommends Design A for clean separation and automatic upstream sync. Editor tools are a separate `ToolRouter` alongside `UiServer`. |
| **Hierarchy Window excluded from P0** | Currently a "TODO" stub in KairosEngine. No entity hierarchy system exists yet. This is a future tool when the ECS entity tree is implemented. |
| **`camera_fly` is P1 not P0** | Orbit + zoom are sufficient for basic scene navigation. Fly adds WASD-style movement which is nice but not essential for first inspection. |

---

## 9. Open Questions

These should be resolved in D1 (requirements) or subsequent tickets:

1. **P0 scope:** Is the 20-tool P0 cut correct, or should some P1 tools be promoted / P0 tools demoted?
2. **`set_asset_field` design:** How should per-kind field schemas be expressed in MCP tool input? As a flat `{field_name: value}` map, or typed per-kind tools (`set_texture_size`, `set_material_shader`, etc.)?
3. **Authorization model:** Should destructive tools like `delete_asset` require an MCP-level authorization handshake, or rely on the client's own confirmation UI?
4. **Resource subscriptions:** Should `resources/subscribe` be implemented in Phase 1, or deferred? Live updates are powerful but add complexity.
5. **`batch_edit` design:** Should batch operations be transactional (all-or-nothing) or best-effort?

---

## Appendix A: Module → Tool Mapping Reference

| KairosEngine Module | Data Held | Public Operations | Mapped Tools / Resources |
|---------------------|-----------|------------------|--------------------------|
| `project_path_tree` | `ProjectPathGraph` (petgraph of `ProjectTreeNode`) | new, refresh, get_node, get_parent, get_children, get_root, create_node, delete_node, rename_node, get_ancestors, sorted_children | `get_project_tree` (R: `editor://project/tree`), `create_asset`, `delete_asset`, `rename_asset` |
| `asset_registry` | `AssetRegistry` (`HashMap<Guid, PathBuf>` + reverse) | new, load, save, get_or_create_guid, get_guid, get_path, insert, delete | `get_asset_registry` (R: `editor://assets`), `get_asset_info` (R: `editor://assets/{guid}`) |
| `project_window` | `ProjectWindowModel` (style, registry, graph, selected, active, renaming, drag) | select_node, navigate_to, create_node, rename_node, delete_node, open_node, get_selected_node_info, drag_start/stop/consume | `select_asset`, `open_asset` |
| `inspector_window` | `InspectorNodeInfo` (name, kind, path, guid, Inspector trait obj) | set_selected, get_inspector, get_inspector_mut, on_close | `inspect_asset`, `get_inspector_state` (R: `editor://inspector/selection`) |
| `inspector/*` | Per-kind inspectors (audio, code, material, mesh, shader, texture, toml, etc.) | Inspector trait: draw, on_exit, render | `set_asset_field` (P1), `save_asset` (P1), audio preview tools (P2), material tools (P2) |
| `scene_window` | `SceneWindowModel` (style, rt_handle, width, height, camera, gizmos) + `SceneCamera` | orbit, zoom, fly, position, forward, right, transform, view_projection | `get_scene_camera` (R: `editor://scene/camera`), `camera_orbit`, `camera_zoom`, `camera_fly`, `scene_screenshot` |
| `game_window` | `GameWindowModel` (style, rt_handle, width, height) | update_size, register_view_bind, try_rece_texture_id | `get_game_state` (R: `editor://game/state`), `game_screenshot` |
| `console_window` | Reads `Log` (VecDeque<LogMessage>) | pop_front | `get_console_logs` (R: `editor://console/logs`), `clear_console` |
| `tool_bar` | `ToolBarModel` (style) + menu items | Menu buttons: About, Quit, New Scene, Preferences, Window→tabs | Covered by generic egui tools (click menu items) + `get_editor_state` |
| `preferences_window` | `PreferencesModel` (style, style_pages, selected_id) | set_selected_id, registe_ui_styles, update_style_page | `get_editor_preferences` (P2, R: `editor://preferences`), `set_editor_preference` (P2) |
| `docking_tab` | `DockState`, `WindowState` (position, size, rect) | Tab management, show inside surfaces | `close_tab` (P1), `open_tab` (P1), `get_dock_layout` (P1, R: `editor://layout`) |
| `hierarchy_window` | **TODO stub** — "TODO: Hierarchy" label | *(none yet)* | *(future: entity tree tools when implemented)* |

## Appendix B: Key Source Locations

```
kairos_engine/src/kairos_editor/
├── ui/
│   ├── hierarchy_window.rs         → Entity tree (TODO stub)
│   ├── inspector_window.rs         → Inspector container + InspectorNodeInfo
│   ├── inspector/
│   │   ├── audio.rs, code.rs, material.rs, mesh.rs,
│   │   │   shader.rs, texture.rs, toml.rs, document.rs,
│   │   │   directory.rs, font.rs, unknown.rs, creater.rs
│   ├── project_window.rs           → Project window + asset operations
│   ├── project_window/
│   │   ├── hierarchy_panel.rs      → Tree rendering (HierarchyPanel)
│   │   ├── content_panel.rs        → File grid rendering (ContentPanel)
│   │   └── context_menu.rs         → Right-click context menu
│   ├── scene_window.rs             → 3D scene viewport + camera input
│   ├── scene_window/
│   │   ├── gizmos.rs               → Editor gizmos (axes, grid)
│   ├── scene_camera.rs             → Orbit camera (SceneCamera)
│   ├── game_window.rs              → Game preview viewport
│   ├── console_window.rs           → Log output panel
│   ├── tool_bar.rs                 → Main menu bar
│   ├── preferences_window.rs       → Settings/Preferences window
│   ├── docking_tab.rs              → Tab docking infrastructure
│   └── ui.rs                       → Message enum, Drawer trait, Messager
├── project_path_tree.rs            → ProjectPathGraph (file tree)
├── asset_registry.rs               → AssetRegistry (GUID↔path)
└── serialize_asset.rs              → Asset save/load
```
