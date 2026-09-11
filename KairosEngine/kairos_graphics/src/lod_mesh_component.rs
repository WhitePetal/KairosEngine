use kairos_asset::next::Handle;
use kairos_ecs::component::Component;

use crate::mesh::Mesh;

#[derive(Component, Debug)]
pub struct LODMesh {
    pub lod0: Handle<Mesh>,
}

impl LODMesh {
    pub fn new(lod0: Handle<Mesh>) -> Self {
        Self { lod0 }
    }
}
