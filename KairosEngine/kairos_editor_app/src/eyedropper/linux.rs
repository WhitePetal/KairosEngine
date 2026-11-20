//! X11 screen sampling and the loupe overlay.
//!
//! A pick uses a private X connection: QueryPointer for the global pointer,
//! GetImage for the 15x15 block around it, and an override-redirect window for
//! the loupe. While a pick runs the pointer is grabbed with button events
//! selected, so the click that confirms it cannot press a widget, another
//! application, or this window's own title bar. Motion is not selected: the
//! cursor keeps moving and the grab does not flood the queue with moves.
//!
//! Wayland is deliberately unavailable. A client cannot read pixels outside
//! its own surface without the ScreenCast portal and has no global pointer, so
//! there is no honest desktop sampler to write here. XWayland is treated the
//! same way: its root window does not contain Wayland clients' pixels. The
//! widget keeps its "not available" tooltip on those sessions.

use kairos_editor_widgets::color_picker::eyedropper::Outcome;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;
use x11rb::connection::Connection;
use x11rb::image::{Image, PixelLayout};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConfigureWindowAux, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask,
    Gcontext, GrabMode, GrabStatus, KeyButMask, Keycode, Setup, StackMode, Window as XWindow,
    WindowClass,
};
use x11rb::rust_connection::RustConnection;

use crate::eyedropper::Sample;

use super::loupe::{GRID, HALF, Loupe};

/// The host-side picker: the X connection, the loupe window and its state.
pub struct ScreenPicker {
    picker: Option<Picker>,
    /// The connection of a pick that ended on a click. The host asks whether
    /// the button is still down before it lets pointer events reach egui
    /// again; the connection goes away as soon as it is up.
    watch: Option<Watch>,
    running: bool,
    /// The last poll ended the pick. A click is still held, so stop() keeps a
    /// watch instead of dropping the connection with the overlay.
    finishing: bool,
}

struct Watch {
    conn: RustConnection,
    root: XWindow,
}

impl Watch {
    fn down(&self) -> bool {
        let Ok(cookie) = self.conn.query_pointer(self.root) else {
            return false;
        };
        let Ok(reply) = cookie.reply() else {
            return false;
        };
        reply
            .mask
            .intersects(KeyButMask::BUTTON1 | KeyButMask::BUTTON2 | KeyButMask::BUTTON3)
    }
}

impl ScreenPicker {
    pub const fn new() -> Self {
        Self {
            picker: None,
            watch: None,
            running: false,
            finishing: false,
        }
    }

    /// Whether a pick can sample this window's display.
    ///
    /// The raw handle has to be Xlib/XCB. The environment checks catch an X11
    /// client running under XWayland: winit may choose X11 there, but the root
    /// window is not the whole desktop any more.
    pub fn available_for(window: &Window) -> bool {
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return false;
        }
        if std::env::var_os("XDG_SESSION_TYPE")
            .is_some_and(|kind| kind.to_string_lossy() == "wayland")
        {
            return false;
        }

