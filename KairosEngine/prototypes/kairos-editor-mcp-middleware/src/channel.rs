//! Channel transport layer: wire format and both ends of the mpsc channel.

use tokio::sync::{mpsc, oneshot};

use crate::types::{EditorCommand, EditorQuery, EditorResponse};

// ── Wire type (single enum for both queries and commands) ────────────────

/// A single message travelling from MCP Server → Editor render loop.
pub enum ChannelMessage {
    Query(EditorQuery, oneshot::Sender<EditorResponse>),
    Command(EditorCommand, oneshot::Sender<EditorResponse>),
}

// ── MCP-side handle ─────────────────────────────────────────────────────

/// Held by the MCP Server.  Sends queries/commands to the editor.
#[derive(Clone)]
pub struct EditorChannel {
    sender: mpsc::UnboundedSender<ChannelMessage>,
}

impl EditorChannel {
    pub fn new(sender: mpsc::UnboundedSender<ChannelMessage>) -> Self {
        Self { sender }
    }

    /// Send a read-only query and await the response.
    ///
    /// Panics if the channel is closed (editor crashed or shutdown).
    pub async fn query(&self, query: EditorQuery) -> EditorResponse {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ChannelMessage::Query(query, tx))
            .expect("editor channel closed");
        rx.await.unwrap_or(EditorResponse::Error("handler dropped".into()))
    }

    /// Send a mutating command and await the response.
    pub async fn command(&self, cmd: EditorCommand) -> EditorResponse {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(ChannelMessage::Command(cmd, tx))
            .expect("editor channel closed");
        rx.await.unwrap_or(EditorResponse::Error("handler dropped".into()))
    }
}

// ── Editor-side plugin ──────────────────────────────────────────────────

use crate::types::{Engine, Log, QueryHandler};

/// Held by the editor.  Processes incoming messages in the render loop.
pub struct EditorChannelPlugin {
    receiver: mpsc::UnboundedReceiver<ChannelMessage>,
    query_handlers: Vec<Box<dyn QueryHandler + Send>>,
}

impl EditorChannelPlugin {
    pub fn new(receiver: mpsc::UnboundedReceiver<ChannelMessage>) -> Self {
        Self {
            receiver,
            query_handlers: Vec::new(),
        }
    }

