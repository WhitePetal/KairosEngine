//! Host-owned view resources.
//!
//! The UI layer only sees the read-only [`HostView`] surface: an egui
//! `TextureId` per viewport, and the console lines. Everything GPU-shaped
//! (wgpu device/queue, render targets, camera pose, picking) stays here, exactly
//! like the real editor's `RenderPipeline` / `World` boundary. The components
//! crate cannot name `wgpu`, so this ownership is enforced by Cargo.

use editor_components_prototype::HostView;
use editor_core_prototype::ViewportId;
use editor_viewport_prototype::ViewportHost;
use egui::TextureId;

/// Maximum number of console lines kept in memory (the prototype's log ring).
const MAX_LOG_LINES: usize = 400;

/// Host resources shared with the UI through [`HostView`].
pub struct HostState {
    /// The wgpu viewport renderer; `None` only if the backend has no wgpu state
    /// (headless/unsupported), in which case the panel shows an explicit error.
    pub viewport: Option<ViewportHost>,
    /// Structured host log lines (what the old prototype printed at the bottom).
    log: Vec<String>,
}

impl HostState {
    /// Build the host resources from the eframe creation context.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut log = Vec::new();
        let viewport = match &cc.wgpu_render_state {
            Some(render_state) => match ViewportHost::new(render_state) {
                Ok(host) => {
                    log.push("wgpu viewport host ready (engine stand-in).".to_string());
                    Some(host)
                }
                Err(error) => {
                    log.push(format!("wgpu viewport host failed to initialize: {error}"));
                    None
                }
            },
            None => {
                log.push("no wgpu render state: 3D viewport disabled.".to_string());
                None
            }
        };
        Self { viewport, log }
    }

    /// Append one console line, keeping the ring bounded.
    pub fn log(&mut self, line: impl Into<String>) {
        self.log.push(line.into());
        if self.log.len() > MAX_LOG_LINES {
            self.log.drain(0..MAX_LOG_LINES / 4);
        }
    }
}

impl HostView for HostState {
    fn viewport_texture(&self, viewport: ViewportId) -> Option<TextureId> {
        self.viewport
            .as_ref()
            .and_then(|host| host.texture_id(viewport.0))
    }

    fn console_lines(&self) -> &[String] {
        &self.log
    }
}