        let Ok(handle) = window.window_handle() else {
            return false;
        };
        matches!(
            handle.as_raw(),
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
        )
    }

    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// The confirming click is grabbed by the X server, but the release that
    /// follows may still reach this window once the grab is gone. The host
    /// keeps pointer events away from egui until the button is up.
    pub const fn masks_pointer(&self) -> bool {
        true
    }

    /// Whether a mouse button is physically down, for the release guard.
    ///
    /// This is only called after a pick ended on a click, at which point the
    /// connection was kept in a watch. It is a synchronous X round trip, but
    /// the host stops asking as soon as the button is up.
    pub fn any_button_down(&mut self) -> bool {
        let Some(watch) = self.watch.as_ref() else {
            return false;
        };
        let down = watch.down();
        if !down {
            self.watch = None;
        }
        down
    }

    pub fn start(&mut self, window: &Window) -> bool {
        if self.running {
            return true;
        }
        if !Self::available_for(window) {
            return false;
        }

        let Some(picker) = Picker::open() else {
            return false;
        };
        self.watch = None;
        self.picker = Some(picker);
        self.running = true;
        self.finishing = false;
        window.set_cursor(winit::window::CursorIcon::Crosshair);
        true
    }

    pub fn poll(&mut self) -> Option<Sample> {
        if !self.running {
            return None;
        }
        let sample = self.picker.as_mut()?.poll()?;
        if sample.outcome != Outcome::Picking {
            self.finishing = true;
        }
        Some(sample)
    }

    pub fn stop(&mut self, window: Option<&Window>) {
        if !self.running {
            return;
        }
        self.running = false;
        let finishing = std::mem::take(&mut self.finishing);
        if let Some(picker) = self.picker.take() {
            self.watch = picker.close(finishing);
        }
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

/// The open pick: one X connection, one loupe window, one grabbed pointer.
struct Picker {
    conn: RustConnection,
    root: XWindow,
    screen_width: u16,
    screen_height: u16,
    layout: PixelLayout,
    loupe: Loupe,
    overlay: XWindow,
    gc: Gcontext,
    image: Image<'static>,
    mapped: bool,
    last_rect: Option<(i16, i16, u16, u16)>,
    grabbed: bool,
    /// The click that started the pick may still be held. Until it is released
    /// once, no button counts as the confirming press.
    armed: bool,
    previous_left: bool,
    previous_right: bool,
    previous_escape: bool,
    escape: Option<Keycode>,
}

impl Picker {
    fn open() -> Option<Self> {
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(connection) => connection,
            Err(error) => {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "eyedropper: cannot open the X display: {error}"
                );
                return None;
            }
        };

        let setup = conn.setup();
        let screen = setup.roots.get(screen_num)?;
        let (root, root_depth, screen_width, screen_height, width_mm, width_px) = (
            screen.root,
            screen.root_depth,
            screen.width_in_pixels,
            screen.height_in_pixels,
            screen.width_in_millimeters,
            screen.width_in_pixels,
        );
        let visual = screen
            .allowed_depths
            .iter()
            .flat_map(|depth| depth.visuals.iter())
            .find(|visual| visual.visual_id == screen.root_visual)
            .copied()?;
        let layout = match PixelLayout::from_visual_type(visual) {
            Ok(layout) => layout,
            Err(error) => {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "eyedropper: the root visual is not TrueColor ({error:?})"
                );
                return None;
            }
        };

        let dpi = detect_dpi(&conn, root, width_px, width_mm);
        let loupe = Loupe::for_dpi(dpi);
        let escape = escape_keycode(&conn, setup);

        let overlay = conn.generate_id().ok()?;
        let gc = conn.generate_id().ok()?;
        let background = layout.encode((0, 0, 0));
        let created = conn
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                overlay,
                root,
                0,
                0,
                loupe.width,
                loupe.height,
                0,
                WindowClass::INPUT_OUTPUT,
                x11rb::COPY_FROM_PARENT,
                &CreateWindowAux::new()
                    .background_pixel(background)
                    .override_redirect(1u32)
                    .event_mask(EventMask::NO_EVENT),
            )
            .ok()?
            .check();
        if let Err(error) = created {
            kairos_log::warn!(
                target: "kairos_editor",
                "eyedropper: cannot create the X loupe window: {error}"
            );
            return None;
        }
        if conn
            .create_gc(gc, overlay, &CreateGCAux::new().graphics_exposures(0u32))
            .ok()?
            .check()
            .is_err()
        {
            let _ = conn.destroy_window(overlay);
            return None;
        }
        let image = match Image::allocate_native(loupe.width, loupe.height, root_depth, setup) {
            Ok(image) => image,
            Err(error) => {
                kairos_log::warn!(
                    target: "kairos_editor",
                    "eyedropper: cannot allocate the X loupe image: {error:?}"
                );
                let _ = conn.destroy_window(overlay);
                return None;
            }
        };

        let grabbed = conn
            .grab_pointer(
                false,
                root,
                EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
                x11rb::NONE,
                x11rb::NONE,
                x11rb::CURRENT_TIME,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| reply.status == GrabStatus::SUCCESS);
        if !grabbed {
            kairos_log::warn!(
                target: "kairos_editor",
                "eyedropper: pointer grab unavailable; clicks can reach other windows during a pick"
            );
        }

        Some(Self {
            conn,
            root,
            screen_width,
            screen_height,
            layout,
            loupe,
            overlay,
            gc,
            image,
            mapped: false,
            last_rect: None,
            grabbed,
            armed: false,
            previous_left: false,
            previous_right: false,
            previous_escape: false,
            escape,
        })
    }

    fn poll(&mut self) -> Option<Sample> {
        let pointer = self.conn.query_pointer(self.root).ok()?.reply().ok()?;
        let left = pointer.mask.contains(KeyButMask::BUTTON1);
        let right = pointer.mask.contains(KeyButMask::BUTTON3);
        let any = pointer
            .mask
            .intersects(KeyButMask::BUTTON1 | KeyButMask::BUTTON2 | KeyButMask::BUTTON3);

        let outcome = self.outcome(left, right, any);
        let (cursor_x, cursor_y) = (pointer.root_x, pointer.root_y);

        // The loupe is part of the desktop. Hide it before copying a block
        // that could contain the previous frame's panel.
        if self.overlaps_loupe(cursor_x, cursor_y) {
            self.hide();
        }
        let block = self.capture(cursor_x, cursor_y);
        let srgb = block.map(|block| {
            let (red, green, blue) = self.layout.decode(block[HALF * GRID + HALF]);
            [(red >> 8) as u8, (green >> 8) as u8, (blue >> 8) as u8, 255]
        });
        if let Some(block) = block {
            self.place(cursor_x, cursor_y);
            self.draw(&block, srgb.unwrap_or([0, 0, 0, 255]));
        }
        let _ = self.conn.flush();

        Some(Sample { srgb, outcome })
    }

    fn outcome(&mut self, left: bool, right: bool, any: bool) -> Outcome {
        let mut outcome = Outcome::Picking;

        // A press can be shorter than one frame, so read it from the grab's
        // queue as well as from the button state.
        loop {
            match self.conn.poll_for_event() {
                Ok(Some(Event::ButtonPress(event))) => match event.detail {
                    1 => outcome = Outcome::Committed,
                    3 => outcome = Outcome::Cancelled,
                    _ => {}
                },
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(_) => break,
            }
        }

        if !self.armed {
            if !any {
                self.armed = true;
            }
        } else if left && !self.previous_left {
            outcome = Outcome::Committed;
        } else if right && !self.previous_right {
            outcome = Outcome::Cancelled;
        }

        // The keyboard is not grabbed, so Esc works wherever focus is.
        let escape = self.escape_down();
        if escape && !self.previous_escape {
            outcome = Outcome::Cancelled;
        }

        self.previous_left = left;
        self.previous_right = right;
        self.previous_escape = escape;
        outcome
    }

    fn escape_down(&self) -> bool {
        let Some(code) = self.escape else {
            return false;
        };
        let Ok(cookie) = self.conn.query_keymap() else {
            return false;
        };
        let Ok(reply) = cookie.reply() else {
            return false;
        };
        let index = usize::from(code) / 8;
        let bit = usize::from(code) % 8;
        reply
            .keys
            .get(index)
            .is_some_and(|byte| byte & (1u8 << bit) != 0)
    }

    /// Copy the 15x15 block under the pointer, clipped to the root window.
    ///
    /// Near an edge the block is padded with black, so the centre cell is
    /// always the pixel under the pointer.
    fn capture(&self, cursor_x: i16, cursor_y: i16) -> Option<[u32; GRID * GRID]> {
        let (left, top, right, bottom) =
            clipped_block(cursor_x, cursor_y, (self.screen_width, self.screen_height))?;
        let want_x = i32::from(cursor_x) - HALF as i32;
        let want_y = i32::from(cursor_y) - HALF as i32;

        let (image, _visual) = Image::get(
            &self.conn,
            self.root,
            left as i16,
            top as i16,
            (right - left) as u16,
            (bottom - top) as u16,
        )
        .ok()?;

        let mut block = [0u32; GRID * GRID];
        for row in 0..(bottom - top) {
            for column in 0..(right - left) {
                let block_x = (left - want_x) + column;
                let block_y = (top - want_y) + row;
                let index = block_y * GRID as i32 + block_x;
                block[index as usize] = image.get_pixel(column as u16, row as u16);
            }
        }
        Some(block)
    }

    fn overlaps_loupe(&self, cursor_x: i16, cursor_y: i16) -> bool {
        let Some((x, y, width, height)) = self.last_rect else {
            return false;
        };
        let left = i32::from(cursor_x) - HALF as i32;
        let top = i32::from(cursor_y) - HALF as i32;
        let right = left + GRID as i32;
        let bottom = top + GRID as i32;
        left < i32::from(x) + i32::from(width)
            && i32::from(x) < right
            && top < i32::from(y) + i32::from(height)
            && i32::from(y) < bottom
    }

    fn place(&mut self, cursor_x: i16, cursor_y: i16) {
        let (x, y) = self.loupe.place(
            (cursor_x, cursor_y),
            (self.screen_width, self.screen_height),
        );
        let _ = self.conn.configure_window(
            self.overlay,
            &ConfigureWindowAux::new()
                .x(i32::from(x))
                .y(i32::from(y))
                .stack_mode(StackMode::ABOVE),
        );
        if !self.mapped {
            let _ = self.conn.map_window(self.overlay);
            self.mapped = true;
        }
        self.last_rect = Some((x, y, self.loupe.width, self.loupe.height));
    }

    fn hide(&mut self) {
        if self.mapped {
            let _ = self.conn.unmap_window(self.overlay);
            self.mapped = false;
        }
        self.last_rect = None;
    }

    fn draw(&mut self, block: &[u32; GRID * GRID], srgb: [u8; 4]) {
        let layout = self.layout;
        let loupe = &self.loupe;
        let image = &mut self.image;
        loupe.paint(block, srgb, |x, y, rgb| {
            let pixel = layout.encode((
                u16::from(rgb[0]) << 8,
                u16::from(rgb[1]) << 8,
                u16::from(rgb[2]) << 8,
            ));
            if x < loupe.width && y < loupe.height {
                image.put_pixel(x, y, pixel);
            }
        });
        let _ = image.put(&self.conn, self.overlay, self.gc, 0, 0);
    }

    fn buttons_down(&self) -> bool {
        let Ok(cookie) = self.conn.query_pointer(self.root) else {
            return false;
        };
        let Ok(reply) = cookie.reply() else {
            return false;
        };
        reply
            .mask
            .intersects(KeyButMask::BUTTON1 | KeyButMask::BUTTON2 | KeyButMask::BUTTON3)
    }

    /// Release the pointer and destroy the overlay; keep the connection only
    /// when a click is still held and the host wants the release guard.
    fn close(self, keep_watch: bool) -> Option<Watch> {
        let held = self.buttons_down();
        if self.grabbed {
            let _ = self.conn.ungrab_pointer(x11rb::CURRENT_TIME);
        }
        if self.mapped {
            let _ = self.conn.unmap_window(self.overlay);
        }
        let _ = self.conn.free_gc(self.gc);
        let _ = self.conn.destroy_window(self.overlay);
        let _ = self.conn.flush();

        if keep_watch && held {
            Some(Watch {
                conn: self.conn,
                root: self.root,
            })
        } else {
            None
        }
    }
}

