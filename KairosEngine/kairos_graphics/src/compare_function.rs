use serde::{Deserialize, Serialize};

/// Project-level comparison function — maps 1:1 to wgpu.
/// Used by samplers (shadow maps), depth/stencil testing, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl CompareFunction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::Less => "Less",
            Self::Equal => "Equal",
            Self::LessEqual => "LessEqual",
            Self::Greater => "Greater",
            Self::NotEqual => "NotEqual",
            Self::GreaterEqual => "GreaterEqual",
            Self::Always => "Always",
        }
    }
}

impl From<CompareFunction> for wgpu::CompareFunction {
    #[inline(always)]
    fn from(value: CompareFunction) -> Self {
        match value {
            CompareFunction::Never => wgpu::CompareFunction::Never,
            CompareFunction::Less => wgpu::CompareFunction::Less,
            CompareFunction::Equal => wgpu::CompareFunction::Equal,
            CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
            CompareFunction::Greater => wgpu::CompareFunction::Greater,
            CompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
            CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
            CompareFunction::Always => wgpu::CompareFunction::Always,
        }
    }
}
