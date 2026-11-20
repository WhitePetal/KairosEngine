//! Host-side screen sampling for the color picker's eyedropper.
//!
//! The widget layer owns the *pick* — which field, the live value, what to
//! restore on cancel — and this module owns the *pointer*: while a pick runs it
//! reads the OS pointer, copies a small block of the desktop around it, and
//! shows the loupe the user samples with (the native Windows/X11 loupe, or the
//! AppKit one on macOS).
//!
//! Each platform uses the most native surface available:
//!
//! * **Windows** (win32.rs): GetCursorPos + BitBlt for the pixels, a topmost
//!   click-through popup for the loupe, a low-level mouse hook to mask the
//!   desktop's buttons, and GetAsyncKeyState for the confirm/cancel click.
//! * **macOS** (macos.rs): a ScreenCaptureKit frame plus an AppKit loupe,
//!   matching Windows' live sampling. Screen Recording permission is requested
//!   by the first capture; when it is missing the backend falls back to
//!   AppKit's NSColorSampler, which needs no permission but reports only at
//!   the end of the pick.
//! * **Linux (X11)** (linux.rs): QueryPointer + GetImage for the pixels, an
//!   override-redirect window for the loupe, and a pointer grab that keeps the
//!   confirming click away from other windows. Wayland is unavailable: a
//!   client cannot read the desktop without the ScreenCast portal.
//!
//! Other platforms report [ScreenPicker::available_for] as false, and the
//! widget keeps the "not available" tooltip it has always had.

use kairos_editor_widgets::color_picker::eyedropper::Outcome;

/// One frame of sampling, in the host's terms.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    /// sRGB bytes under the pointer, when the backend has one this frame.
    ///
    /// Windows, X11 and macOS's ScreenCaptureKit path return a live sample.
    /// The macOS fallback hands the whole pick to the system sampler, which
    /// reports its result only at the end, so those picking frames carry no
    /// colour.
    pub srgb: Option<[u8; 4]>,
    /// Whether this sample is a live preview or the end of the pick.
    pub outcome: Outcome,
}

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::ScreenPicker;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::ScreenPicker;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::ScreenPicker;

/// The loupe's geometry and font; used by the X11 backend and, for layout,
/// by the macOS overlay. Testable on any host because it has no platform
/// dependency.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod loupe;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod unsupported;
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub use unsupported::ScreenPicker;
