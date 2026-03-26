use kairos_engine::math::Color32;


#[derive(Clone, Copy)]
pub enum StyleField {
    FloatStyleField(FloatStyleField),
    ColorStyleField(ColorStyleField),
}

#[derive(Clone)]
pub struct StylePage {
    pub id: usize,
    pub name: &'static str,
    pub fields: Vec<StyleField>
}
impl StylePage {
    pub fn new(id: usize, name: &'static str, fields: Vec<StyleField>) -> Self {
        Self {
            id,
            name,
            fields
        }
    }
}


#[derive(Clone, Copy)]
pub enum FloatFieldEditViewType {
    Field,
    Slider
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
    pub fn new(name: &'static str, value: f32, min: f32, max: f32, view_type: FloatFieldEditViewType) -> Self {
        Self { 
            name,
            value, 
            min,
            max,
            view_type
        }
    }
}


#[derive(Clone, Copy)]
pub struct ColorStyleField {
    pub name: &'static str,
    pub color: Color32
}
impl ColorStyleField {
    pub fn new(name: &'static str, color: Color32) -> Self {
        Self {
            name,
            color
        }
    }
}