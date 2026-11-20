//! macOS screen sampling: ScreenCaptureKit with the system sampler as fallback.
//!
//! The primary path is the same shape as Windows: read the pixels around the
//! pointer every frame, draw a loupe, and let the editor's field follow the
//! colour live. ScreenCaptureKit supplies the pixels; the overlay in
//! overlay.rs supplies the loupe and the input. The first pick asks for Screen
//! Recording with CGRequestScreenCaptureAccess, so the user sees the OS prompt
//! directly, with no extra confirmation window from the editor.
//!
//! If ScreenCaptureKit is unavailable (macOS before 14, permission denied, or
//! a capture that fails), the pick falls back to NSColorSampler, AppKit's
//! permission-free system eyedropper. That fallback has no live colour and no
//! right-click cancel, but it keeps the feature usable.

mod capture;
mod overlay;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use block2::RcBlock;
use kairos_editor_widgets::color_picker::eyedropper::Outcome;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace, NSEvent};
use objc2_core_foundation::CGFloat;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use self::capture::{CaptureSession, ContentSlot};
use self::overlay::Overlay;
use crate::eyedropper::Sample;

/// A ScreenCaptureKit call that never calls back must not leave the pick
/// waiting forever.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(1);

/// How close a completed capture must be to a confirming click to count as
/// that click's colour.
const COMMIT_EPSILON: f64 = 0.25;

enum Mode {
    Idle,
    Requesting(ContentSlot),
    Capture(Box<CaptureSession>),
    System,
    Done,
}

/// The host-side picker: one of the two sampling paths and its overlay.
pub struct ScreenPicker {
    /// The system sampler, used as the fallback and on older macOS.
    sampler: Option<Retained<NSColorSampler>>,
    /// Shared with the fallback's selection block.
    system_session: Option<Arc<SystemSession>>,
    /// The AppKit loupe (ScreenCaptureKit path only).
    overlay: Option<Overlay>,
    mode: Mode,
    running: bool,
    /// Whether this pick masked the pointer; kept after stop so the host's
    /// release guard is armed correctly for the click that just ended.
    masks: bool,
    /// Whether this process has already asked for Screen Recording.
    requested_permission: bool,
}

/// The one value AppKit gives the fallback, written from the selection block.
#[derive(Default)]
struct SystemSession {
    /// None: still sampling. Some(None): the user cancelled. Some(Some): the
    /// picked sRGB bytes.
    result: Mutex<Option<Option<[u8; 4]>>>,
}

impl ScreenPicker {
    pub const fn new() -> Self {
        Self {
            sampler: None,
            system_session: None,
            overlay: None,
            mode: Mode::Idle,
            running: false,
            masks: false,
            requested_permission: false,
        }
    }

    /// Whether this window can start either a ScreenCaptureKit pick or the
    /// system sampler.
    pub fn available_for(window: &Window) -> bool {
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        if !matches!(handle.as_raw(), RawWindowHandle::AppKit(_)) {
            return false;
        }
        capture::available() || AnyClass::get(c"NSColorSampler").is_some()
    }

    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// ScreenCaptureKit's confirming click is consumed by our overlay, so its
    /// release must be kept away from egui. The system sampler owns the click
    /// end to end and needs no guard.
    pub const fn masks_pointer(&self) -> bool {
        self.masks
    }

    /// Whether any physical mouse button is down. The host uses this to keep
    /// its release guard armed until the click has really finished.
    pub fn any_button_down(&self) -> bool {
        NSEvent::pressedMouseButtons() != 0
    }

    pub fn start(&mut self, window: &Window) -> bool {
        if self.running {
            return true;
        }
        if !Self::available_for(window) {
            return false;
        }
        if MainThreadMarker::new().is_none() {
            kairos_log::warn!(
                target: "kairos_editor",
                "eyedropper: the macOS picker must start on the main thread"
            );
            return false;
        }

        if capture::available() {
            // Ask for Screen Recording up front when the process does not have
            // it yet. ScreenCaptureKit itself prompts on the content request,
            // but making the request explicit guarantees the system alert even
            // if the framework call is slow to start.
            if !capture::permission_granted() {
                capture::request_permission();
                self.requested_permission = true;
            }
            let content = capture::request_content();
            self.mode = Mode::Requesting(content);
            self.running = true;
            self.masks = true;
            return true;
        }
        self.start_system()
    }

    /// AppKit's permission-free sampler: the fallback when ScreenCaptureKit
    /// cannot be used. It reports only when the pick ends.
    fn start_system(&mut self) -> bool {
        if MainThreadMarker::new().is_none() {
            return false;
        }
        if AnyClass::get(c"NSColorSampler").is_none() {
            return false;
        }

        let session = Arc::new(SystemSession::default());
        let state = Arc::clone(&session);
        // SAFETY: AppKit calls the block once on the main thread with either a
        // valid NSColor or nil; the block copies the bytes before returning.
        let handler = RcBlock::new(move |color: *mut NSColor| {
            let picked = unsafe { srgb(color) };
            if let Ok(mut slot) = state.result.lock() {
                *slot = Some(picked);
            }
        });
        let sampler = NSColorSampler::new();
        // Bring the editor forward before the system loupe appears; after a
        // trip through System Settings the app is usually inactive.
        if let Some(mtm) = MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
        // SAFETY: both the sampler and the block stay alive for the call, and
        // the class was checked above.
        unsafe { sampler.showSamplerWithSelectionHandler(&handler) };

        self.sampler = Some(sampler);
        self.system_session = Some(session);
        self.mode = Mode::System;
        self.running = true;
        self.masks = false;
        true
    }

