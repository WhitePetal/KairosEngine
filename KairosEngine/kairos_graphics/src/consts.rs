//! Capacity constants for the graphics asset stores.
//!
//! Each constant lives next to the asset type it sizes rather than in
//! `kairos_asset`: the capacity is a per-type preallocation knob, consumed by
//! the type's own `install`.

/// How many shader slots [`Assets<ShaderAsset>`](kairos_asset::Assets)
/// preallocates.
pub const SHADER_ASSETS_CAPACITY: usize = 128;

/// How many texture slots [`Assets<Texture>`](kairos_asset::Assets)
/// preallocates.
pub const TEXTURE_ASSETS_CAPACITY: usize = 512;

/// How many material slots [`Assets<Material>`](kairos_asset::Assets)
/// preallocates.
pub const MATERIAL_ASSETS_CAPACITY: usize = 512;

/// How many mesh slots [`Assets<Mesh>`](kairos_asset::Assets) preallocates.
pub const MESH_ASSETS_CAPACITY: usize = 512;

/// How many serialized-material slots
/// [`Assets<SerializedMaterial>`](kairos_asset::Assets) preallocates.
pub const SERIALIZED_MATERIAL_ASSETS_CAPACITY: usize = 128;
