//! The macOS loupe: one transparent full-screen window per screen.
//!
//! Per-screen windows, rather than one window spanning the union of all
//! screens, are what macOS wants: displays may have separate Spaces and
//! different backing scales, and a single window cannot belong to two Spaces.
//! Each view is flipped so layout matches the Windows loupe's top-left
//! coordinates, and the view under the pointer draws the panel; the others
//! stay transparent.
//!
//! The Windows panel's geometry is reused through loupe::Loupe::for_points,
//! while the pixels and text are drawn natively at the display's backing scale.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use kairos_editor_widgets::color_picker::eyedropper::Outcome;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezierPath, NSColor, NSCursor, NSEvent, NSFont,
    NSFontAttributeName, NSFontWeightRegular, NSForegroundColorAttributeName, NSGraphicsContext,
    NSScreen, NSScreenSaverWindowLevel, NSStringDrawing, NSTrackingArea, NSTrackingAreaOptions,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::CGContext;
use objc2_foundation::{NSAttributedStringKey, NSDictionary, NSString};

use super::capture::CaptureResult;
use crate::eyedropper::loupe::{GRID, HALF, Loupe};

/// Everything the views and the capture pipeline share.
///
/// The view under the pointer writes the position and the terminal outcome;
/// the capture completion (on a private queue) writes the pixels; the picker
/// reads it all once per frame on the main thread.
pub(super) struct OverlayState {
    /// Pointer in global top-left points.
    pub(super) cursor: (f64, f64),
    /// Set by a view when the user commits or cancels.
    pub(super) outcome: Option<Outcome>,
    /// The point of a confirming click, while its capture is pending.
    pub(super) commit: Option<(f64, f64)>,
    /// The latest 15x15 RGBA block, row 0 = top.
    pub(super) block: [u8; GRID * GRID * 4],
    /// The latest centre pixel.
    pub(super) color: [u8; 4],
    pub(super) has_color: bool,
    /// A completed capture waiting for the picker to drain it.
    pub(super) pending: Option<CaptureResult>,
    pub(super) in_flight: bool,
    pub(super) in_flight_since: Option<Instant>,
    /// Incremented for every new capture; stale completions are ignored.
    pub(super) generation: u64,
    /// The point of the last successful capture.
    pub(super) last_capture: Option<(f64, f64)>,
    pub(super) error: Option<String>,
}

impl OverlayState {
    fn new(cursor: (f64, f64)) -> Self {
        Self {
            cursor,
            outcome: None,
            commit: None,
            block: [0; GRID * GRID * 4],
            color: [0; 4],
            has_color: false,
            pending: None,
            in_flight: false,
            in_flight_since: None,
            generation: 0,
            last_capture: None,
            error: None,
        }
    }
}

#[derive(Clone, Copy)]
struct ScreenInfo {
    /// Global top-left origin, in points.
    origin: (f64, f64),
    /// Size in points.
    size: (f64, f64),
}

impl ScreenInfo {
    fn contains(&self, point: (f64, f64)) -> bool {
        point.0 >= self.origin.0
            && point.0 < self.origin.0 + self.size.0
            && point.1 >= self.origin.1
            && point.1 < self.origin.1 + self.size.1
    }
}

/// The live overlay: one window and view per screen.
pub(super) struct Overlay {
    windows: Vec<Retained<OverlayWindow>>,
    views: Vec<Retained<OverlayView>>,
    state: Arc<Mutex<OverlayState>>,
    cursor_pushed: bool,
}