    /// One frame. The mode is taken out so its session can be borrowed and put
    /// back without borrowing all of self.
    pub fn poll(&mut self) -> Option<Sample> {
        if !self.running {
            return None;
        }
        match std::mem::replace(&mut self.mode, Mode::Done) {
            Mode::Requesting(slot) => self.poll_requesting(slot),
            Mode::Capture(session) => self.poll_capture(session),
            Mode::System => self.poll_system(),
            Mode::Idle | Mode::Done => None,
        }
    }

    fn poll_requesting(&mut self, slot: ContentSlot) -> Option<Sample> {
        let content = match slot.lock() {
            Ok(mut slot) => slot.take(),
            Err(_) => Some(Err(
                "the ScreenCaptureKit content lock was poisoned".to_owned()
            )),
        };
        let Some(content) = content else {
            // Still waiting for the system prompt / content. Keep the session
            // alive and report a colourless picking frame.
            self.mode = Mode::Requesting(slot);
            return Some(Sample {
                srgb: None,
                outcome: Outcome::Picking,
            });
        };

        let content = match content {
            Ok(content) => content,
            Err(message) => {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "eyedropper: ScreenCaptureKit unavailable: {message}"
                );
                if !capture::permission_granted() && !self.requested_permission {
                    // A fallback request for a content call that failed before
                    // TCC was asked (for example, a transient SCK failure).
                    capture::request_permission();
                    self.requested_permission = true;
                }
                if !self.fallback_system(&message) {
                    self.running = false;
                    self.mode = Mode::Done;
                    return Some(Sample {
                        srgb: None,
                        outcome: Outcome::Cancelled,
                    });
                }
                return self.poll_system();
            }
        };

        let Some(mtm) = MainThreadMarker::new() else {
            self.fallback_system("the picker is not on the main thread");
            self.running = false;
            self.mode = Mode::Done;
            return Some(Sample {
                srgb: None,
                outcome: Outcome::Cancelled,
            });
        };
        let Some(overlay) = Overlay::new(mtm) else {
            if !self.fallback_system("the overlay window could not be created") {
                self.running = false;
                self.mode = Mode::Done;
                return Some(Sample {
                    srgb: None,
                    outcome: Outcome::Cancelled,
                });
            }
            return self.poll_system();
        };
        let state = overlay.state();
        let Some(session) = CaptureSession::new(&content, state) else {
            // Close the loupe before starting the system sampler; otherwise the
            // key overlay would sit in front of the sampler.
            drop(overlay);
            if !self.fallback_system("ScreenCaptureKit exposes no capturable display") {
                self.running = false;
                self.mode = Mode::Done;
                return Some(Sample {
                    srgb: None,
                    outcome: Outcome::Cancelled,
                });
            }
            return self.poll_system();
        };

