pub trait Trigonometric {
    type Output;

    fn cos(self) -> Self::Output;

    fn tan(self) -> Self::Output;
}

#[inline(always)]
pub fn cos<T>(value: T) -> T::Output
where
    T: Trigonometric,
{
    value.cos()
}

#[inline(always)]
pub fn tan<T>(value: T) -> T::Output
where
    T: Trigonometric,
{
    value.tan()
}

impl Trigonometric for f32 {
    type Output = f32;

    fn cos(self) -> Self::Output {
        f32::cos(self)
    }

    fn tan(self) -> Self::Output {
        f32::tan(self)
    }
}