impl Overlay {
    pub(super) fn new(mtm: MainThreadMarker) -> Option<Self> {
        let screens = NSScreen::screens(mtm);
        if screens.is_empty() {
            return None;
        }

        // AppKit's global origin is the bottom-left of the screen with the
        // menu bar, which is always screens[0] and has frame origin (0, 0).
        // NSScreen::mainScreen is *not* that screen: it is the screen with the
        // keyboard focus. Using it would flip every point by the height of
        // whichever display the editor window happens to be on, so a pick that
        // starts on a secondary display would sample the wrong screen at the
        // wrong place.
        let anchor = screens
            .iter()
            .find(|screen| {
                let frame = screen.frame();
                frame.origin.x == 0.0 && frame.origin.y == 0.0
            })
            .or_else(|| screens.iter().next())?;
        let anchor_frame = anchor.frame();
        let menu_bar_top = anchor_frame.origin.y + anchor_frame.size.height;

        let pointer = NSEvent::mouseLocation();
        let cursor = (pointer.x, menu_bar_top - pointer.y);
        let state = Arc::new(Mutex::new(OverlayState::new(cursor)));

        let mut windows = Vec::new();
        let mut views = Vec::new();
        let mut active_screen = None;
        for (index, screen) in screens.iter().enumerate() {
            let frame = screen.frame();
            let info = ScreenInfo {
                origin: (
                    frame.origin.x,
                    menu_bar_top - (frame.origin.y + frame.size.height),
                ),
                size: (frame.size.width, frame.size.height),
            };
            if info.contains(cursor) && active_screen.is_none() {
                active_screen = Some(index);
            }

            let ivars = ViewIvars {
                state: Arc::clone(&state),
                screen: info,
                window_origin: (frame.origin.x, frame.origin.y),
                menu_bar_top,
                layout: Loupe::for_points(),
                panel: color(32.0 / 255.0, 32.0 / 255.0, 32.0 / 255.0),
                border: color(64.0 / 255.0, 64.0 / 255.0, 64.0 / 255.0),
                hex_attrs: text_attributes(232.0 / 255.0, 232.0 / 255.0, 232.0 / 255.0),
                dim_attrs: text_attributes(148.0 / 255.0, 148.0 / 255.0, 148.0 / 255.0),
            };

            let view = OverlayView::alloc(mtm).set_ivars(ivars);
            let view_rect = CGRect::new(
                CGPoint::ZERO,
                CGSize::new(frame.size.width, frame.size.height),
            );
            // SAFETY: the allocated view is initialised with a valid frame and
            // the class is a plain NSView subclass.
            let view: Retained<OverlayView> =
                unsafe { msg_send![super(view), initWithFrame: view_rect] };

            let owner: &AnyObject = unsafe { &*(&*view as *const OverlayView as *const AnyObject) };
            // SAFETY: the tracking area is owned by the view after
            // addTrackingArea, and the owner outlives it.
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    view_rect,
                    NSTrackingAreaOptions::MouseMoved
                        | NSTrackingAreaOptions::ActiveAlways
                        | NSTrackingAreaOptions::InVisibleRect,
                    Some(owner),
                    None,
                )
            };
            view.addTrackingArea(&area);

            let window = OverlayWindow::alloc(mtm).set_ivars(());
            // SAFETY: a borderless window on one screen; the content view is
            // set immediately below.
            let window: Retained<OverlayWindow> = unsafe {
                msg_send![
                    super(window),
                    initWithContentRect: frame,
                    styleMask: NSWindowStyleMask::Borderless,
                    backing: NSBackingStoreType::Buffered,
                    defer: false
                ]
            };
            // SAFETY: the window is not auto-released; we own it in the overlay.
            unsafe { window.setReleasedWhenClosed(false) };
            window.setBackgroundColor(Some(&NSColor::clearColor()));
            window.setOpaque(false);
            window.setHasShadow(false);
            window.setIgnoresMouseEvents(false);
            window.setAcceptsMouseMovedEvents(true);
            window.setHidesOnDeactivate(false);
            window.setLevel(NSScreenSaverWindowLevel);
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::Stationary,
            );
            window.setContentView(Some(&view));
            window.makeFirstResponder(Some(&view));
            window.orderFrontRegardless();

            windows.push(window);
            views.push(view);
        }

        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        if let Some(index) = active_screen.or(Some(0)) {
            if let Some(window) = windows.get(index) {
                window.makeKeyAndOrderFront(None);
            }
        }
        NSCursor::crosshairCursor().push();

        Some(Self {
            windows,
            views,
            state,
            cursor_pushed: true,
        })
    }

    pub(super) fn state(&self) -> Arc<Mutex<OverlayState>> {
        Arc::clone(&self.state)
    }

    /// The pointer the overlay was opened at.
    pub(super) fn cursor(&self) -> (f64, f64) {
        self.state
            .lock()
            .map(|state| state.cursor)
            .unwrap_or((0.0, 0.0))
    }

    pub(super) fn set_needs_display(&self) {
        for view in &self.views {
            view.setNeedsDisplay(true);
        }
    }

    pub(super) fn close(&mut self) {
        for window in &self.windows {
            window.orderOut(None);
        }
        if self.cursor_pushed {
            NSCursor::pop_class();
            self.cursor_pushed = false;
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.close();
    }
}

