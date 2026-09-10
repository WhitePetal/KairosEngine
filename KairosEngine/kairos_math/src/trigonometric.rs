pub trait Sin {
    fn sin(self) -> Self;
}
pub trait Cos {
    fn cos(self) -> Self;
}
pub trait Tan {
    fn tan(self) -> Self;
}

#[inline(always)]
pub fn _sin<T: Sin>(value: T) -> T {
    value.sin()
}
#[inline(always)]
pub fn cos<T: Cos>(value: T) -> T {
    value.cos()
}
#[inline(always)]
pub fn tan<T: Tan>(value: T) -> T {
    value.tan()
}

impl Sin for f32 {
    #[inline(always)]
    fn sin(self) -> Self {
        self.sin()
    }
}
impl Cos for f32 {
    #[inline(always)]
    fn cos(self) -> Self {
        self.cos()
    }
}
impl Tan for f32 {
    #[inline(always)]
    fn tan(self) -> Self {
        self.tan()
    }
}
