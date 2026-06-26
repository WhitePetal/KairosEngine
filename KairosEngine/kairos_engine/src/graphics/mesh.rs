use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::graphics::vertex::Vertex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshAsset {
    pub meta: Meta,
    pub mesh: Mesh,
}

impl Mesh {
    pub fn new(vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        Self { vertices, indices }
    }
}