/// The part of the 15x15 block that lies on the root window.
///
/// Returns the clipped rectangle and leaves the block's origin at
/// cursor - HALF, so the caller can pad the outside with black and keep the
/// centre cell on the pointer.
fn clipped_block(cursor_x: i16, cursor_y: i16, screen: (u16, u16)) -> Option<(i32, i32, i32, i32)> {
    let want_x = i32::from(cursor_x) - HALF as i32;
    let want_y = i32::from(cursor_y) - HALF as i32;
    let left = want_x.max(0);
    let top = want_y.max(0);
    let right = (want_x + GRID as i32).min(i32::from(screen.0));
    let bottom = (want_y + GRID as i32).min(i32::from(screen.1));
    (right > left && bottom > top).then_some((left, top, right, bottom))
}

/// The desktop's DPI, for the loupe's size. Xft.dpi is the authoritative
/// value when a desktop sets it; the physical millimetres are a fallback for
/// bare X sessions, where EDID is often absent.
fn detect_dpi(conn: &RustConnection, root: XWindow, width_px: u16, width_mm: u16) -> f32 {
    if let Some(dpi) = xft_dpi(conn, root) {
        return dpi;
    }
    if width_mm >= 50 {
        let dpi = f32::from(width_px) * 25.4 / f32::from(width_mm);
        if (50.0..=500.0).contains(&dpi) {
            return dpi;
        }
    }
    96.0
}

