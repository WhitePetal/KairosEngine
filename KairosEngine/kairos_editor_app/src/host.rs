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
use kairos_editor_widgets::color_picker::eyedropper::{self, Outcome, Update};
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

use crate::eyedropper::ScreenPicker;
use crate::{app::EditorApp, branding};

/// The app's layer: egui state plus the panel pass.
pub struct EditorLayer {
    app: EditorApp,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    /// The host half of the color picker's eyedropper: the OS pointer, the
    /// desktop input mask and the loupe. Windows, X11 and macOS's
    /// ScreenCaptureKit path draw the loupe; the macOS fallback hands the pick
    /// to AppKit.
    eyedropper: ScreenPicker,
    /// Keeps the release that ends a pick away from egui (see
    /// [`ReleaseGuard`]).
    release: ReleaseGuard,
    /// The session whose terminal update was already published. If the field
    /// stopped drawing (its panel closed mid-pick), the widget has not consumed
    /// that update yet; starting a second sampler for the same session would
    /// show the pick again. Wait for a field or the session timeout instead.
    finished: Option<u64>,
}

/// Keeps the release that ends a pick away from egui.
///
/// Windows, X11 and macOS's ScreenCaptureKit overlay hide the confirming press
/// from every window, but the matching release is still pending when the pick
/// stops. egui's pointer never
/// moved during the pick, so a release at that stale position lands on the
/// eyedropper button (an earlier implementation measured it starting a new
/// pick). The guard swallows pointer events until two consecutive frames see no
/// mouse button down: the first clear frame catches a release still in flight,
/// the second guarantees the event loop has drained it.
#[derive(Default)]
struct ReleaseGuard {
    swallowing: bool,
    settling: bool,
}

impl ReleaseGuard {
    /// Hold pointer events from the end of a masked pick.
    fn arm(&mut self) {
        self.swallowing = true;
        self.settling = false;
    }

    /// Let pointer events through at once; used when the backend consumed the
    /// whole click itself (the macOS NSColorSampler fallback).
    fn disarm(&mut self) {
        self.swallowing = false;
        self.settling = false;
    }

    /// One frame of the guard's life: feed it the current mouse-button state.
    fn update(&mut self, button_down: bool) {
        if !self.swallowing {
            return;
        }
        if button_down {
            self.settling = false;
        } else if self.settling {
            self.disarm();
        } else {
            self.settling = true;
        }
    }

