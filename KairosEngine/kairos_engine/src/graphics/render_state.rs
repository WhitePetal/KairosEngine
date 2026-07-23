use serde::{Deserialize, Serialize};

use crate::graphics::compare_function::CompareFunction;

// ============================================================
// CullMode — project-level back-face culling control
// ============================================================

/// Replaces `Option<Face>`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, strum::EnumIter,
)]
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
    #[inline(always)]
    fn from(value: CullMode) -> Self {
        match value {
            CullMode::None => None,
            CullMode::Back => Some(wgpu::Face::Back),
            CullMode::Front => Some(wgpu::Face::Front),
        }
    }
}

impl From<Option<wgpu::Face>> for CullMode {
    #[inline(always)]
    fn from(value: Option<wgpu::Face>) -> Self {
        match value {
            None => CullMode::None,
            Some(wgpu::Face::Back) => CullMode::Back,
            Some(wgpu::Face::Front) => CullMode::Front,
        }
    }
}

// ============================================================
// PrimitiveTopology — project-level topology
// ============================================================

/// Replaces `wgpu::PrimitiveTopology`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, strum::EnumIter,
)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    #[default]
    TriangleList,
    TriangleStrip,
}

impl PrimitiveTopology {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PointList => "PointList",
            Self::LineList => "LineList",
            Self::LineStrip => "LineStrip",
            Self::TriangleList => "TriangleList",
            Self::TriangleStrip => "TriangleStrip",
        }
    }
}

impl From<PrimitiveTopology> for wgpu::PrimitiveTopology {
    #[inline(always)]
    fn from(value: PrimitiveTopology) -> Self {
        match value {
            PrimitiveTopology::PointList => wgpu::PrimitiveTopology::PointList,
            PrimitiveTopology::LineList => wgpu::PrimitiveTopology::LineList,
            PrimitiveTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
            PrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
            PrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
        }
    }
}

impl From<wgpu::PrimitiveTopology> for PrimitiveTopology {
    #[inline(always)]
    fn from(value: wgpu::PrimitiveTopology) -> Self {
        match value {
            wgpu::PrimitiveTopology::PointList => PrimitiveTopology::PointList,
            wgpu::PrimitiveTopology::LineList => PrimitiveTopology::LineList,
            wgpu::PrimitiveTopology::LineStrip => PrimitiveTopology::LineStrip,
            wgpu::PrimitiveTopology::TriangleList => PrimitiveTopology::TriangleList,
            wgpu::PrimitiveTopology::TriangleStrip => PrimitiveTopology::TriangleStrip,
        }
    }
}

// ============================================================
// BlendFactor — project-level blend factor
// ============================================================

/// Follows the `CompareFunction` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum BlendFactor {
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

impl BlendFactor {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Zero => "Zero",
            Self::One => "One",
            Self::Src => "Src",
            Self::OneMinusSrc => "OneMinusSrc",
            Self::SrcAlpha => "SrcAlpha",
            Self::OneMinusSrcAlpha => "OneMinusSrcAlpha",
            Self::Dst => "Dst",
            Self::OneMinusDst => "OneMinusDst",
            Self::DstAlpha => "DstAlpha",
            Self::OneMinusDstAlpha => "OneMinusDstAlpha",
            Self::SrcAlphaSaturated => "SrcAlphaSaturated",
            Self::Constant => "Constant",
            Self::OneMinusConstant => "OneMinusConstant",
            Self::Src1 => "Src1",
            Self::OneMinusSrc1 => "OneMinusSrc1",
            Self::Src1Alpha => "Src1Alpha",
            Self::OneMinusSrc1Alpha => "OneMinusSrc1Alpha",
        }
    }
}

impl From<BlendFactor> for wgpu::BlendFactor {
    #[inline(always)]
    fn from(value: BlendFactor) -> Self {
        match value {
            BlendFactor::Zero => wgpu::BlendFactor::Zero,
            BlendFactor::One => wgpu::BlendFactor::One,
            BlendFactor::Src => wgpu::BlendFactor::Src,
            BlendFactor::OneMinusSrc => wgpu::BlendFactor::OneMinusSrc,
            BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
            BlendFactor::Dst => wgpu::BlendFactor::Dst,
            BlendFactor::OneMinusDst => wgpu::BlendFactor::OneMinusDst,
            BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
            BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
            BlendFactor::SrcAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
            BlendFactor::Constant => wgpu::BlendFactor::Constant,
            BlendFactor::OneMinusConstant => wgpu::BlendFactor::OneMinusConstant,
            BlendFactor::Src1 => wgpu::BlendFactor::Src1,
            BlendFactor::OneMinusSrc1 => wgpu::BlendFactor::OneMinusSrc1,
            BlendFactor::Src1Alpha => wgpu::BlendFactor::Src1Alpha,
            BlendFactor::OneMinusSrc1Alpha => wgpu::BlendFactor::OneMinusSrc1Alpha,
        }
    }
}