fn xft_dpi(conn: &RustConnection, root: XWindow) -> Option<f32> {
    let reply = conn
        .get_property(
            false,
            root,
            AtomEnum::RESOURCE_MANAGER,
            AtomEnum::STRING,
            0,
            1 << 16,
        )
        .ok()?
        .reply()
        .ok()?;
    super::loupe::parse_xft_dpi(&reply.value)
}

/// The keycode whose keysyms include Escape, for the global key-state poll.
fn escape_keycode(conn: &RustConnection, setup: &Setup) -> Option<Keycode> {
    let count = setup
        .max_keycode
        .checked_sub(setup.min_keycode)?
        .checked_add(1)?;
    let reply = conn
        .get_keyboard_mapping(setup.min_keycode, count)
        .ok()?
        .reply()
        .ok()?;
    let per_keycode = usize::from(reply.keysyms_per_keycode);
    if per_keycode == 0 {
        return None;
    }
    for (index, keysyms) in reply.keysyms.chunks(per_keycode).enumerate() {
        if keysyms.contains(&0xff1b) {
            return setup.min_keycode.checked_add(u8::try_from(index).ok()?);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_that_leaves_the_screen_is_clipped_to_the_root() {
        let screen = (1920, 1080);
        for cursor in [(0i16, 0i16), (1, 2), (1919, 1079)] {
            let (left, top, right, bottom) = clipped_block(cursor.0, cursor.1, screen)
                .expect("the cursor is on the screen, so the block is not empty");
            assert!(left >= 0 && top >= 0);
            assert!(right <= i32::from(screen.0) && bottom <= i32::from(screen.1));
            // The centre cell still exists inside the clipped block.
            assert!(i32::from(cursor.0) >= left && i32::from(cursor.0) < right);
            assert!(i32::from(cursor.1) >= top && i32::from(cursor.1) < bottom);
        }
    }

    #[test]
    fn a_block_off_the_screen_is_empty() {
        assert_eq!(clipped_block(-20, -20, (1920, 1080)), None);
        assert_eq!(clipped_block(2000, 2000, (1920, 1080)), None);
    }
}