        let cursor = overlay.cursor();
        session.capture(cursor);
        self.overlay = Some(overlay);
        self.mode = Mode::Capture(Box::new(session));
        Some(Sample {
            srgb: None,
            outcome: Outcome::Picking,
        })
    }

    fn poll_capture(&mut self, session: Box<CaptureSession>) -> Option<Sample> {
        let Some(overlay) = self.overlay.as_ref() else {
            self.mode = Mode::Done;
            return None;
        };
        let state = overlay.state();
        let mut changed = false;
        let mut action = None;
        let mut terminal = None;
        let (has_color, color, error) = {
            let Ok(mut state) = state.lock() else {
                self.mode = Mode::Done;
                return None;
            };
            if let Some(result) = state.pending.take() {
                state.block = result.block;
                state.color = result.color;
                state.has_color = true;
                state.last_capture = Some(result.point);
                state.in_flight = false;
                state.in_flight_since = None;
                state.error = None;
                changed = true;
            }
            if state.in_flight
                && state
                    .in_flight_since
                    .is_some_and(|started| started.elapsed() > CAPTURE_TIMEOUT)
            {
                state.in_flight = false;
                state.in_flight_since = None;
                state.error = Some("the ScreenCaptureKit capture timed out".to_owned());
                changed = true;
            }

            let cursor = state.cursor;
            let commit = state.commit;
            let has_color = state.has_color;
            let color = state.color;
            let last_capture = state.last_capture;
            let in_flight = state.in_flight;
            let error = state.error.clone();
            let failed = error.is_some() && !in_flight;

            match state.outcome {
                Some(Outcome::Cancelled) => {
                    terminal = Some(Sample {
                        srgb: if has_color { Some(color) } else { None },
                        outcome: Outcome::Cancelled,
                    });
                }
                Some(Outcome::Committed) => {
                    let point = commit.unwrap_or(cursor);
                    if has_color && (near(last_capture, point) || failed) {
                        terminal = Some(Sample {
                            srgb: Some(color),
                            outcome: Outcome::Committed,
                        });
                    } else if !in_flight {
                        action = Some(point);
                    }
                }
                Some(Outcome::Picking) | None => {
                    if !in_flight && !near(last_capture, cursor) {
                        action = Some(cursor);
                    }
                }
            }
            (has_color, color, error)
        };

        if changed {
            overlay.set_needs_display();
        }
        if let Some(sample) = terminal {
            self.mode = Mode::Done;
            return Some(sample);
        }
        if let Some(message) = error {
            if !has_color {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "eyedropper: ScreenCaptureKit capture failed: {message}"
                );
                if self.fallback_system(&message) {
                    return self.poll_system();
                }
                self.running = false;
                self.mode = Mode::Done;
                return Some(Sample {
                    srgb: None,
                    outcome: Outcome::Cancelled,
                });
            }
        }

        if let Some(point) = action {
            session.capture(point);
        }
        self.mode = Mode::Capture(session);
        Some(Sample {
            srgb: if has_color { Some(color) } else { None },
            outcome: Outcome::Picking,
        })
    }

    fn poll_system(&mut self) -> Option<Sample> {
        let session = self.system_session.as_ref()?;
        let result = session.result.lock().ok()?.take();
        let sample = match result {
            None => Sample {
                srgb: None,
                outcome: Outcome::Picking,
            },
            Some(Some(srgb)) => Sample {
                srgb: Some(srgb),
                outcome: Outcome::Committed,
            },
            Some(None) => Sample {
                srgb: None,
                outcome: Outcome::Cancelled,
            },
        };
        if sample.outcome == Outcome::Picking {
            // poll() took the mode out; put the live session back.
            self.mode = Mode::System;
        } else {
            self.mode = Mode::Done;
        }
        Some(sample)
    }

    /// Drop the ScreenCaptureKit overlay and start the system sampler. Returns
    /// whether the fallback started.
    fn fallback_system(&mut self, reason: &str) -> bool {
        kairos_log::warn!(
            target: "kairos_editor",
            "eyedropper: falling back to the system sampler: {reason}"
        );
        self.overlay = None;
        if self.start_system() {
            true
        } else {
            self.mode = Mode::Done;
            false
        }
    }

    pub fn stop(&mut self, window: Option<&Window>) {
        self.running = false;
        self.mode = Mode::Idle;
        // Dropping the overlay restores the cursor and closes the window. The
        // sampler's block may still be alive; it writes into an orphaned Arc.
        self.overlay = None;
        self.sampler = None;
        self.system_session = None;
        if let Some(window) = window {
            window.set_cursor(winit::window::CursorIcon::Default);
        }
    }
}

impl Drop for ScreenPicker {
    fn drop(&mut self) {
        self.stop(None);
    }
}

fn near(actual: Option<(f64, f64)>, expected: (f64, f64)) -> bool {
    actual.is_some_and(|actual| {
        (actual.0 - expected.0).abs() < COMMIT_EPSILON
            && (actual.1 - expected.1).abs() < COMMIT_EPSILON
    })
}

/// The selected NSColor as sRGB bytes; None when the user cancelled.
///
/// The system sampler returns a colour in the display's colour space. Asking
/// for sRGB makes the bytes match what the rest of the editor means by "screen
/// colour", even on a P3 display.
///
/// SAFETY: "color" must be null or a valid NSColor for the duration of the
/// call. It is only read here, before the selection block returns.
unsafe fn srgb(color: *mut NSColor) -> Option<[u8; 4]> {
    let color = unsafe { color.as_ref() }?;
    let space = NSColorSpace::sRGBColorSpace();
    let color = color.colorUsingColorSpace(&space)?;
    Some([
        channel(color.redComponent()),
        channel(color.greenComponent()),
        channel(color.blueComponent()),
        255,
    ])
}

/// One [0, 1] component as an 8-bit sRGB channel.
fn channel(component: CGFloat) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_round_and_clamp() {
        assert_eq!(channel(0.0), 0);
        assert_eq!(channel(1.0), 255);
        assert_eq!(channel(0.5), 128);
        assert_eq!(channel(0.123), 31);
        assert_eq!(channel(-2.0), 0);
        assert_eq!(channel(2.0), 255);
    }

    #[test]
    fn a_capture_point_must_be_close_to_the_click() {
        assert!(near(Some((10.0, 20.0)), (10.0, 20.0)));
        assert!(near(Some((10.1, 19.9)), (10.0, 20.0)));
        assert!(!near(Some((10.5, 20.0)), (10.0, 20.0)));
        assert!(!near(None, (10.0, 20.0)));
    }
}