struct ViewIvars {
    state: Arc<Mutex<OverlayState>>,
    /// This window's screen in global top-left points.
    screen: ScreenInfo,
    /// This window's frame origin in global bottom-left points.
    window_origin: (f64, f64),
    /// Global top-left y of the menu-bar screen's top edge.
    menu_bar_top: f64,
    layout: Loupe,
    panel: Retained<NSColor>,
    border: Retained<NSColor>,
    hex_attrs: Retained<NSDictionary<NSAttributedStringKey, AnyObject>>,
    dim_attrs: Retained<NSDictionary<NSAttributedStringKey, AnyObject>>,
}

define_class!(
    // SAFETY: NSWindow has no subclassing requirements beyond the overridable
    // key/main-window predicates, which are implemented below.
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ()]
    struct OverlayWindow;

    impl OverlayWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

define_class!(
    // SAFETY: NSView has no subclassing requirements; the overrides below only
    // read and write the view's own ivars.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ViewIvars]
    struct OverlayView;

    impl OverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: &NSEvent) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: CGRect) {
            let Some(context) = NSGraphicsContext::currentContext() else {
                return;
            };
            let cg = context.CGContext();
            let (cursor, block, color, has_color) = {
                let Ok(state) = self.ivars().state.lock() else {
                    return;
                };
                (state.cursor, state.block, state.color, state.has_color)
            };

            // Only the view for the screen under the pointer paints; the other
            // per-screen windows stay transparent.
            let screen = self.ivars().screen;
            if !screen.contains(cursor) {
                return;
            }
            let local = (cursor.0 - screen.origin.0, cursor.1 - screen.origin.1);
            let (left, top) = self.ivars().layout.place(
                (local.0.round() as i16, local.1.round() as i16),
                (
                    screen.size.0.round() as u16,
                    screen.size.1.round() as u16,
                ),
            );
            let origin = (f64::from(left), f64::from(top));

            let layout = &self.ivars().layout;
            let panel = CGRect::new(
                CGPoint::new(origin.0, origin.1),
                CGSize::new(f64::from(layout.width), f64::from(layout.height)),
            );
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(panel, 10.0, 10.0);
            self.ivars().panel.setFill();
            path.fill();

            let cell = f64::from(layout.cell);
            let margin = f64::from(layout.margin);
            let grid_x = origin.0 + margin;
            let grid_y = origin.1 + margin;
            for row in 0..GRID {
                for column in 0..GRID {
                    let index = (row * GRID + column) * 4;
                    let rgb = if has_color {
                        [block[index], block[index + 1], block[index + 2]]
                    } else {
                        [0, 0, 0]
                    };
                    fill_rect(
                        &cg,
                        grid_x + column as f64 * cell,
                        grid_y + row as f64 * cell,
                        cell,
                        cell,
                        rgb,
                    );
                }
            }

            let centre_x = grid_x + HALF as f64 * cell;
            let centre_y = grid_y + HALF as f64 * cell;
            frame_rect(&cg, centre_x, centre_y, cell, cell, [64, 150, 224]);
            frame_rect(
                &cg,
                centre_x + 1.0,
                centre_y + 1.0,
                cell - 2.0,
                cell - 2.0,
                [255, 255, 255],
            );

            let text_x = origin.0 + margin;
            let text_y = origin.1 + f64::from(layout.text_top);
            let chip = f64::from(layout.chip);
            let text_width = f64::from(layout.width) - margin * 2.0 - chip - 3.0;
            let hex = if has_color {
                format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
            } else {
                "#------".to_owned()
            };
            let numbers = if has_color {
                format!("{} {} {}", color[0], color[1], color[2])
            } else {
                String::new()
            };
            draw_text(
                &self.ivars().hex_attrs,
                &hex,
                CGPoint::new(text_x, text_y),
                text_width,
            );
            draw_text(
                &self.ivars().dim_attrs,
                &numbers,
                CGPoint::new(text_x, text_y + f64::from(layout.line)),
                text_width,
            );

            let chip_x = origin.0 + f64::from(layout.width) - margin - chip;
            let chip_color = if has_color {
                [color[0], color[1], color[2]]
            } else {
                [16, 16, 16]
            };
            fill_rect(&cg, chip_x, text_y, chip, chip, chip_color);
            frame_rect(&cg, chip_x, text_y, chip, chip, [64, 64, 64]);

            self.ivars().border.setStroke();
            path.setLineWidth(1.0);
            path.stroke();
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let point = self.global_top_left(event);
            let Ok(mut state) = self.ivars().state.lock() else {
                return;
            };
            if state.outcome.is_some() {
                return;
            }
            state.cursor = point;
            drop(state);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let point = self.global_top_left(event);
            let Ok(mut state) = self.ivars().state.lock() else {
                return;
            };
            if state.outcome.is_some() {
                return;
            }
            state.cursor = point;
            state.commit = Some(point);
            state.outcome = Some(Outcome::Committed);
            drop(state);
            self.setNeedsDisplay(true);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, _event: &NSEvent) {
            self.cancel();
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if event.keyCode() == 53 {
                self.cancel();
            } else {
                // SAFETY: passing an unhandled key to NSView is the normal
                // responder-chain behaviour.
                unsafe {
                    let _: () = msg_send![super(self), keyDown: event];
                }
            }
        }

        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.cancel();
        }
    }
);

