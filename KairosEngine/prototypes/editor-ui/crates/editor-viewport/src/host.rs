//! The viewport host API consumed by the editor UI.
//!
//! [`ViewportHost`] is the only type the app layer needs: it owns one wgpu
//! offscreen target per open viewport, the orbit cameras, selection state and
//! GPU picking, and it exposes egui handles only. The app never sees a wgpu type
//! from this crate.
//!
//! # Frame flow
//!
//! 1. [`ViewportHost::bind`] with the panel's physical pixel size; it creates the
//!    target on first use and resizes it only when the size actually changes.
//! 2. Apply input with [`ViewportHost::apply_camera_input`] and
//!    [`ViewportHost::pick`], and mirror selection into the scene with
//!    [`ViewportHost::set_selected`].
//! 3. Call [`ViewportHost::render`] from the frame-logic phase (prototype:
//!    `eframe::App::logic`; production: RenderViewports) before egui paints,
//!    mirroring the engine's "render viewports before BuildUi" step. It records
//!    and submits its own command buffer and never blocks on the GPU.
//! 4. Draw the returned `egui::TextureId` as an image.
//! 5. Call [`ViewportHost::destroy`] when the viewport closes.
//!
//! Resizing reallocates the egui texture, so a `TextureId` obtained before a
//! resize must not be cached; use the id returned by `bind` (or
//! [`ViewportHost::texture_id`]) every frame.

use std::collections::HashMap;

use crate::camera::{CameraInput, OrbitCamera};
use crate::error::ViewportError;
use crate::pick::pick_object;
use crate::renderer::{ViewportRenderer, ViewportTarget};

/// Owns the GPU resources for every open viewport and exposes only egui handles.
///
/// One host supports any number of viewports keyed by an application-defined
/// `u32` (normally the panel/viewport id). Each viewport has its own offscreen
/// target, camera and selection state; the scene pipeline is shared.
pub struct ViewportHost {
    renderer: ViewportRenderer,
    viewports: HashMap<u32, ViewportEntry>,
}

struct ViewportEntry {
    target: ViewportTarget,
    camera: OrbitCamera,
    selected: Option<u64>,
}

impl ViewportHost {
    /// Creates the host and its shared render pipeline from the egui-wgpu state.
    ///
    /// Obtain the state from `eframe::Frame::wgpu_render_state` (or any
    /// [`egui_wgpu::RenderState`]) once at startup.
    ///
    /// # Errors
    /// Reserved for future GPU capability negotiation; the prototype pipeline is
    /// always creatable on a device that already rendered egui.
    pub fn new(render_state: &egui_wgpu::RenderState) -> Result<Self, ViewportError> {
        Ok(Self {
            renderer: ViewportRenderer::new(render_state)?,
            viewports: HashMap::new(),
        })
    }

    /// Returns the egui texture id for `key`, creating or resizing the offscreen
    /// target as needed.
    ///
    /// Call this every frame with the viewport's physical pixel size. It is
    /// idempotent: the GPU target is only reallocated when the size changes.
    /// Zero-sized requests are clamped to `1x1` so collapsed panels stay valid.
    ///
    /// The returned id can change when the target is reallocated (after a
    /// resize), so always draw with the id from the most recent call.
    ///
    /// # Errors
    /// [`ViewportError::SizeOutOfRange`] when the size exceeds the device limits.
    pub fn bind(
        &mut self,
        key: u32,
        width: u32,
        height: u32,
    ) -> Result<egui::TextureId, ViewportError> {
        if let Some(entry) = self.viewports.get_mut(&key) {
            self.renderer
                .ensure_target_size(&mut entry.target, key, width, height)?;
            return Ok(entry.target.egui_texture());
        }
        let target = self.renderer.create_target(key, width, height)?;
        let texture_id = target.egui_texture();
        self.viewports.insert(
            key,
            ViewportEntry {
                target,
                camera: OrbitCamera::default(),
                selected: None,
            },
        );
        Ok(texture_id)
    }

