//! The editor app's layer on the engine's frame loop.
//!
//! The loop ([`kairos_engine::runtime`]) owns the window, the swapchain and the
//! frame boundary. This layer owns the editor app's egui state and its pass:
//! raw input from winit, [`egui::Context::run_ui`], and the tessellated output
//! turned into a [`GraphicsCommand`].
//!
//! The egui glue lives here, with the host that keeps it. The legacy
//! `kairos_editor` has its own private copy; sharing it would let a crate that
//! is being retired shape this one.

use std::error::Error;

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
    window::Window,
};

use crate::app::EditorApp;

/// The app's layer: egui state plus the panel pass.
pub struct EditorLayer {
    app: EditorApp,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
}

impl EditorLayer {
    /// Builds the app and wires egui's repaint requests into the engine loop.
    ///
    /// The engine is the loop's, so it is not needed here; it arrives with every
    /// UI pass.
    pub fn new(proxy: &EventLoopProxy<RuntimeEvent>) -> Result<Self, Box<dyn Error>> {
        let egui_ctx = egui::Context::default();
        egui_extras::install_image_loaders(&egui_ctx);

        // egui asks for frames on its own schedule (an animation, a drag). The
        // engine's loop only knows "redraw after this delay", so translate the
        // request into a runtime event. Only the root viewport is real: the app
        // opens no auxiliary egui viewports.
        let egui_event_proxy = proxy.clone();
        egui_ctx.set_request_repaint_callback(move |info| {
            if info.viewport_id != ViewportId::ROOT {
                return;
            }

            let _ = egui_event_proxy.send_event(RuntimeEvent::RequestRedraw { delay: info.delay });
        });

        let app = EditorApp::new(&egui_ctx);

        Ok(Self {
            app,
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
            self.app.ui(engine, ui);
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
                kairos_log::debug!(
                    target: "kairos_editor",
                    "ignored viewport actions: {actions_requested:?}"
                );
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
}
