//! The per-view frame buffers the extract stage writes and the window play
//! layers read.
//!
//! There is one typed view resource per window (Scene, Game): its physical
//! size, the camera entity it renders from, and the draw list of *this* frame.
//! Draws are deliberately **not** shared between views — once per-camera
//! frustum culling lands the visible sets diverge, so the buffer is shaped per
//! view from the start. `Scene` and `Game` therefore hold equal content today
//! and are still separate containers.
//!
//! Ownership: the UI writes [`size`](SceneView::size) and
//! [`camera`](SceneView::camera); the extract stage resets and rebuilds
//! [`draws`](SceneView::draws) every frame; the window play layer only drains
//! them. The view's view-projection lives on the camera entity's
//! [`CameraView`](crate::camera::CameraView) — the single source of
//! truth — not on the resource.

use kairos_ecs::{entity::Entity, resource::Resource};

use crate::drawer::DrawCommand;

/// The physical size of a view, in pixels.
///
/// A zero on either axis means "nothing to render": the window is closed (or
/// has never been laid out), a zero-sized attachment cannot be built, and
/// `width / height` would divide by zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportSize {
    pub width: u32,
    pub height: u32,
}

impl ViewportSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Whether this view renders anything at all this frame.
    pub const fn is_renderable(&self) -> bool {
        self.width > 0 && self.height > 0
    }

    /// `width / height`, the aspect a projection is built from.
    ///
    /// Only meaningful for a renderable size — check [`is_renderable`] first,
    /// which is also what keeps this away from a division by zero.
    ///
    /// [`is_renderable`]: Self::is_renderable
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }
}

/// What the extract stage needs from a typed view resource.
///
/// The per-view work — reset the buffer, derive the view projection, collect
/// draws — exists once and is instantiated per view through this trait, so the
/// two views cannot drift apart in how they are extracted.
pub trait ViewResource {
    /// Physical size the view renders at this frame.
    fn size(&self) -> ViewportSize;
    /// The camera entity this view renders from, if one is bound.
    fn camera(&self) -> Option<Entity>;
    /// This frame's draw list; the extract stage rebuilds it from scratch.
    fn draws_mut(&mut self) -> &mut Vec<DrawCommand>;
}

/// The Scene window's view: the editor camera's frame buffer.
#[derive(Resource, Default)]
pub struct SceneView {
    /// Written by the UI layer (`Message::UpdateSceneWindowSize` /
    /// `Message::CloseSceneTab`).
    pub size: ViewportSize,
    /// Bound when the editor camera entity is spawned; the extract stage reads
    /// the frame's view projection off it.
    pub camera: Option<Entity>,
    /// Rebuilt every frame by the extract stage.
    pub draws: Vec<DrawCommand>,
}

/// The Game window's view: the game camera's frame buffer.
#[derive(Resource, Default)]
pub struct GameView {
    /// Written by the UI layer (`Message::UpdateGameWindowSize` /
    /// `Message::CloseGameTab`).
    pub size: ViewportSize,
    /// Bound where the game camera is spawned.
    pub camera: Option<Entity>,
    /// Rebuilt every frame by the extract stage.
    pub draws: Vec<DrawCommand>,
}

/// `ViewResource` is three accessors over the same three fields in every view
/// resource, so the implementations are generated rather than copied — the same
/// hand-rolled-macro choice the schedule labels make.
macro_rules! impl_view_resource {
    ($($view:ident),+ $(,)?) => {
        $(
            impl ViewResource for $view {
                fn size(&self) -> ViewportSize {
                    self.size
                }

                fn camera(&self) -> Option<Entity> {
                    self.camera
                }

                fn draws_mut(&mut self) -> &mut Vec<DrawCommand> {
                    &mut self.draws
                }
            }
        )+
    };
}

impl_view_resource!(SceneView, GameView);
