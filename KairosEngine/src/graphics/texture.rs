use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Texture {
    pub data: String, // base64
    pub width: u32,
    pub height: u32
}