    fn swallows(&self) -> bool {
        self.swallowing
    }
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
            eyedropper: ScreenPicker::new(),
            release: ReleaseGuard::default(),
            finished: None,
        })
    }

    /// Serve the color field's pick with the OS pointer.
    ///
    /// One `Update` per frame feeds the widget: a live colour on Windows, X11
    /// and macOS's ScreenCaptureKit path, the result only at the end on the
    /// macOS system-sampler fallback. A `Committed` or `Cancelled` update ends
    /// the pick and hides the loupe. The widget owns the session, so a pick that disappears (Esc
    /// in the window, the picker closing) stops the sampler here.
    fn publish_eyedropper(&mut self, window: &Window) -> bool {
        let Some(session) = eyedropper::session(&self.egui_ctx) else {
            if self.eyedropper.is_running() {
                kairos_log::debug!(target: "kairos_editor", "eyedropper: no session, stopping");
            }
            self.eyedropper.stop(Some(window));
            self.finished = None;
            return false;
        };
        if self.finished == Some(session.id) {
            // The terminal update is still waiting for the field that owns it.
            return false;
        }

        if !self.eyedropper.is_running() {
            kairos_log::debug!(
                target: "kairos_editor",
                "eyedropper: start session {} for {:?}",
                session.id,
                session.field
            );
        }
        if !self.eyedropper.is_running() && !self.eyedropper.start(window) {
            eyedropper::publish(
                &self.egui_ctx,
                Update {
                    id: session.id,
                    field: session.field,
                    srgb: None,
                    outcome: Outcome::Cancelled,
                },
            );
            self.finished = Some(session.id);
            return false;
        }

        let Some(sample) = self.eyedropper.poll() else {
            return false;
        };
        eyedropper::publish(
            &self.egui_ctx,
            Update {
                id: session.id,
                field: session.field,
                srgb: sample.srgb,
                outcome: sample.outcome,
            },
        );
        if sample.outcome == Outcome::Picking {
            return false;
        }
        let hex = sample.srgb.map_or_else(
            || "no colour".to_owned(),
            |[red, green, blue, _]| format!("#{red:02X}{green:02X}{blue:02X}"),
        );
        kairos_log::debug!(
            target: "kairos_editor",
            "eyedropper: {:?} session {} at {}",
            sample.outcome,
            session.id,
            hex
        );
        self.eyedropper.stop(Some(window));
        // The click that ended the pick may still have a release pending. The
        // Windows/X11 masks and the macOS ScreenCaptureKit overlay consume it,
        // so their release must not reach egui; the NSColorSampler fallback
        // owns the whole click.
        if self.eyedropper.masks_pointer() {
            self.release.arm();
        } else {
            self.release.disarm();
        }
        self.finished = Some(session.id);
        sample.outcome == Outcome::Committed
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

        // Tell the color fields whether this host can read the screen; without
        // it the eyedropper keeps its "not available" tooltip. This runs after
        // the window exists because the Linux backend can only tell Wayland
        // apart by its raw window handle.
        let supported = ScreenPicker::available_for(window);
        eyedropper::set_supported(&self.egui_ctx, supported);
        if !supported {
            kairos_log::debug!(
                target: "kairos_editor",
                "eyedropper: this display has no desktop sampler"
            );
        }

        // The window exists, so the desktop can be given the app's own icon: the
        // window icon from `RuntimeConfig` does not reach the Dock or the
        // taskbar.
        branding::brand_app(window);
    }

    fn window_event(&mut self, window: &Window, event: &WindowEvent) -> EventOutcome {
        // A pick owns the mouse: the click that confirms it must not also press
        // whatever widget sits under the pointer. The platform mask/overlay (or
        // the macOS system-sampler fallback) discards button events, and the moves that
        // keep the cursor alive arrive here with the capture held - none of them
        // may reach egui. This filter also keeps the *release* that follows out:
        // egui's pointer never moved during the pick, so a stray release would
        // land on the eyedropper button and start a new pick.
        if (self.eyedropper.is_running() || self.release.swallows())
            && matches!(
                event,
                WindowEvent::MouseInput { .. }
                    | WindowEvent::CursorMoved { .. }
                    | WindowEvent::CursorEntered { .. }
                    | WindowEvent::CursorLeft { .. }
            )
        {
            return EventOutcome {
                consumed: true,
                repaint: false,
            };
        }

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
        if self.egui_state.is_none() {
            return FrameOutput::default();
        }

        // A pick reads the OS pointer, not egui's: the pointer may be over
        // another window entirely, and the sample feeds this frame's UI.
        self.publish_eyedropper(window);
        if self.release.swallows() {
            let button_down = self.eyedropper.any_button_down();
            self.release.update(button_down);
        }

        let state = self
            .egui_state
            .as_mut()
            .expect("the egui state was checked above");

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

#[cfg(test)]
mod tests {
    use super::ReleaseGuard;

    #[test]
    fn the_release_guard_swallows_until_two_clear_frames_after_the_button_is_up() {
        let mut guard = ReleaseGuard::default();
        guard.arm();

        guard.update(true);
        assert!(guard.swallows());

        // The first clear frame may still be followed by a release event that
        // was already in flight, so it does not release the guard by itself.
        guard.update(false);
        assert!(guard.swallows());

        // A button that goes down again restarts the count.
        guard.update(true);
        guard.update(false);
        assert!(guard.swallows());
        guard.update(false);
        assert!(!guard.swallows());
    }

    #[test]
    fn the_release_guard_is_inert_when_disarmed() {
        let mut guard = ReleaseGuard::default();
        guard.update(true);
        assert!(!guard.swallows());
        guard.update(false);
        assert!(!guard.swallows());

        guard.arm();
        guard.disarm();
        guard.update(false);
        assert!(!guard.swallows());
    }
}
