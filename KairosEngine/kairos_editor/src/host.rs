//! The editor's layer on the engine's frame loop.
//!
//! The loop ([`kairos_engine::runtime`]) owns the window, the swapchain and the
//! frame boundary; this layer owns the editor's egui state and its pass.
//!
//! This is a **private copy** of the egui glue, not a shared abstraction:
//! `kairos_editor` is the legacy host, slated for retirement, and it must not
//! shape the architecture of `kairos_editor_app`, where the same code lives for
//! good. When this crate goes, the copy goes with it.

use std::error::Error;

#[cfg(target_os = "macos")]
use std::path::Path;

use egui::ViewportId;
use kairos_engine::{
    Engine,
    graphics::{
        attachment::{
            Attachment, AttachmentLoadAction, AttachmentStoreAction, InternalAttachmentId,
        },
        graphics_graph::{GraphicsCommand, graphics_node::ColorAttachmentBind},
    },
    math::float4x4,
    runtime::{EventOutcome, FrameOutput, Layer, RuntimeEvent},
};
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoopProxy},
    window::{Icon, Window},
};

use crate::{Editor, ui::paths};

/// The editor's layer: egui state plus the UI pass.
pub struct EditorLayer {
    editor: Editor,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
}

impl EditorLayer {
    /// Builds the editor on the loop's engine and wires egui's repaint requests
    /// into the loop.
    pub fn new(
        engine: &mut Engine,
        proxy: &EventLoopProxy<RuntimeEvent>,
    ) -> Result<Self, Box<dyn Error>> {
        let egui_ctx = egui::Context::default();
        egui_extras::install_image_loaders(&egui_ctx);

        // egui asks for frames on its own schedule (an animation, a drag). The
        // loop only knows "redraw after this delay". Only the root viewport is
        // real: this host opens no auxiliary egui viewports.
        let egui_event_proxy = proxy.clone();
        egui_ctx.set_request_repaint_callback(move |info| {
            if info.viewport_id != ViewportId::ROOT {
                return;
            }

            let _ = egui_event_proxy.send_event(RuntimeEvent::RequestRedraw { delay: info.delay });
        });

        let editor = Editor::new(&egui_ctx, engine)?;

        Ok(Self {
            editor,
            egui_ctx,
            egui_state: None,
        })
    }

    /// Builds the egui frame's graphics command: one render pass that loads the
    /// engine's frame buffer attachment and draws the tessellated UI over it.
    fn egui_command(
        &self,
        size_in_pixels: [u32; 2],
        pixels_per_point: f32,
        clipped_primitives: Vec<egui::ClippedPrimitive>,
        set_textures: Vec<(epaint::TextureId, epaint::ImageDelta)>,
        free_textures: Vec<epaint::TextureId>,
    ) -> GraphicsCommand {
        let mut command = GraphicsCommand::new(2, 2, 0, 4);
        let frame_buffer_attachment =
            Attachment::from_internal_id(InternalAttachmentId::FrameBufferColorAttachment);
        let frame_buffer_attachment_id = command.create_color_attachment(frame_buffer_attachment);
        let frame_buffer_attachment_bind = ColorAttachmentBind::new(
            frame_buffer_attachment_id,
            AttachmentLoadAction::LoadClear,
            AttachmentStoreAction::Store,
        );
        let view_projection_id = command.set_view_projection_matrix(float4x4::IDENTITY);
        command.begin_render_pass(
            Some("Egui Graphics Render Pass"),
            vec![frame_buffer_attachment_bind],
            None,
            view_projection_id,
            0,
        );

        command.draw_egui(
            clipped_primitives,
            egui_wgpu::ScreenDescriptor {
                size_in_pixels,
                pixels_per_point,
            },
            set_textures,
        );
        command.end_render_pass();
        command.output_to_framebuffer(frame_buffer_attachment_id, free_textures);

        command
    }
}

