//! Core type definitions: queries, commands, responses, and the QueryHandler trait.
//!
//! These types are the **API contract** between the MCP server and the editor.
//! Every query/command is a single enum variant for type safety.

use serde::{Deserialize, Serialize};

// ── GUID / AssetKind (minimal stand-ins for the prototype) ──────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guid(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Directory,
    Texture,
    Material,
    Mesh,
    Shader,
    Audio,
    Font,
    Toml,
    Code,
    Document,
    Scene,
    Unknown,
}

// ── Snapshot types (pure data, no references into editor memory) ────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub guid: Guid,
    pub name: String,
    pub path: String,
    pub kind: AssetKind,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetInfo {
    pub guid: Guid,
    pub name: String,
    pub path: String,
    pub kind: AssetKind,
    pub asset_path: Option<String>,
    pub parent_path: Option<String>,
    pub children: Vec<AssetChildEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetChildEntry {
    pub name: String,
    pub path: String,
    pub kind: AssetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    pub guid: Guid,
    pub path: String,
    pub kind: AssetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraState {
    pub position: [f64; 3],
    pub pivot: [f64; 3],
    pub fov: f64,
    pub aspect: f64,
    pub near: f64,
    pub far: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectorState {
    pub selected: Option<AssetInfo>,
    pub inspector_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameViewState {
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorTabInfo {
    pub name: String,
    pub title: String,
    pub surface_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorState {
    pub project_root: String,
    pub open_tabs: Vec<EditorTabInfo>,
    pub selected_asset: Option<AssetInfo>,
    pub peer_version: String,
    pub uptime_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub caller: Option<String>,
    pub timestamp: u64,
}

// ── EditorQuery (read-only) ─────────────────────────────────────────────

/// Read-only state queries.  Each variant is handled by one `QueryHandler`.
#[derive(Debug, Clone)]
pub enum EditorQuery {
    /// Get the full project file tree.
    GetProjectTree,
    /// Get detailed info for a specific asset (by GUID or path).
    GetAssetInfo { guid: Option<Guid>, path: Option<String> },
    /// List registered assets, optionally filtered by kind / search / limit.
    GetAssetRegistry {
        kind: Option<AssetKind>,
        search: Option<String>,
        limit: Option<usize>,
    },
    /// Get the currently selected asset in the project window.
    GetSelectedAsset,
    /// Get the current inspector window state.
    GetInspectorState,
    /// Get the scene view camera state. (v2 — requires GPU)
    GetSceneCamera,
    /// Get the game viewport state (dimensions, running status).
    GetGameState,
    /// Get filtered console log entries.
    GetConsoleLogs {
        level: Option<String>,
        pattern: Option<String>,
        after: Option<u64>,
        limit: Option<usize>,
    },
    /// Get the overall editor state (tabs, selection, uptime).
    GetEditorState,
}

// ── EditorCommand (side effects) ────────────────────────────────────────

/// Mutating commands that need `&mut` access to editor state.
#[derive(Debug, Clone)]
pub enum EditorCommand {
    /// Select an asset in the project window.
    SelectAsset { guid: Option<Guid>, path: Option<String> },
    /// Create a new asset.
    CreateAsset {
        parent_path: String,
        name: String,
        kind: AssetKind,
    },
    /// Delete an asset (and its file).
    DeleteAsset { guid: Option<Guid>, path: Option<String> },
    /// Rename an asset.
    RenameAsset { guid: Guid, new_name: String },
    /// Refresh the project tree (re-scan filesystem).
    RefreshProject,
    /// Orbit the scene camera.
    CameraOrbit { dx: f64, dy: f64, dt: Option<f64> },
    /// Zoom the scene camera.
    CameraZoom { delta: f64, dt: Option<f64> },
    /// Fly the scene camera (WASD-style).
    CameraFly { right: Option<f64>, forward: Option<f64>, dt: Option<f64> },
    /// Clear the console log buffer.
    ClearConsole,
}

// ── EditorResponse ──────────────────────────────────────────────────────

/// The result of a query or command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorResponse {
    // Success variants — one per meaningful response shape
    ProjectTree { root: TreeNode },
    AssetInfo(AssetInfo),
    AssetRegistry { assets: Vec<AssetEntry>, total: usize },
    SelectedAsset(Option<AssetInfo>),
    InspectorState { selected: Option<AssetInfo>, inspector_kind: Option<String> },
    SceneCamera(CameraState),
    GameState(GameViewState),
    ConsoleLogs { entries: Vec<LogEntry>, total: usize },
    EditorState(EditorState),
    /// Generic ok (for commands that don't return data).
    Ok,
    /// Command succeeded with a result message.
    OkWith(String),
    /// An error occurred.
    Error(String),
}

// ── QueryHandler trait ──────────────────────────────────────────────────

/// Implemented by Drawers that expose read-only state to the MCP server.
///
/// Each drawer inspects the query — if it can answer it, it returns `Some`;
/// otherwise `None` so the next drawer can try.
///
/// In the real editor, `Engine` and `Log` are the actual KairosEngine types.
/// In this prototype they are stand-in unit structs.
pub struct Engine;
pub struct Log;

pub trait QueryHandler: Send + Sync {
    /// Attempt to handle a query.  Return `Some(response)` if this handler
    /// owns the relevant state, or `None` to let another handler try.
    fn handle_query(&self, query: &EditorQuery, engine: &Engine, log: &Log) -> Option<EditorResponse>;
}
