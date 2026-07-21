use serde::{Deserialize, Serialize};
use wgpu::BlendState;

use crate::graphics::compare_function::CompareFunction;

// ============================================================
// CullMode — project-level back-face culling control
// ============================================================

/// Replaces `Option<Face>`. Serialized as lowercase for backward compatibility
/// with existing `.mat` files that used wgpu's `Face` serde format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CullMode {
    /// No face culling.
    None,
    /// Cull back faces (default).
    #[default]
    Back,
    /// Cull front faces.
    Front,
}

impl CullMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Back => "Back",
            Self::Front => "Front",
        }
    }
}

impl From<CullMode> for Option<wgpu::Face> {
    fn from(value: CullMode) -> Self {
        match value {
            CullMode::None => None,
            CullMode::Back => Some(wgpu::Face::Back),
            CullMode::Front => Some(wgpu::Face::Front),
        }
    }
}

impl From<Option<wgpu::Face>> for CullMode {
    fn from(value: Option<wgpu::Face>) -> Self {
        match value {
            None => CullMode::None,
            Some(wgpu::Face::Back) => CullMode::Back,
            Some(wgpu::Face::Front) => CullMode::Front,
        }
    }
}

// ============================================================
// WrappedPrimitiveTopology — project-level topology
// ============================================================

/// Replaces `PrimitiveTopology`. Serialized as kebab-case for backward
/// compatibility with existing `.mat` files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WrappedPrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    #[default]
    TriangleList,
    TriangleStrip,
}

impl WrappedPrimitiveTopology {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PointList => "Point List",
            Self::LineList => "Line List",
            Self::LineStrip => "Line Strip",
            Self::TriangleList => "Triangle List",
            Self::TriangleStrip => "Triangle Strip",
        }
    }
}

impl From<WrappedPrimitiveTopology> for wgpu::PrimitiveTopology {
    fn from(value: WrappedPrimitiveTopology) -> Self {
        match value {
            WrappedPrimitiveTopology::PointList => wgpu::PrimitiveTopology::PointList,
            WrappedPrimitiveTopology::LineList => wgpu::PrimitiveTopology::LineList,
            WrappedPrimitiveTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            WrappedPrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            WrappedPrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

impl From<wgpu::PrimitiveTopology> for WrappedPrimitiveTopology {
    fn from(value: wgpu::PrimitiveTopology) -> Self {
        match value {
            wgpu::PrimitiveTopology::PointList => WrappedPrimitiveTopology::PointList,
            wgpu::PrimitiveTopology::LineList => WrappedPrimitiveTopology::LineList,
            wgpu::PrimitiveTopology::LineStrip => WrappedPrimitiveTopology::LineStrip,
            wgpu::PrimitiveTopology::TriangleList => WrappedPrimitiveTopology::TriangleList,
            wgpu::PrimitiveTopology::TriangleStrip => WrappedPrimitiveTopology::TriangleStrip,
        }
    }
}

// ============================================================
// BlendPreset — high-level blend mode with Custom escape hatch
// ============================================================

/// Replaces `Option<BlendState>`. The serialized format is identical to
/// wgpu's `BlendState` so that existing `.mat` files remain valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendPreset {
    /// `BlendState::REPLACE` — src * 1 + dst * 0
    Replace,
    /// Additive blending: src * SrcAlpha + dst * 1
    Add,
    /// Multiplicative blending: src * Dst + dst * 0
    Multiply,
    /// Standard alpha blending: `BlendState::ALPHA_BLENDING`
    AlphaBlend,
    /// Fully custom blend state (6 sub-fields: color.src, color.dst, color.op,
    /// alpha.src, alpha.dst, alpha.op).
    Custom(BlendState),
}

impl BlendPreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::AlphaBlend => "Alpha Blend",
            Self::Custom(_) => "Custom",
        }
    }

    /// Convert the preset into its equivalent `wgpu::BlendState`.
    pub(crate) fn to_blend_state(&self) -> BlendState {
        match self {
            Self::Replace => BlendState::REPLACE,
            Self::Add => BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            Self::Multiply => BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            Self::AlphaBlend => BlendState::ALPHA_BLENDING,
            Self::Custom(state) => *state,
        }
    }

    /// Try to match a `BlendState` against a known preset.
    fn from_blend_state(state: BlendState) -> Self {
        if state == BlendState::REPLACE {
            return Self::Replace;
        }
        if state == BlendState::ALPHA_BLENDING {
            return Self::AlphaBlend;
        }
        let additive = BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        if state == additive {
            return Self::Add;
        }
        let multiply = BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        };
        if state == multiply {
            return Self::Multiply;
        }
        Self::Custom(state)
    }
}

