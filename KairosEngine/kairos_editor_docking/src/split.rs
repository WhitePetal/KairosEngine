#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Split {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy)]
pub struct Fraction {
    fraction: f32
}

impl Fraction {
    pub fn new(value: f32) -> Self {
        Self { fraction: value.clamp(0.0, 1.0) }
    }

    pub fn value(self) -> f32 {
        self.fraction
    }
}

impl From<f32> for Fraction {
    fn from(value: f32) -> Self {
        Self { fraction: value.clamp(0.0, 1.0) }
    }
}

impl From<Fraction> for f32 {
    fn from(value: Fraction) -> Self {
        value.fraction
    }
}