impl From<wgpu::BlendFactor> for BlendFactor {
    #[inline(always)]
    fn from(value: wgpu::BlendFactor) -> Self {
        match value {
            wgpu::BlendFactor::Zero => BlendFactor::Zero,
            wgpu::BlendFactor::One => BlendFactor::One,
            wgpu::BlendFactor::Src => BlendFactor::Src,
            wgpu::BlendFactor::OneMinusSrc => BlendFactor::OneMinusSrc,
            wgpu::BlendFactor::SrcAlpha => BlendFactor::SrcAlpha,
            wgpu::BlendFactor::OneMinusSrcAlpha => BlendFactor::OneMinusSrcAlpha,
            wgpu::BlendFactor::Dst => BlendFactor::Dst,
            wgpu::BlendFactor::OneMinusDst => BlendFactor::OneMinusDst,
            wgpu::BlendFactor::DstAlpha => BlendFactor::DstAlpha,
            wgpu::BlendFactor::OneMinusDstAlpha => BlendFactor::OneMinusDstAlpha,
            wgpu::BlendFactor::SrcAlphaSaturated => BlendFactor::SrcAlphaSaturated,
            wgpu::BlendFactor::Constant => BlendFactor::Constant,
            wgpu::BlendFactor::OneMinusConstant => BlendFactor::OneMinusConstant,
            wgpu::BlendFactor::Src1 => BlendFactor::Src1,
            wgpu::BlendFactor::OneMinusSrc1 => BlendFactor::OneMinusSrc1,
            wgpu::BlendFactor::Src1Alpha => BlendFactor::Src1Alpha,
            wgpu::BlendFactor::OneMinusSrc1Alpha => BlendFactor::OneMinusSrc1Alpha,
        }
    }
}

// ============================================================
// BlendOperation — project-level blend operation
// ============================================================

/// Follows the `CompareFunction` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl BlendOperation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::ReverseSubtract => "ReverseSubtract",
            Self::Min => "Min",
            Self::Max => "Max",
        }
    }
}

impl From<BlendOperation> for wgpu::BlendOperation {
    #[inline(always)]
    fn from(value: BlendOperation) -> Self {
        match value {
            BlendOperation::Add => wgpu::BlendOperation::Add,
            BlendOperation::Subtract => wgpu::BlendOperation::Subtract,
            BlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
            BlendOperation::Min => wgpu::BlendOperation::Min,
            BlendOperation::Max => wgpu::BlendOperation::Max,
        }
    }
}

impl From<wgpu::BlendOperation> for BlendOperation {
    #[inline(always)]
    fn from(value: wgpu::BlendOperation) -> Self {
        match value {
            wgpu::BlendOperation::Add => BlendOperation::Add,
            wgpu::BlendOperation::Subtract => BlendOperation::Subtract,
            wgpu::BlendOperation::ReverseSubtract => BlendOperation::ReverseSubtract,
            wgpu::BlendOperation::Min => BlendOperation::Min,
            wgpu::BlendOperation::Max => BlendOperation::Max,
        }
    }
}

// ============================================================
// BlendComponent — project-level blend component
// ============================================================

/// Mirrors `wgpu::BlendComponent` using project-level wrapper types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlendComponent {
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
    pub operation: BlendOperation,
}

impl From<BlendComponent> for wgpu::BlendComponent {
    #[inline(always)]
    fn from(value: BlendComponent) -> Self {
        wgpu::BlendComponent {
            src_factor: value.src_factor.into(),
            dst_factor: value.dst_factor.into(),
            operation: value.operation.into(),
        }
    }
}