    /// Resizes an existing viewport's offscreen target.
    ///
    /// Prefer [`ViewportHost::bind`], which resizes automatically; this explicit
    /// form exists for hosts that already know the size changed.
    ///
    /// # Errors
    /// [`ViewportError::UnknownViewport`] when `key` was never bound, or
    /// [`ViewportError::SizeOutOfRange`] when the size exceeds device limits.
    pub fn resize(&mut self, key: u32, width: u32, height: u32) -> Result<(), ViewportError> {
        let entry = self
            .viewports
            .get_mut(&key)
            .ok_or(ViewportError::UnknownViewport { key })?;
        self.renderer
            .ensure_target_size(&mut entry.target, key, width, height)
    }

    /// Renders the viewport's current camera into its offscreen target and
    /// submits the command buffer.
    ///
    /// Call from the frame-logic phase (prototype: `eframe::App::logic`; production:
    /// RenderViewports) before egui's backend
    /// paints; this mirrors rendering viewports before `BuildUi` in the real
    /// engine. The call records one render pass and submits it; it does not
    /// block on the GPU.
    ///
    /// # Errors
    /// [`ViewportError::UnknownViewport`] when `key` was never bound.
    pub fn render(&mut self, key: u32) -> Result<(), ViewportError> {
        let entry = self
            .viewports
            .get(&key)
            .ok_or(ViewportError::UnknownViewport { key })?;
        self.renderer
            .render(&entry.target, &entry.camera, entry.selected)
    }

    /// Returns the egui texture id currently registered for `key`, if bound.
    pub fn texture_id(&self, key: u32) -> Option<egui::TextureId> {
        self.viewports
            .get(&key)
            .map(|entry| entry.target.egui_texture())
    }

    /// Destroys the viewport's GPU resources and frees its egui texture.
    ///
    /// Destroying an unbound key is a no-op, so the app may call this whenever a
    /// panel closes without tracking bind state.
    pub fn destroy(&mut self, key: u32) {
        self.viewports.remove(&key);
    }

    /// Applies one frame of orbit/pan/zoom input to the viewport camera.
    ///
    /// Unknown keys are ignored, so input plumbing does not need to know whether
    /// the viewport has been bound yet.
    pub fn apply_camera_input(&mut self, key: u32, input: CameraInput) {
        if let Some(entry) = self.viewports.get_mut(&key) {
            entry.camera.apply(input);
        }
    }

    /// Sets the object highlighted by the viewport, or clears the highlight with
    /// `None`.
    ///
    /// The demo scene highlights the cube (object id [`crate::CUBE_OBJECT_ID`])
    /// with a warm rim light; the id is passed through to the shader unchanged so
    /// a real engine would implement its own selection visualisation.
    pub fn set_selected(&mut self, key: u32, object: Option<u64>) {
        if let Some(entry) = self.viewports.get_mut(&key) {
            entry.selected = object;
        }
    }

    /// Picks the demo object at a normalized device coordinate.
    ///
    /// `ndc` is `[x, y]` in `[-1, 1]` with `y` up. egui's pointer space is y-down,
    /// so the app must flip y before calling this. Returns `None` for a ray that
    /// misses the scene or when `key` is not bound.
    pub fn pick(&self, key: u32, ndc: [f32; 2]) -> Option<u64> {
        let entry = self.viewports.get(&key)?;
        let [width, height] = entry.target.size();
        let aspect = width as f32 / height as f32;
        pick_object(&entry.camera, aspect, ndc)
    }

    /// Returns the viewport's orbit camera, if bound.
    pub fn camera(&self, key: u32) -> Option<&OrbitCamera> {
        self.viewports.get(&key).map(|entry| &entry.camera)
    }

    /// Returns the physical pixel size of the viewport's offscreen target.
    pub fn viewport_size(&self, key: u32) -> Option<[u32; 2]> {
        self.viewports.get(&key).map(|entry| entry.target.size())
    }
}