    /// Constructor without a receiver — for unit testing dispatch logic.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self::new(rx)
    }

    /// Register a QueryHandler (typically a Drawer that exposes state).
    pub fn add_handler(&mut self, handler: Box<dyn QueryHandler + Send>) {
        self.query_handlers.push(handler);
    }

    // ── Called from the render loop ─────────────────────────────────

    /// Process all pending **commands** (mutations).
    /// Should be called **before** `draw_ui()` so changes are visible this frame.
    pub fn process_commands(&mut self, _engine: &mut Engine, _log: &mut Log) {
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                ChannelMessage::Command(_cmd, reply) => {
                    // TODO: dispatch_command(cmd, engine, ui_ctx, log)
                    let _ = reply.send(EditorResponse::Ok);
                }
                ChannelMessage::Query(query, reply) => {
                    let resp = self.dispatch_query(&query, &Engine, &Log);
                    let _ = reply.send(resp);
                }
            }
        }
    }

    /// Dispatch a single query through registered handlers. First handler
    /// that returns `Some` wins.
    pub fn dispatch_query(&self, query: &EditorQuery, engine: &Engine, log: &Log) -> EditorResponse {
        for handler in &self.query_handlers {
            if let Some(resp) = handler.handle_query(query, engine, log) {
                return resp;
            }
        }
        EditorResponse::Error(format!("unhandled query: {:?}", query))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    /// A mock handler that returns a fixed project tree.
    struct MockProjectWindow {
        tree: Option<TreeNode>,
        selected: Option<AssetInfo>,
    }

    impl QueryHandler for MockProjectWindow {
        fn handle_query(
            &self,
            query: &EditorQuery,
            _engine: &Engine,
            _log: &Log,
        ) -> Option<EditorResponse> {
            match query {
                EditorQuery::GetProjectTree => {
                    self.tree.clone().map(|root| EditorResponse::ProjectTree { root })
                }
                EditorQuery::GetSelectedAsset => {
                    Some(EditorResponse::SelectedAsset(self.selected.clone()))
                }
                _ => None,
            }
        }
    }

    /// A mock handler for console logs.
    struct MockLogHandler;

    impl QueryHandler for MockLogHandler {
        fn handle_query(
            &self,
            query: &EditorQuery,
            _engine: &Engine,
            _log: &Log,
        ) -> Option<EditorResponse> {
            match query {
                EditorQuery::GetConsoleLogs { .. } => Some(EditorResponse::ConsoleLogs {
                    entries: vec![],
                    total: 0,
                }),
                _ => None,
            }
        }
    }

    // ── Unit tests: dispatch logic (no channels needed) ─────────────

    #[test]
    fn dispatch_first_handler_wins() {
        let mut plugin = EditorChannelPlugin::new_for_test();
        plugin.add_handler(Box::new(MockProjectWindow {
            tree: Some(TreeNode {
                guid: Guid(1),
                name: "root".into(),
                path: "/".into(),
                kind: AssetKind::Directory,
                children: vec![],
            }),
            selected: None,
        }));
        plugin.add_handler(Box::new(MockLogHandler));

        let resp = plugin.dispatch_query(&EditorQuery::GetProjectTree, &Engine, &Log);
        assert!(matches!(resp, EditorResponse::ProjectTree { .. }));
    }

    #[test]
    fn dispatch_falls_through_to_next_handler() {
        let mut plugin = EditorChannelPlugin::new_for_test();
        plugin.add_handler(Box::new(MockLogHandler));
        plugin.add_handler(Box::new(MockProjectWindow {
            tree: Some(TreeNode {
                guid: Guid(1),
                name: "root".into(),
                path: "/".into(),
                kind: AssetKind::Directory,
                children: vec![],
            }),
            selected: None,
        }));

        let resp = plugin.dispatch_query(&EditorQuery::GetProjectTree, &Engine, &Log);
        assert!(matches!(resp, EditorResponse::ProjectTree { .. }));
    }

    #[test]
    fn dispatch_unhandled_returns_error() {
        let mut plugin = EditorChannelPlugin::new_for_test();
        plugin.add_handler(Box::new(MockLogHandler));

        let resp = plugin.dispatch_query(&EditorQuery::GetProjectTree, &Engine, &Log);
        assert!(matches!(resp, EditorResponse::Error(_)));
    }

    #[test]
    fn dispatch_no_handlers_returns_error() {
        let plugin = EditorChannelPlugin::new_for_test();
        let resp = plugin.dispatch_query(&EditorQuery::GetProjectTree, &Engine, &Log);
        assert!(matches!(resp, EditorResponse::Error(_)));
    }

    #[test]
    fn dispatch_returns_selected_asset() {
        let mut plugin = EditorChannelPlugin::new_for_test();
        plugin.add_handler(Box::new(MockProjectWindow {
            tree: None,
            selected: Some(AssetInfo {
                guid: Guid(42),
                name: "test.mat".into(),
                path: "Assets/test.mat".into(),
                kind: AssetKind::Material,
                asset_path: None,
                parent_path: Some("Assets".into()),
                children: vec![],
            }),
        }));

        let resp = plugin.dispatch_query(&EditorQuery::GetSelectedAsset, &Engine, &Log);
        assert!(matches!(resp, EditorResponse::SelectedAsset(Some(_))));
    }

    // ── Integration test: full channel round-trip ──────────────────

    #[tokio::test]
    async fn channel_roundtrip_query_and_command() {
        let (tx, mut rx) = mpsc::unbounded_channel::<ChannelMessage>();
        let channel = EditorChannel::new(tx);

        // Simulated render loop running in a background task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    ChannelMessage::Query(q, reply) => {
                        let resp = match q {
                            EditorQuery::GetProjectTree => EditorResponse::ProjectTree {
                                root: TreeNode {
                                    guid: Guid(1),
                                    name: "test".into(),
                                    path: "test".into(),
                                    kind: AssetKind::Directory,
                                    children: vec![],
                                },
                            },
                            _ => EditorResponse::Error("unhandled".into()),
                        };
                        let _ = reply.send(resp);
                    }
                    ChannelMessage::Command(_cmd, reply) => {
                        let _ = reply.send(EditorResponse::Ok);
                    }
                }
            }
        });

        let resp = channel.query(EditorQuery::GetProjectTree).await;
        assert!(matches!(resp, EditorResponse::ProjectTree { .. }));

        let resp = channel.command(EditorCommand::ClearConsole).await;
        assert!(matches!(resp, EditorResponse::Ok));
    }
}
