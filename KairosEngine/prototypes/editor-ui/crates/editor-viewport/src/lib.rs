//! Host-side 3D viewport for the merged editor UI prototype.
//!
//! The real editor does not draw 3D geometry in egui: the engine renders the
//! scene into an offscreen attachment and the editor binds that attachment to an
//! egui texture (`GraphicsCommand::bind_attachment_to_egui` in
//! `kairos_graphics`). This crate is the throwaway stand-in for that host
//! boundary and owns exactly the same responsibilities:
//!
//! * one offscreen `Rgba8UnormSrgb` render target per open viewport;
//! * a wgpu pipeline that renders the demo scene into it;
//! * registration of the target with `egui_wgpu::Renderer` so UI code can draw
//!   it as an `egui::TextureId`;
//! * the orbit camera and CPU picking the app drives.
//!
//! # Layer rule
//!
//! `editor-components` must never depend on this crate. Panels see only
//! `egui::TextureId` (through `editor_components_prototype::HostView`), so the
//! widget layer stays free of GPU and engine types. This crate likewise depends
//! on `egui`/`egui-wgpu` only: no `eframe`, no `winit`, no `editor-core`, no
//! `editor-components` and no engine crates.
//!
//! # Mapping to the real engine
//!
//! * engine offscreen render-graph attachment -> `ViewportTarget` in `renderer.rs`;
//! * `GraphicsCommand::bind_attachment_to_egui` ->
//!   `Renderer::register_native_texture`;
//! * engine "render viewports before BuildUi" -> [`ViewportHost::render`] called
//!   from the app's frame-logic phase (prototype: `eframe::App::logic`; production:
//!   RenderViewports);
//! * engine scene -> fullscreen-triangle SDF raymarch in `scene.wgsl` (checker
//!   grid ground plane at `y = 0` plus one cube at `(0, 0.5, 0)`);
//! * engine camera controller -> [`OrbitCamera`] / [`CameraInput`];
//! * engine picking -> CPU ray tests in `pick.rs`.
//!
//! # Usage
//!
//! ```no_run
//! use editor_viewport_prototype::{CameraInput, ViewportError, ViewportHost};
//!
//! fn draw_viewport(
//!     host: &mut ViewportHost,
//!     size: [u32; 2],
//! ) -> Result<egui::TextureId, ViewportError> {
//!     let texture_id = host.bind(0, size[0], size[1])?;
//!     host.apply_camera_input(
//!         0,
//!         CameraInput {
//!             orbit: [0.01, 0.0],
//!             ..CameraInput::default()
//!         },
//!     );
//!     host.render(0)?;
//!     Ok(texture_id)
//! }
//! ```
//!
//! # Threading
//!
//! The host is owned by the UI thread and creates no background threads. Render
//! work is recorded and submitted synchronously; `queue.submit` only queues the
//! GPU work, it does not wait for it. The app decides when to repaint while a
//! viewport is visible.
//!
//! # Colour space
//!
//! The offscreen target is `Rgba8UnormSrgb`, like the engine's LDR attachments.
//! egui-wgpu expects user textures to contain gamma-encoded bytes, so the host
//! registers an `Rgba8Unorm` view of the same texture. See `renderer.rs` for the
//! details; without it the viewport would render noticeably dark.
//!
//! # Errors
//!
//! Recoverable failures are returned as [`ViewportError`]; nothing in this crate
//! panics on bad input. `unsafe` is forbidden crate-wide.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod camera;
mod error;
mod host;
mod pick;
mod renderer;

pub use camera::{
    CameraInput, DEFAULT_FOV_Y, FAR, MAX_DISTANCE, MAX_FOV_Y, MAX_PITCH, MIN_DISTANCE, MIN_FOV_Y,
    MIN_PITCH, Mat4, NEAR, OrbitCamera,
};
pub use error::ViewportError;
pub use host::ViewportHost;
pub use pick::{
    CUBE_CENTER, CUBE_HALF_EXTENT, CUBE_OBJECT_ID, GROUND_OBJECT_ID, GROUND_Y, Ray, ndc_to_ray,
    pick_object, ray_aabb, ray_plane_y,
};