impl From<wgpu::BlendComponent> for BlendComponent {
    #[inline(always)]
    fn from(value: wgpu::BlendComponent) -> Self {
        BlendComponent {
            src_factor: value.src_factor.into(),
            dst_factor: value.dst_factor.into(),
            operation: value.operation.into(),
        }
    }
}

// ============================================================
// BlendState — project-level wrapper that mirrors wgpu::BlendState
// ============================================================

/// Mirrors `wgpu::BlendState` using project-level wrapper types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

impl BlendState {
    /// `BlendState::REPLACE` — src * 1 + dst * 0
    pub const REPLACE: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    };

    /// Standard alpha blending: `BlendState::ALPHA_BLENDING`
    pub const ALPHA_BLENDING: Self = Self {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    };
}

impl Default for BlendState {
    fn default() -> Self {
        Self::REPLACE
    }
}

impl From<BlendState> for Option<wgpu::BlendState> {
    #[inline(always)]
    fn from(value: BlendState) -> Self {
        Some(wgpu::BlendState {
            color: value.color.into(),
            alpha: value.alpha.into(),
        })
    }
}

impl From<BlendState> for wgpu::BlendState {
    #[inline(always)]
    fn from(value: BlendState) -> Self {
        wgpu::BlendState {
            color: value.color.into(),
            alpha: value.alpha.into(),
        }
    }
}

impl From<wgpu::BlendState> for BlendState {
    #[inline(always)]
    fn from(value: wgpu::BlendState) -> Self {
        BlendState {
            color: value.color.into(),
            alpha: value.alpha.into(),
        }
    }
}

// ============================================================
// BlendPreset — high-level blend mode helper (for material inspector)
// ============================================================

/// Helper for the material inspector: provides a set of named blend presets
/// (plus a Custom escape hatch) for displaying options and constructing
/// [`BlendState`] values. Not used directly in [`RenderState`]; use
/// [`BlendState`] there.
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
    /// Fully custom blend state
    Custom(BlendState),
}

impl BlendPreset {
    /// Human-readable label for the material inspector.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Replace => "Replace",
            Self::Add => "Add",
            Self::Multiply => "Multiply",
            Self::AlphaBlend => "AlphaBlend",
            Self::Custom(_) => "Custom",
        }
    }

    /// Convert the preset into a [`BlendState`] suitable for [`RenderState`].
    pub fn to_blend_state(&self) -> BlendState {
        match self {
            Self::Replace => BlendState::REPLACE,
            Self::Add => BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            },
            Self::Multiply => BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::Zero,
                    operation: BlendOperation::Add,
                },
            },
            Self::AlphaBlend => BlendState::ALPHA_BLENDING,
            Self::Custom(state) => *state,
        }
    }

    /// Try to match a [`BlendState`] against a known preset.
    /// Returns `Custom` if no preset matches.
    pub fn from_blend_state(state: BlendState) -> Self {
        if state == BlendState::REPLACE {
            return Self::Replace;
        }
        if state == BlendState::ALPHA_BLENDING {
            return Self::AlphaBlend;
        }
        let additive = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        };
        if state == additive {
            return Self::Add;
        }
        let multiply = BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::Dst,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
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

impl From<BlendPreset> for BlendState {
    #[inline(always)]
    fn from(value: BlendPreset) -> Self {
        value.to_blend_state()
    }
}

// ============================================================
// RenderState
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RenderState {
    pub depth_test: Option<CompareFunction>,
    pub depth_write: bool,
    #[serde(default)]
    pub cull_mod: CullMode,
    #[serde(default)]
    pub blend_mod: Option<BlendState>,
    #[serde(default)]
    pub topology: PrimitiveTopology,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            depth_test: Some(CompareFunction::LessEqual),
            depth_write: true,
            cull_mod: CullMode::Back,
            blend_mod: None,
            topology: PrimitiveTopology::TriangleList,
        }
    }
}

impl RenderState {
    /// wgpu 生效的 depth write 标志。
    ///
    /// wgpu 校验规则（`DepthStencilStateError::MissingDepthCompare`）：
    /// 带 depth attachment 的 pipeline 启用 depth write 时 `depth_compare`
    /// 必须为 `Some` —— 即“只写不测”是非法组合。因此 `depth_test = None`
    /// （关闭深度测试）时 depth write 一并视为关闭。
    pub fn depth_write_enable(&self) -> bool {
        self.depth_test.is_some() && self.depth_write
    }
}
