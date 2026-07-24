//! Prototype: kairos_editor_mcp middleware types.
//!
//! This module defines the core abstractions for communication between
//! the MCP Server and KairosEngine Editor.  It is intentionally pure-data
//! and does not depend on KairosEngine internals — only on `serde` for
//! snapshot serialization and `tokio::sync` for channel types.
//!
//! ## Architecture
//!
//! ```text
//! MCP Server                     Editor (render loop)
//! ─────────                      ────────────────────
//! EditorChannel                  EditorChannelPlugin
//!   .query(EditorQuery)  ──mpsc──>  process_queries()
//!   .command(EditorCmd)  ──mpsc──>  process_commands()
//! ```
//!
//! Queries are dispatched to `QueryHandler` trait impls on Drawers.
//! Commands are dispatched directly in the render loop with `&mut` access.

pub mod channel;
pub mod types;

pub use channel::{ChannelMessage, EditorChannel, EditorChannelPlugin};
pub use types::*;