impl Layer for EditorLayer {
    fn window_created(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &std::sync::Arc<Window>,
        max_texture_side: u32,
    ) {
        let mut state = egui_winit::State::new(
            self.egui_ctx.clone(),
            ViewportId::ROOT,
            event_loop,
            Some(window.scale_factor() as f32),
            window.theme(),
            None,
        );
        state.set_max_texture_side(max_texture_side as usize);
        self.egui_state = Some(state);

        brand_window(window);
    }

    fn window_event(&mut self, window: &Window, event: &WindowEvent) -> EventOutcome {
        let Some(state) = self.egui_state.as_mut() else {
            return EventOutcome::default();
        };

        let response = state.on_window_event(window, event);

        EventOutcome {
            consumed: response.consumed,
            repaint: response.repaint,
        }
    }

    fn render(
        &mut self,
        engine: &mut Engine,
        window: &Window,
        size_in_pixels: [u32; 2],
        out: &mut Vec<GraphicsCommand>,
    ) -> FrameOutput {
        let Some(state) = self.egui_state.as_mut() else {
            return FrameOutput::default();
        };

        let raw_input = state.take_egui_input(window);

        let full_output = self.egui_ctx.run_ui(raw_input, |ui| {
            // The 3D viewports are rendered before the panels that display
            // them, so a panel samples this frame's output, not the previous
            // one.
            out.append(&mut self.editor.render_ui(engine));
            self.editor.handle_ui(engine, ui);
            self.editor.draw_ui(engine, ui);
        });

        state.handle_platform_output(window, full_output.platform_output);

        let mut output = FrameOutput::default();

        if let Some(root_output) = full_output.viewport_output.get(&ViewportId::ROOT) {
            output.close_requested = root_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Close));
            output.repaint_delay = Some(root_output.repaint_delay);

            let mut actions_requested = Vec::new();
            let viewport_info = state
                .egui_input_mut()
                .viewports
                .entry(ViewportId::ROOT)
                .or_default();

            egui_winit::process_viewport_commands(
                &self.egui_ctx,
                viewport_info,
                root_output.commands.iter().cloned(),
                window,
                &mut actions_requested,
            );

            if !actions_requested.is_empty() {
                log::debug!("ignored viewport actions: {actions_requested:?}");
            }
        }

        let pixels_per_point = full_output.pixels_per_point;
        let clipped_primitives = self
            .egui_ctx
            .tessellate(full_output.shapes, pixels_per_point);

        out.push(self.egui_command(
            size_in_pixels,
            pixels_per_point,
            clipped_primitives,
            full_output.textures_delta.set,
            full_output.textures_delta.free,
        ));

        output
    }

    fn on_exit(&mut self, engine: &mut Engine) {
        self.editor.on_exit(engine);
    }
}

/// Loads the engine's window icon, if the packaged asset is present.
pub fn load_icon() -> Option<Icon> {
    let bytes = std::fs::read(paths::PATH_ENGINE_ICON).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Icon::from_rgba(image.into_raw(), width, height).ok()
}

#[cfg(target_os = "windows")]
fn brand_window(window: &Window) {
    use winit::platform::windows::WindowExtWindows;

    window.set_taskbar_icon(load_icon());
}

#[cfg(target_os = "macos")]
fn brand_window(_window: &Window) {
    set_macos_dock_icon(paths::PATH_ENGINE_ICON);
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn brand_window(_window: &Window) {}

#[cfg(target_os = "macos")]
fn set_macos_dock_icon(path: impl AsRef<Path>) {
    use objc2::{AnyThread as _, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSString;

    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("cannot set the macOS Dock icon outside the main thread");
        return;
    };

    let path = path.as_ref();
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path = path.to_string_lossy();
    let path = NSString::from_str(&path);

    let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) else {
        log::warn!("failed to load the macOS Dock icon from {path}");
        return;
    };

    let app = NSApplication::sharedApplication(mtm);

    // SAFETY: AppKit owns the application object, and `image` is a valid NSImage.
    unsafe { app.setApplicationIconImage(Some(&image)) };
}
