use std::ops::{Deref, Mul, Neg};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Vector, float3};

///
/// direction3
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize)]
pub struct Dir3(pub(crate) float3);

impl Dir3 {
    pub const RIGHT: Self = Self(float3::RIGHT);
    pub const LEFT: Self = Self(float3::LEFT);
    pub const UP: Self = Self(float3::UP);
    pub const DOWN: Self = Self(float3::DOWN);
    pub const FORWARD: Self = Self(float3::FORWARD);
    pub const BACK: Self = Self(float3::BACK);

    /// Create a direction from a finite, nonzero [`float3`], normalizing it.
    ///
    /// Returns [`Err(InvalidDirectionError)`](InvalidDirectionError) if the length
    /// of the given vector is zero (or very close to zero), infinite, or `NaN`.
    pub fn new(value: float3) -> Result<Self, InvalidDirectionError> {
        Self::new_and_length(value).map(|(dir, _)| dir)
    }

    /// Create a [`Dir3`] from a [`float3`] that is already normalized.
    ///
    /// # Warning
    ///
    /// `value` must be normalized, i.e its length must be `1.0`.
    pub fn new_unchecked(value: float3) -> Self {
        #[cfg(debug_assertions)]
        assert_is_normalized(
            "The vector given to `Dir3::new_unchecked` is not normalized.",
            value.len_sq(),
        );
        Self(value)
    }

    /// Create a direction from a finite, nonzero [`float3`], normalizing it and
    /// also returning its original length.
    ///
    /// Returns [`Err(InvalidDirectionError)`](InvalidDirectionError) if the length
    /// of the given vector is zero (or very close to zero), infinite, or `NaN`.
    pub fn new_and_length(value: float3) -> Result<(Self, f32), InvalidDirectionError> {
        let len = value.len();
        let dir = (len.is_finite() && len > 0.0).then_some(value / len);

        dir
            .map(|dir| (Self(dir), len))
            .ok_or(InvalidDirectionError::from_len(len))
    }

    /// Create a direction from its `x`, `y`, and `z` components.
    ///
    /// Returns [`Err(InvalidDirectionError)`](InvalidDirectionError) if the length
    /// of the vector formed by the components is zero (or very close to zero), infinite, or `NaN`.
    pub fn from_xyz(x: f32, y: f32, z: f32) -> Result<Self, InvalidDirectionError> {
        Self::new(float3::new(x, y, z))
    }

    /// Create a direction from its `x`, `y`, and `z` components, assuming the resulting vector is normalized.
    ///
    /// # Warning
    ///
    /// The vector produced from `x`, `y`, and `z` must be normalized, i.e its length must be `1.0`.
    pub fn from_xyz_unchecked(x: f32, y: f32, z: f32) -> Self {
        Self::new_unchecked(float3::new(x, y, z))
    }

    /// Returns the inner [`float3`]
    pub const fn as_float3(&self) -> float3 {
        self.0
    }
}

impl TryFrom<float3> for Dir3 {
    type Error = InvalidDirectionError;

    fn try_from(value: float3) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Deref for Dir3 {
    type Target = float3;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Neg for Dir3 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Mul<f32> for Dir3 {
    type Output = float3;

    fn mul(self, rhs: f32) -> Self::Output {
        self.0 * rhs
    }
}

/// An error indicating that a direction is invalid.
#[derive(Debug, PartialEq, Error)]
pub enum InvalidDirectionError {
    /// The length of the direction vector is zero or very close to zero.
    #[error("The length of the direction vector is zero or very close to zero")]
    Zero,
    /// The length of the direction vector is `std::f32::INFINITY`.
    #[error("The length of the direction vector is 'std::f32::INFINITY`")]
    Infinite,
    /// The length of the direction vector is `NaN`.
    #[error("The length of the diection vector is `NaN`")]
    NaN,
}

impl InvalidDirectionError {
    /// Creates an [`InvalidDirectionError`] from the length of an invalid direction vector.
    pub fn from_len(len: f32) -> Self {
        if len.is_nan() {
            InvalidDirectionError::NaN
        } else if !len.is_finite() {
            InvalidDirectionError::Infinite
        } else {
            InvalidDirectionError::Zero
        }
    }
}

/// Checks that a vector with the given squared length is normalized.
///
/// Warns for small error with a length threshold of approximately `1e-4`,
/// and panics for large error with a length threshold of approximately `1e-2`.
///
/// The format used for the logged warning is `"Warning: {warning} The length is {length}`,
/// and similarly for the error.
#[cfg(debug_assertions)]
pub(crate) fn assert_is_normalized(message: &str, length_squared: f32) {
    use crate::abs;

    let length_error_squared = abs(length_squared - 1.0);

    // Panic for large error and warn for slight error.
    if length_error_squared > 2e-2 || length_error_squared.is_nan() {
        // Length error is approximately 1e-2 or more.

        use crate::sqrt;
        panic!(
            "Error: {message} The length is {}.",
            sqrt(length_squared)
        );
    } else if length_error_squared > 2e-4 {
        // Length error is approximately 1e-4 or more.
        #[expect(clippy::print_stderr, reason = "Allowed behind `std` feature gate.")]
        {
            use crate::sqrt;

            eprintln!(
                "Warning: {message} The length is {}.",
                sqrt(length_squared)
            );
        }
    }
}
