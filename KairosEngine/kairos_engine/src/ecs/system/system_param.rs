use std::{borrow::Cow, fmt::Display};

use thiserror::Error;

use crate::debug::DebugName;

/// An error that occurs when a system parameter is not valid,
/// used by system executors to determine what to do with a system.
///
/// Returned as an error from [`SystemParam::get_param`],
/// and handled using the unified error handling mechanisms defined in [`bevy_ecs::error`].
#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub struct SystemParamValidationError {
    /// Whether the system should be skipped.
    ///
    /// If `false`, the error should be handled.
    /// By default, this will result in a panic. See [`error`](`crate::error`) for more information.
    ///
    /// This is the default behavior, and is suitable for system params that should *always* be valid,
    /// either because sensible fallback behavior exists (like [`Query`]) or because
    /// failures in validation should be considered a bug in the user's logic that must be immediately addressed (like [`Res`]).
    ///
    /// If `true`, the system should be skipped.
    /// This is set by wrapping the system param in [`If`],
    /// and indicates that the system is intended to only operate in certain application states.
    pub skipped: bool,

    /// A message describing the validation error.
    pub message: Cow<'static, str>,

    /// A string identifying the invalid parameter.
    /// This is usually the type name of the parameter.
    pub param: DebugName,

    /// A string identifying the field within a parameter using `#[derive(SystemParam)]`.
    /// This will be an empty string for other parameters.
    ///
    /// This will be printed after `param` in the `Display` impl, and should include a `::` prefix if non-empty.
    pub field: Cow<'static, str>,
}

impl SystemParamValidationError {
    /// Constructs a `SystemParamValidationError` that skips the system.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn skipped<T>(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new::<T>(true, message, Cow::Borrowed(""))
    }

    /// Constructs a `SystemParamValidationError` for an invalid parameter that should be treated as an error.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn invalid<T>(message: impl Into<Cow<'static, str>>) -> Self {
        Self::new::<T>(false, message, Cow::Borrowed(""))
    }

    /// Constructs a `SystemParamValidationError` for an invalid parameter.
    /// The parameter name is initialized to the type name of `T`, so a `SystemParam` should usually pass `Self`.
    pub fn new<T>(
        skipped: bool,
        message: impl Into<Cow<'static, str>>,
        field: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            skipped,
            message: message.into(),
            param: DebugName::type_name::<T>(),
            field: field.into(),
        }
    }

    pub(crate) const EMPTY: Self = Self {
        skipped: false,
        message: Cow::Borrowed(""),
        param: DebugName::borrowed(""),
        field: Cow::Borrowed(""),
    };
}

impl Display for SystemParamValidationError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(
            fmt,
            "Parameter `{}{}` failed validation: {}",
            self.param.shortname(),
            self.field,
            self.message
        )?;
        if !self.skipped {
            write!(
                fmt,
                "\nIf this is an expected state, wrap the parameter in `Option<T>` and handle `None` when it happens, or wrap the parameter in `If<T>` to skip the system when it happens."
            )?;
        }
        Ok(())
    }
}

// TODO!
