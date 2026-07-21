use serde::{Deserialize, Serialize};

use crate::graphics::compare_function::CompareFunction;

// ============================================================
// Sampler enums — project-level, maps to wgpu 1:1
// ============================================================

/// Magnification + minification filter, combined into a single setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum FilterMode {
    Nearest,
    Linear,
}

impl FilterMode {
    pub fn label(&self) -> &'static str {
        match self {
            FilterMode::Nearest => "Nearest",
            FilterMode::Linear => "Linear",
        }
    }
}

/// Texture coordinate wrapping mode per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum AddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
    ClampToBorder,
}

impl AddressMode {
    /// Human-readable label for the Inspector.
    pub fn label(&self) -> &'static str {
        match self {
            AddressMode::ClampToEdge => "ClampToEdge",
            AddressMode::Repeat => "Repeat",
            AddressMode::MirrorRepeat => "MirrorRepeat",
            AddressMode::ClampToBorder => "ClampToBorder",
        }
    }
}

/// Mipmap level blending filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum MipmapFilter {
    Nearest,
    Linear,
}

impl MipmapFilter {
    pub fn label(&self) -> &'static str {
        match self {
            MipmapFilter::Nearest => "Nearest",
            MipmapFilter::Linear => "Linear",
        }
    }
}

// ============================================================
// Sampler config structures
// ============================================================

/// Project-level sampler border color — maps 1:1 to wgpu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum BorderColor {
    TransparentBlack,
    OpaqueBlack,
    OpaqueWhite,
    Zero,
}

impl BorderColor {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TransparentBlack => "TransparentBlack",
            Self::OpaqueBlack => "OpaqueBlack",
            Self::OpaqueWhite => "OpaqueWhite",
            Self::Zero => "Zero",
        }
    }
}

impl From<BorderColor> for wgpu::SamplerBorderColor {
    fn from(value: BorderColor) -> Self {
        match value {
            BorderColor::TransparentBlack => wgpu::SamplerBorderColor::TransparentBlack,
            BorderColor::OpaqueBlack => wgpu::SamplerBorderColor::OpaqueBlack,
            BorderColor::OpaqueWhite => wgpu::SamplerBorderColor::OpaqueWhite,
            BorderColor::Zero => wgpu::SamplerBorderColor::Zero,
        }
    }
}

/// Anisotropic filtering levels (power of two, 1 = off handled by UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumIter)]
pub enum AnisotropyLevel {
    Level2 = 2,
    Level4 = 4,
    Level8 = 8,
    Level16 = 16,
}

impl AnisotropyLevel {
    pub fn as_u16(self) -> u16 {
        self as u16
    }

    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            2 => Some(Self::Level2),
            4 => Some(Self::Level4),
            8 => Some(Self::Level8),
            16 => Some(Self::Level16),
            _ => None,
        }
    }
}

/// Mipmap-related sampler settings.
/// Only active when `SamplerConfig::mipmap` is `Some`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MipmapConfig {
    /// Blending filter between mipmap levels.
    pub filter: MipmapFilter,
    /// Anisotropic filtering level. 1 = off, 2–16 = on.
    pub anisotropy_clamp: u16,
    /// Minimum LOD level to sample from.
    pub lod_min_clamp: f32,
    /// Maximum LOD level to sample from.
    pub lod_max_clamp: f32,
}

/// Complete sampler configuration for a texture.
///
/// Stored alongside the texture format in `.texture` TOML files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplerConfig {
    /// Magnification + minification filter.
    pub filter_mode: FilterMode,
    /// U-axis wrap mode.
    pub address_mode_u: AddressMode,
    /// V-axis wrap mode.
    pub address_mode_v: AddressMode,
    /// W-axis wrap mode.
    pub address_mode_w: AddressMode,
    /// Mipmap configuration. `None` = mipmapping disabled.
    pub mipmap: Option<MipmapConfig>,
    /// Comparison function for shadow samplers. `None` = no comparison.
    pub compare: Option<CompareFunction>,
    /// Border color, only relevant when any address mode is `ClampToBorder`.
    pub border_color: Option<BorderColor>,
}

// ============================================================
// Into<wgpu::...> conversions
// ============================================================

impl From<FilterMode> for wgpu::FilterMode {
    fn from(value: FilterMode) -> Self {
        match value {
            FilterMode::Nearest => wgpu::FilterMode::Nearest,
            FilterMode::Linear => wgpu::FilterMode::Linear,
        }
    }
}

impl From<AddressMode> for wgpu::AddressMode {
    fn from(value: AddressMode) -> Self {
        match value {
            AddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            AddressMode::Repeat => wgpu::AddressMode::Repeat,
            AddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
            AddressMode::ClampToBorder => wgpu::AddressMode::ClampToBorder,
        }
    }
}

impl From<MipmapFilter> for wgpu::MipmapFilterMode {
    fn from(value: MipmapFilter) -> Self {
        match value {
            MipmapFilter::Nearest => wgpu::MipmapFilterMode::Nearest,
            MipmapFilter::Linear => wgpu::MipmapFilterMode::Linear,
        }
    }
}

impl SamplerConfig {
    /// Build a `wgpu::SamplerDescriptor` from this config.
    /// The caller should provide a label for the created sampler.
    pub fn to_wgpu_descriptor<'a>(&'a self, label: &'a str) -> wgpu::SamplerDescriptor<'a> {
        let filter: wgpu::FilterMode = self.filter_mode.into();
        let (mipmap_filter, lod_min, lod_max, anisotropy) = match &self.mipmap {
            Some(mip) => (
                mip.filter.into(),
                mip.lod_min_clamp,
                mip.lod_max_clamp,
                mip.anisotropy_clamp,
            ),
            None => (wgpu::MipmapFilterMode::Nearest, 0.0f32, 0.0f32, 1u16),
        };

        wgpu::SamplerDescriptor {
            label: Some(label),
            address_mode_u: self.address_mode_u.into(),
            address_mode_v: self.address_mode_v.into(),
            address_mode_w: self.address_mode_w.into(),
            mag_filter: filter,
            min_filter: filter,
            mipmap_filter,
            lod_min_clamp: lod_min,
            lod_max_clamp: lod_max,
            compare: self.compare.map(Into::into),
            anisotropy_clamp: anisotropy,
            border_color: self.border_color.map(Into::into),
        }
    }
}