impl Default for BlendPreset {
    fn default() -> Self {
        Self::Replace
    }
}

impl From<BlendPreset> for Option<BlendState> {
    fn from(value: BlendPreset) -> Self {
        Some(value.to_blend_state())
    }
}

impl From<Option<BlendState>> for BlendPreset {
    fn from(value: Option<BlendState>) -> Self {
        match value {
            None => BlendPreset::Replace,
            Some(state) => BlendPreset::from_blend_state(state),
        }
    }
}

// Custom serde for BlendPreset — serializes as Option<BlendState> for
// full backward compatibility with existing `.mat` files.
impl Serialize for BlendPreset {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let blend_state = Some(self.to_blend_state());
        blend_state.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BlendPreset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let opt: Option<BlendState> = Option::deserialize(deserializer)?;
        Ok(BlendPreset::from(opt))
    }
}

// ============================================================
// WrappedBlendFactor — project-level blend factor
// ============================================================

/// Follows the `CompareFunction` pattern. Serializes as kebab-case like wgpu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum WrappedBlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
    Src1,
    OneMinusSrc1,
    Src1Alpha,
    OneMinusSrc1Alpha,
}

impl WrappedBlendFactor {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Zero => "Zero",
            Self::One => "One",
            Self::Src => "Src",
            Self::OneMinusSrc => "1 - Src",
            Self::SrcAlpha => "Src Alpha",
            Self::OneMinusSrcAlpha => "1 - Src Alpha",
            Self::Dst => "Dst",
            Self::OneMinusDst => "1 - Dst",
            Self::DstAlpha => "Dst Alpha",
            Self::OneMinusDstAlpha => "1 - Dst Alpha",
            Self::SrcAlphaSaturated => "Src Alpha Saturated",
            Self::Constant => "Constant",
            Self::OneMinusConstant => "1 - Constant",
            Self::Src1 => "Src1",
            Self::OneMinusSrc1 => "1 - Src1",
            Self::Src1Alpha => "Src1 Alpha",
            Self::OneMinusSrc1Alpha => "1 - Src1 Alpha",
        }
    }
}

impl From<WrappedBlendFactor> for wgpu::BlendFactor {
    fn from(value: WrappedBlendFactor) -> Self {
        match value {
            WrappedBlendFactor::Zero => wgpu::BlendFactor::Zero,
            WrappedBlendFactor::One => wgpu::BlendFactor::One,
            WrappedBlendFactor::Src => wgpu::BlendFactor::Src,
            WrappedBlendFactor::OneMinusSrc => wgpu::BlendFactor::OneMinusSrc,
            WrappedBlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            WrappedBlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            WrappedBlendFactor::Dst => wgpu::BlendFactor::Dst,
            WrappedBlendFactor::OneMinusDst => wgpu::BlendFactor::OneMinusDst,
            WrappedBlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            WrappedBlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
            WrappedBlendFactor::SrcAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
            WrappedBlendFactor::Constant => wgpu::BlendFactor::Constant,
            WrappedBlendFactor::OneMinusConstant => wgpu::BlendFactor::OneMinusConstant,
            WrappedBlendFactor::Src1 => wgpu::BlendFactor::Src1,
            WrappedBlendFactor::OneMinusSrc1 => wgpu::BlendFactor::OneMinusSrc1,
            WrappedBlendFactor::Src1Alpha => wgpu::BlendFactor::Src1Alpha,
            WrappedBlendFactor::OneMinusSrc1Alpha => wgpu::BlendFactor::OneMinusSrc1Alpha,
        }
    }
}

