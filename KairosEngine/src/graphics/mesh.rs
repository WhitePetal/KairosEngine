use crate::graphics::vertex::Vertex;

// TODO: to asset

#[derive(Debug, Clone)]
pub struct Mesh {
    pub id: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Mesh {
    pub fn new(id: usize, vertices: Vec<Vertex>, indices: Vec<u16>) -> Self {
        Self { id, vertices, indices }
    }
}
