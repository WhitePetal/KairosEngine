//! Host-side screen sampling for the color picker's eyedropper.
//!
//! The widget layer owns the *pick* — which field, the live value, what to
//! restore on cancel — and this module owns the *pointer*: while a pick runs it
//! reads the OS cursor, copies a small block of the desktop around it, and shows
//! the loupe the user samples with.
//!
//! Windows is the first backend: `GetCursorPos` + `BitBlt` for the pixels, a
//! topmost click-through popup for the loupe, and `GetAsyncKeyState` for the
//! confirm/cancel click, so a pick can end anywhere on the desktop — including
//! outside this window, which is what Unity's eyedropper does.
//!
//! A pick is modal: the Windows backend masks the desktop's mouse input with a
//! low-level hook for its duration, so the click that confirms it cannot
//! minimize/close this window or press another application's widgets. The
//! masked press is read from the hook itself, because a discarded event never
//! reaches `GetAsyncKeyState`.
//!
//! Other platforms report [`ScreenPicker::available`] as false, and the widget
//! keeps the "not available" tooltip it has always had.

use kairos_editor_widgets::color_picker::eyedropper::Outcome;

/// One frame of sampling, in the host's terms.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// sRGB bytes under the pointer.
    pub srgb: [u8; 4],
    /// Whether this sample is a live preview or the end of the pick.
    pub outcome: Outcome,
}

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::ScreenPicker;

#[cfg(not(windows))]
mod unsupported;
#[cfg(not(windows))]
pub use unsupported::ScreenPicker;
