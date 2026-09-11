//! Capacity constants for the graphics asset stores.
//!
//! The capacities are real preallocation knobs carried over from the legacy
//! stack: each constant lives next to the asset type it sizes rather than in
//! `kairos_asset`. The legacy per-type asset systems still read them from here
//! until they are deleted with the rest of the old stack.
//!
//! The channel-buffer constants the legacy stack also used are **not** migrated:
//! the next-generation core has no per-type load/drop channels, so those
//! constants die with the old systems.

/// How many shader slots [`Assets<ShaderAsset>`](kairos_asset::next::Assets)
/// preallocates.
pub const SHADER_ASSETS_CAPACITY: usize = 128;

/// How many texture slots [`Assets<Texture>`](kairos_asset::next::Assets)
/// preallocates.
pub const TEXTURE_ASSETS_CAPACITY: usize = 512;

/// How many material slots [`Assets<Material>`](kairos_asset::next::Assets)
/// preallocates.
pub const MATERIAL_ASSETS_CAPACITY: usize = 512;

/// How many mesh slots [`Assets<Mesh>`](kairos_asset::next::Assets) preallocates.
pub const MESH_ASSETS_CAPACITY: usize = 512;

/// How many serialized-material slots
/// [`Assets<SerializedMaterial>`](kairos_asset::next::Assets) preallocates.
pub const SERIALIZED_MATERIAL_ASSETS_CAPACITY: usize = 128;
