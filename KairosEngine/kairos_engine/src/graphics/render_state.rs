use serde::{Deserialize, Serialize};
use wgpu::{BlendState, CompareFunction, Face, PrimitiveTopology};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderState {
    pub depth_test: Option<CompareFunction>,
    pub depth_write: bool,
    pub cull_mod: Option<Face>,
    pub blend_mod: Option<BlendState>,
    pub topology: PrimitiveTopology,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            depth_test: Some(CompareFunction::LessEqual),
            depth_write: true,
            cull_mod: Some(Face::Back),
            blend_mod: Some(BlendState::REPLACE),
            topology: PrimitiveTopology::TriangleList,
        }
    }
}
