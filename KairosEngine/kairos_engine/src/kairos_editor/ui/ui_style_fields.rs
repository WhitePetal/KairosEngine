use crate::math::{Color32, float2, float3, float4};

#[derive(Clone, Copy)]
pub enum StyleField {
    FloatStyleField(FloatStyleField),
    ColorStyleField(ColorStyleField),
    Vector2StyleField(Vector2StyleField),
    Vector3StyleField(Vector3StyleField),
    Vector4StyleField(Vector4StyleField),
    RangeStyleField(RangeStyleField),
}

#[derive(Clone)]
pub struct StylePage {
    pub id: usize,
    pub name: &'static str,
    pub fields: Vec<StyleField>,
}
impl StylePage {
    pub fn new(id: usize, name: &'static str, fields: Vec<StyleField>) -> Self {
        Self { id, name, fields }
    }
}

#[derive(Clone, Copy)]
pub enum FloatFieldEditViewType {
    Field,
    Slider,
}

#[derive(Clone, Copy)]
pub struct FloatStyleField {
    pub name: &'static str,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub view_type: FloatFieldEditViewType,
}
impl FloatStyleField {
    pub fn new(
        name: &'static str,
        value: f32,
        min: f32,
        max: f32,
        view_type: FloatFieldEditViewType,
    ) -> Self {
        Self {
            name,
            value,
            min,
            max,
            view_type,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ColorStyleField {
    pub name: &'static str,
    pub color: Color32,
}
impl ColorStyleField {
    pub fn new(name: &'static str, color: Color32) -> Self {
        Self { name, color }
    }
}

#[derive(Clone, Copy)]
pub struct Vector2StyleField {
    pub name: &'static str,
    pub value: float2,
    pub min: f32,
    pub max: f32,
}
impl Vector2StyleField {
    pub fn new(name: &'static str, value: float2, min: f32, max: f32) -> Self {
        Self {
            name,
            value,
            min,
            max,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Vector3StyleField {
    pub name: &'static str,
    pub value: float3,
    pub min: f32,
    pub max: f32,
}
impl Vector3StyleField {
    pub fn new(name: &'static str, value: float3, min: f32, max: f32) -> Self {
        Self {
            name,
            value,
            min,
            max,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Vector4StyleField {
    pub name: &'static str,
    pub value: float4,
    pub min: f32,
    pub max: f32,
}
impl Vector4StyleField {
    pub fn new(name: &'static str, value: float4, min: f32, max: f32) -> Self {
        Self {
            name,
            value,
            min,
            max,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RangeStyleField {
    pub name: &'static str,
    pub range: float2,
    pub min: f32,
    pub max: f32,
}
impl RangeStyleField {
    pub fn new(name: &'static str, range: float2, min: f32, max: f32) -> Self {
        Self {
            name,
            range,
            min,
            max,
        }
    }
}