impl From<wgpu::BlendFactor> for WrappedBlendFactor {
    fn from(value: wgpu::BlendFactor) -> Self {
        match value {
            wgpu::BlendFactor::Zero => WrappedBlendFactor::Zero,
            wgpu::BlendFactor::One => WrappedBlendFactor::One,
            wgpu::BlendFactor::Src => WrappedBlendFactor::Src,
            wgpu::BlendFactor::OneMinusSrc => WrappedBlendFactor::OneMinusSrc,
            wgpu::BlendFactor::SrcAlpha => WrappedBlendFactor::SrcAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha => WrappedBlendFactor::OneMinusSrcAlpha,
            wgpu::BlendFactor::Dst => WrappedBlendFactor::Dst,
            wgpu::BlendFactor::OneMinusDst => WrappedBlendFactor::OneMinusDst,
            wgpu::BlendFactor::DstAlpha => WrappedBlendFactor::DstAlpha,
            wgpu::BlendFactor::OneMinusDstAlpha => WrappedBlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::SrcAlphaSaturated => WrappedBlendFactor::SrcAlphaSaturated,
            wgpu::BlendFactor::Constant => WrappedBlendFactor::Constant,
            wgpu::BlendFactor::OneMinusConstant => WrappedBlendFactor::OneMinusConstant,
            wgpu::BlendFactor::Src1 => WrappedBlendFactor::Src1,
            wgpu::BlendFactor::OneMinusSrc1 => WrappedBlendFactor::OneMinusSrc1,
            wgpu::BlendFactor::Src1Alpha => WrappedBlendFactor::Src1Alpha,
            wgpu::BlendFactor::OneMinusSrc1Alpha => WrappedBlendFactor::OneMinusSrc1Alpha,
        }
    }
}

// ============================================================
// WrappedBlendOperation — project-level blend operation
// ============================================================

/// Follows the `CompareFunction` pattern. Serializes as kebab-case like wgpu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
#[serde(rename_all = "kebab-case")]
pub enum WrappedBlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl WrappedBlendOperation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::ReverseSubtract => "Reverse Subtract",
            Self::Min => "Min",
            Self::Max => "Max",
        }
    }
}

impl From<WrappedBlendOperation> for wgpu::BlendOperation {
    fn from(value: WrappedBlendOperation) -> Self {
        match value {
            WrappedBlendOperation::Add => wgpu::BlendOperation::Add,
            WrappedBlendOperation::Subtract => wgpu::BlendOperation::Subtract,
            WrappedBlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
            WrappedBlendOperation::Min => wgpu::BlendOperation::Min,
            WrappedBlendOperation::Max => wgpu::BlendOperation::Max,
        }
    }
}

impl From<wgpu::BlendOperation> for WrappedBlendOperation {
    fn from(value: wgpu::BlendOperation) -> Self {
        match value {
            wgpu::BlendOperation::Add => WrappedBlendOperation::Add,
            wgpu::BlendOperation::Subtract => WrappedBlendOperation::Subtract,
            wgpu::BlendOperation::ReverseSubtract => WrappedBlendOperation::ReverseSubtract,
            wgpu::BlendOperation::Min => WrappedBlendOperation::Min,
            wgpu::BlendOperation::Max => WrappedBlendOperation::Max,
        }
    }
}

// ============================================================
// RenderState — updated to use project-level wrapper types
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderState {
    pub depth_test: Option<CompareFunction>,
    pub depth_write: bool,
    #[serde(default)]
    pub cull_mod: CullMode,
    #[serde(default)]
    pub blend_mod: BlendPreset,
    #[serde(default)]
    pub topology: WrappedPrimitiveTopology,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            depth_test: Some(CompareFunction::LessEqual),
            depth_write: true,
            cull_mod: CullMode::Back,
            blend_mod: BlendPreset::Replace,
            topology: WrappedPrimitiveTopology::TriangleList,
        }
    }
}