impl OverlayView {
    fn global_top_left(&self, event: &NSEvent) -> (f64, f64) {
        let local = event.locationInWindow();
        let global_x = self.ivars().window_origin.0 + local.x;
        let global_y = self.ivars().window_origin.1 + local.y;
        (global_x, self.ivars().menu_bar_top - global_y)
    }

    fn cancel(&self) {
        let Ok(mut state) = self.ivars().state.lock() else {
            return;
        };
        if state.outcome.is_none() {
            state.outcome = Some(Outcome::Cancelled);
        }
        drop(state);
        self.setNeedsDisplay(true);
    }
}

fn color(red: f64, green: f64, blue: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, 1.0)
}

fn any<T>(object: &T) -> &AnyObject {
    // SAFETY: every Objective-C object begins with its isa pointer, which is
    // the layout of AnyObject. The reference is only used for the duration of
    // the dictionary construction, which retains the objects.
    unsafe { &*(object as *const T as *const AnyObject) }
}

fn text_attributes(
    red: f64,
    green: f64,
    blue: f64,
) -> Retained<NSDictionary<NSAttributedStringKey, AnyObject>> {
    let font = NSFont::monospacedSystemFontOfSize_weight(11.0, unsafe { NSFontWeightRegular });
    let color = color(red, green, blue);
    let keys: [&NSAttributedStringKey; 2] =
        unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [any(&*font), any(&*color)];
    NSDictionary::from_slices(&keys, &values)
}

fn fill_rect(cg: &CGContext, x: f64, y: f64, width: f64, height: f64, rgb: [u8; 3]) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    CGContext::set_rgb_fill_color(
        Some(cg),
        f64::from(rgb[0]) / 255.0,
        f64::from(rgb[1]) / 255.0,
        f64::from(rgb[2]) / 255.0,
        1.0,
    );
    CGContext::fill_rect(
        Some(cg),
        CGRect::new(CGPoint::new(x, y), CGSize::new(width, height)),
    );
}

fn frame_rect(cg: &CGContext, x: f64, y: f64, width: f64, height: f64, rgb: [u8; 3]) {
    fill_rect(cg, x, y, width, 1.0, rgb);
    fill_rect(cg, x, y + height - 1.0, width, 1.0, rgb);
    fill_rect(cg, x, y, 1.0, height, rgb);
    fill_rect(cg, x + width - 1.0, y, 1.0, height, rgb);
}

fn draw_text(
    attrs: &NSDictionary<NSAttributedStringKey, AnyObject>,
    text: &str,
    origin: CGPoint,
    width: f64,
) {
    let string = NSString::from_str(text);
    // SAFETY: the attributes dictionary uses the documented keys and values,
    // and the rect is a finite, positive-size rectangle.
    unsafe {
        string
            .drawInRect_withAttributes(CGRect::new(origin, CGSize::new(width, 14.0)), Some(attrs));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_owns_the_half_open_rectangle() {
        let screen = ScreenInfo {
            origin: (0.0, 0.0),
            size: (1920.0, 1080.0),
        };
        assert!(screen.contains((0.0, 0.0)));
        assert!(screen.contains((1919.9, 1079.9)));
        assert!(!screen.contains((1920.0, 0.0)));
        assert!(!screen.contains((0.0, 1080.0)));
    }
}
