//! Named error type returned by the viewport host.
//!
//! The host API never panics on recoverable problems (an unknown viewport key,
//! an impossible texture size); it returns this error instead so the app can log
//! and keep running. wgpu device-loss and validation failures surface through
//! wgpu's own error handling and are out of scope for the prototype.

use std::fmt;

/// Errors returned by [`crate::ViewportHost`] operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewportError {
    /// The caller referenced a viewport key that has not been bound with
    /// [`crate::ViewportHost::bind`].
    UnknownViewport {
        /// Application-defined viewport key.
        key: u32,
    },
    /// A viewport size exceeds the GPU's maximum 2D texture dimension.
    ///
    /// Zero-sized requests are *not* an error: they are clamped to `1x1` so a
    /// collapsed panel does not spam the host with failures.
    SizeOutOfRange {
        /// Application-defined viewport key.
        key: u32,
        /// Requested width in physical pixels, after clamping zero to one.
        width: u32,
        /// Requested height in physical pixels, after clamping zero to one.
        height: u32,
        /// The device's maximum 2D texture dimension.
        max: u32,
    },
}

impl fmt::Display for ViewportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownViewport { key } => {
                write!(formatter, "no viewport is bound for key {key}")
            }
            Self::SizeOutOfRange {
                key,
                width,
                height,
                max,
            } => write!(
                formatter,
                "viewport {key} requested a {width}x{height} target, \
                 but this device supports at most {max}x{max}"
            ),
        }
    }
}

impl std::error::Error for ViewportError {}
