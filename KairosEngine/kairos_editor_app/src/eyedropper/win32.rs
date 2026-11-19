//! Windows screen sampling and the loupe overlay.
//!
//! A pick reads the desktop directly: `GetCursorPos` for the pointer, a 15x15
//! `BitBlt` from the screen DC for the pixels, and `GetAsyncKeyState` for the
//! click/key that ends the pick. That is why a pick can end anywhere on the
//! desktop, not only over the editor window: the OS, not winit, answers where the
//! pointer is.
//!
//! The loupe is a topmost, click-through, no-activate popup that follows the
//! cursor. It draws with GDI into a DIB back buffer and blits that to its window,
//! so it stays independent of the engine's wgpu swapchain and of the egui pass.
//!
//! A pick also switches the desktop's mouse buttons off: a low-level mouse hook
//! discards button and wheel events for the whole pick, so the click that
//! confirms it cannot press a widget, another application, or this window's own
//! caption buttons. Hover moves must keep flowing - the system moves the cursor
//! as it delivers them, so swallowing them freezes the pointer. A discarded
//! event is invisible to `GetAsyncKeyState` too, so the hook records the
//! confirming/cancelling press itself (see `MASK_EVENTS`).

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use kairos_editor_widgets::color_picker::eyedropper::Outcome;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, CLEARTYPE_QUALITY,
    CLIP_DEFAULT_PRECIS, COLORONCOLOR, CreateCompatibleDC, CreateDIBSection, CreateFontW,
    CreateRoundRectRgn, CreateSolidBrush, DEFAULT_CHARSET, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    FF_MODERN, FW_NORMAL, FillRect, FrameRect, GetDC, GetMonitorInfoW, HBITMAP, HBRUSH, HDC, HFONT,
    HGDIOBJ, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, OUT_DEFAULT_PRECIS,
    ReleaseDC, SRCCOPY, SelectObject, SetBkMode, SetStretchBltMode, SetTextColor, SetWindowRgn,
    StretchBlt, TRANSPARENT, TextOutW, ValidateRect,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, ReleaseCapture, SetCapture, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos, HHOOK,
    HTTRANSPARENT, HWND_TOPMOST, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SetWindowPos, SetWindowsHookExW, ShowWindow, UnhookWindowsHookEx, WH_MOUSE_LL,
    WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEACTIVATE,
    WM_MOUSEHWHEEL, WM_MOUSEWHEEL, WM_NCHITTEST, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use crate::eyedropper::Sample;

/// The loupe samples a 15x15 block around the pointer.
const GRID: i32 = 15;
/// The pointer always samples the block's centre pixel.
const HALF: i32 = GRID / 2;

const CLASS_NAME: &str = "KairosEyedropperLoupe";

/// The host-side picker: pointer, pixels, loupe.
pub struct ScreenPicker {
    overlay: Option<Overlay>,
    /// The desktop's mouse input, masked while the pick runs.
    mask: Option<InputMask>,
    previous: Buttons,
    /// The first poll only records the button state: the click that armed the
    /// pick may still be down, and its release must not count as a confirm.
    primed: bool,
    running: bool,
}

impl ScreenPicker {
    pub const fn new() -> Self {
        Self {
            overlay: None,
            mask: None,
            previous: Buttons::none(),
            primed: false,
            running: false,
        }
    }

    /// Windows can sample the whole desktop.
    pub const fn available() -> bool {
        true
    }

    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Whether any mouse button is physically down right now.
    ///
    /// The pick ends on a press, and the matching release still has to be kept
    /// away from egui: the pointer position egui holds is the one from before
    /// the pick (it never saw the moves), so a stray release would land on the
    /// eyedropper button and start a new pick.
    pub fn any_button_down(&self) -> bool {
        key_down(VK_LBUTTON as i32) || key_down(VK_RBUTTON as i32) || key_down(VK_MBUTTON as i32)
    }

    /// Start reading the pointer. Returns false if the window has no Win32
    /// handle or the loupe could not be created.
    pub fn start(&mut self, window: &Window) -> bool {
        if self.running {
            return true;
        }

        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return false;
        };
        let owner = handle.hwnd.get() as HWND;
        if !ensure_class() {
            return false;
        }

        let dpi = unsafe { GetDpiForWindow(owner) };
        let Some(overlay) = Overlay::new(dpi.max(96)) else {
            return false;
        };

        // Keep the click that confirms the pick away from the window under the
        // pointer. The pick still polls the key state, so losing the message
        // stream would not lose the pick.
        unsafe { SetCapture(owner) };
        // Capture does not cover this window's own caption buttons, so the
        // whole desktop gets its mouse buttons masked as well.
        self.mask = InputMask::install();
        if self.mask.is_none() {
            kairos_log::warn!(
                target: "kairos_editor",
                "eyedropper: desktop input mask unavailable; caption buttons stay live during a pick"
            );
        }
        window.set_cursor(winit::window::CursorIcon::Crosshair);

        self.overlay = Some(overlay);
        self.previous = Buttons::read();
        self.primed = false;
        self.running = true;
        true
    }

    /// One frame of sampling: the colour under the pointer, the loupe, and
    /// whether the pick ended.
    pub fn poll(&mut self) -> Option<Sample> {
        if !self.running {
            return None;
        }

        let mut cursor = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor) } == 0 {
            return None;
        }
        let overlay = self.overlay.as_mut()?;

        // The loupe is part of the desktop. Hide it while copying a block that
        // could contain it: a fast pointer can move into the previous frame's
        // loupe position.
        let capture = RECT {
            left: cursor.x - HALF,
            top: cursor.y - HALF,
            right: cursor.x + HALF + 1,
            bottom: cursor.y + HALF + 1,
        };
        if overlay.visible && intersects(&overlay.last_rect, &capture) {
            overlay.hide();
        }

        let block = overlay.capture(cursor);
        let center = block[(HALF * GRID + HALF) as usize];
        let srgb = [
            ((center >> 16) & 0xff) as u8,
            ((center >> 8) & 0xff) as u8,
            (center & 0xff) as u8,
            255,
        ];

        overlay.place(cursor);
        overlay.draw(srgb);

        let keys = Buttons::read();
        let mut outcome = Outcome::Picking;
        if self.mask.is_some() {
            // A discarded event never reaches `GetAsyncKeyState`, so the
            // masked press is read from the hook's record instead.
            let events = MASK_EVENTS.swap(0, Ordering::SeqCst);
            if events & CONFIRM != 0 {
                outcome = Outcome::Committed;
            } else if events & CANCEL != 0 {
                outcome = Outcome::Cancelled;
            }
        } else if !self.primed {
            self.primed = true;
        } else if keys.left && !self.previous.left {
            outcome = Outcome::Committed;
        } else if keys.right && !self.previous.right {
            outcome = Outcome::Cancelled;
        }
        // The keyboard is not masked, so Esc still works in both modes.
        if outcome == Outcome::Picking && keys.escape && !self.previous.escape {
            outcome = Outcome::Cancelled;
        }
        self.previous = keys;

        Some(Sample { srgb, outcome })
    }

    /// Stop reading the pointer and hide the loupe.
    pub fn stop(&mut self, window: Option<&Window>) {
        if !self.running {
            return;
        }
        self.running = false;
        MASK_EVENTS.store(0, Ordering::SeqCst);
        self.mask = None;
        unsafe { ReleaseCapture() };
        if let Some(overlay) = self.overlay.as_mut() {
            overlay.hide();
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

/// A button the mask saw that the pick treats as confirm or cancel.
const CONFIRM: u32 = 1 << 0;
const CANCEL: u32 = 1 << 1;

/// Buttons recorded for the pick. The hook procedure cannot reach
/// [`ScreenPicker`] (it is one plain function for the process), and the pick
/// cannot poll the key state instead: an event a low-level hook discards never
/// reaches `GetAsyncKeyState`, so the record *is* the click from the pick's
/// point of view. Both run on the event-loop thread.
static MASK_EVENTS: AtomicU32 = AtomicU32::new(0);

/// The desktop's pointer buttons and wheel, switched off for the duration of a
/// pick. Hover moves are deliberately left alone: the system moves the cursor
/// while it delivers them, so a discarded move freezes the whole pointer.
///
/// `SetCapture` already keeps clicks on *other* windows away from them, but the
/// capture window still handles its own non-client area: a click on this
/// window's caption buttons would minimize, maximize or close it mid-pick. A
/// low-level mouse hook sees the event before Windows turns it into a click,
/// and discarding it there keeps it away from *every* window, caption included
/// - and from `GetAsyncKeyState`, which is why the confirming press is
/// recorded in [`MASK_EVENTS`] instead.
///
/// Hooks are delivered to the thread that installed them - the event-loop
/// thread - so the procedure does nothing but compare a message id. Windows
/// removes a low-level hook whose procedure stops answering, which doubles as
/// the watchdog if the event loop stalls.
struct InputMask {
    hook: HHOOK,
}

impl InputMask {
    /// Mask every thread's input (the whole desktop). `None` when the OS
    /// refuses; the pick then falls back to capture plus `GetAsyncKeyState`.
    fn install() -> Option<Self> {
        MASK_EVENTS.store(0, Ordering::SeqCst);
        let hook =
            unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mask_proc), GetModuleHandleW(null()), 0) };
        if hook.is_null() {
            None
        } else {
            Some(Self { hook })
        }
    }
}

impl Drop for InputMask {
    fn drop(&mut self) {
        unsafe { UnhookWindowsHookEx(self.hook) };
    }
}

unsafe extern "system" fn mask_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        match wparam as u32 {
            // The pick reads the press from this record: a discarded event
            // never reaches `GetAsyncKeyState`.
            WM_LBUTTONDOWN => {
                MASK_EVENTS.fetch_or(CONFIRM, Ordering::SeqCst);
                return 1;
            }
            WM_RBUTTONDOWN => {
                MASK_EVENTS.fetch_or(CANCEL, Ordering::SeqCst);
                return 1;
            }
            // Everything that could press a window is discarded. Moves are
            // *not* in this list: the system moves the cursor while it
            // delivers them, so a discarded move freezes the pointer (which
            // would make the pick unusable). They reach this window anyway
            // because the pick holds the capture, and the host keeps them
            // away from egui.
            WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_XBUTTONDOWN
            | WM_XBUTTONUP | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // Non-zero discards the event: no window sees it, not even
                // this one.
                return 1;
            }
            _ => {}
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

#[derive(Clone, Copy)]
struct Buttons {
    left: bool,
    right: bool,
    escape: bool,
}

impl Buttons {
    const fn none() -> Self {
        Self {
            left: false,
            right: false,
            escape: false,
        }
    }

    fn read() -> Self {
        Self {
            left: key_down(VK_LBUTTON as i32),
            right: key_down(VK_RBUTTON as i32),
            escape: key_down(VK_ESCAPE as i32),
        }
    }
}

fn key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// The loupe: a click-through popup with its own DIB back buffer.
struct Overlay {
    hwnd: HWND,
    dc: HDC,
    bitmap: HBITMAP,
    bitmap_old: HGDIOBJ,
    source: HDC,
    source_bitmap: HBITMAP,
    source_old: HGDIOBJ,
    source_bits: *mut u32,
    font: HFONT,
    panel: HBRUSH,
    border: HBRUSH,
    accent: HBRUSH,
    white: HBRUSH,
    scale: i32,
    cell: i32,
    margin: i32,
    width: i32,
    height: i32,
    text_top: i32,
    last_rect: RECT,
    visible: bool,
}

impl Overlay {
    fn new(dpi: u32) -> Option<Self> {
        let scale = ((dpi as f32 / 96.0).round() as i32).max(1);
        let cell = 8 * scale;
        let margin = 8 * scale;
        let grid = GRID * cell;
        let text_top = margin + grid + 3 * scale;
        let width = margin * 2 + grid;
        let height = text_top + 14 * scale + 12 * scale + margin;

        let class_name = wide(CLASS_NAME);
        let title = wide("");
        let instance = unsafe { GetModuleHandleW(null()) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                0,
                0,
                width,
                height,
                null_mut(),
                null_mut(),
                instance,
                null(),
            )
        };
        if hwnd.is_null() {
            return None;
        }

        // Windows 11 rounds top-level windows itself; older versions get a
        // region so the loupe does not read as a hard rectangle.
        let region =
            unsafe { CreateRoundRectRgn(0, 0, width + 1, height + 1, 10 * scale, 10 * scale) };
        if !region.is_null() {
            unsafe { SetWindowRgn(hwnd, region, 1) };
        }

        let screen = unsafe { GetDC(null_mut()) };
        let dc = unsafe { CreateCompatibleDC(screen) };
        let source_dc = unsafe { CreateCompatibleDC(screen) };
        if dc.is_null() || source_dc.is_null() {
            unsafe {
                if !dc.is_null() {
                    DeleteDC(dc);
                }
                if !source_dc.is_null() {
                    DeleteDC(source_dc);
                }
                ReleaseDC(null_mut(), screen);
                DestroyWindow(hwnd);
            }
            return None;
        }

        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        };
        let mut bits: *mut c_void = null_mut();
        let bitmap =
            unsafe { CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };

        let mut source_info = BITMAPINFO::default();
        source_info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: GRID,
            biHeight: -GRID,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        };
        let mut source_bits: *mut c_void = null_mut();
        let source_bitmap = unsafe {
            CreateDIBSection(
                screen,
                &source_info,
                DIB_RGB_COLORS,
                &mut source_bits,
                null_mut(),
                0,
            )
        };
        unsafe { ReleaseDC(null_mut(), screen) };

        if bitmap.is_null() || source_bitmap.is_null() {
            unsafe {
                if !bitmap.is_null() {
                    DeleteObject(bitmap as HGDIOBJ);
                }
                if !source_bitmap.is_null() {
                    DeleteObject(source_bitmap as HGDIOBJ);
                }
                DeleteDC(dc);
                DestroyWindow(hwnd);
            }
            return None;
        }

        let bitmap_old = unsafe { SelectObject(dc, bitmap as HGDIOBJ) };
        let source = source_dc;
        let source_old = unsafe { SelectObject(source, source_bitmap as HGDIOBJ) };

        let face = wide("Consolas");
        let font = unsafe {
            CreateFontW(
                -(11 * scale),
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                FF_MODERN as u32,
                face.as_ptr(),
            )
        };

        Some(Self {
            hwnd,
            dc,
            bitmap,
            bitmap_old,
            source,
            source_bitmap,
            source_old,
            source_bits: source_bits as *mut u32,
            font,
            panel: solid(32, 32, 32),
            border: solid(64, 64, 64),
            accent: solid(64, 150, 224),
            white: solid(255, 255, 255),
            scale,
            cell,
            margin,
            width,
            height,
            text_top,
            last_rect: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            visible: false,
        })
    }

    /// Copy the block under the pointer. The block's top-left is
    /// `cursor - HALF`, so the centre pixel is the one under the pointer.
    fn capture(&mut self, cursor: POINT) -> [u32; (GRID * GRID) as usize] {
        let mut block = [0u32; (GRID * GRID) as usize];
        unsafe {
            let screen = GetDC(null_mut());
            if !screen.is_null() {
                BitBlt(
                    self.source,
                    0,
                    0,
                    GRID,
                    GRID,
                    screen,
                    cursor.x - HALF,
                    cursor.y - HALF,
                    SRCCOPY | CAPTUREBLT,
                );
                ReleaseDC(null_mut(), screen);
            }
            for (index, pixel) in block.iter_mut().enumerate() {
                *pixel = *self.source_bits.add(index);
            }
        }
        block
    }

    /// Put the loupe next to the pointer, flipping it when it would leave the
    /// monitor's work area.
    fn place(&mut self, cursor: POINT) {
        let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            rcWork: RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            dwFlags: 0,
        };
        let work = if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            info.rcWork
        } else {
            RECT {
                left: 0,
                top: 0,
                right: 4096,
                bottom: 4096,
            }
        };

        let gap = 18 * self.scale;
        let mut x = cursor.x + gap;
        let mut y = cursor.y + gap;
        if x + self.width > work.right {
            x = cursor.x - gap - self.width;
        }
        if y + self.height > work.bottom {
            y = cursor.y - gap - self.height;
        }
        x = x.max(work.left);
        y = y.max(work.top);

        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                self.width,
                self.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        self.last_rect = RECT {
            left: x,
            top: y,
            right: x + self.width,
            bottom: y + self.height,
        };
        self.visible = true;
    }

    fn draw(&mut self, srgb: [u8; 4]) {
        unsafe {
            let panel = RECT {
                left: 0,
                top: 0,
                right: self.width,
                bottom: self.height,
            };
            FillRect(self.dc, &panel, self.panel);
            FrameRect(self.dc, &panel, self.border);

            SetStretchBltMode(self.dc, COLORONCOLOR);
            StretchBlt(
                self.dc,
                self.margin,
                self.margin,
                GRID * self.cell,
                GRID * self.cell,
                self.source,
                0,
                0,
                GRID,
                GRID,
                SRCCOPY,
            );

            // The sampled pixel: one cell outline, plus a white inner line so
            // the marker reads on both bright and dark pixels.
            let cell_x = self.margin + HALF * self.cell;
            let cell = RECT {
                left: cell_x,
                top: cell_x,
                right: cell_x + self.cell,
                bottom: cell_x + self.cell,
            };
            FrameRect(self.dc, &cell, self.accent);
            let inner = RECT {
                left: cell_x + 1,
                top: cell_x + 1,
                right: cell_x + self.cell - 1,
                bottom: cell_x + self.cell - 1,
            };
            FrameRect(self.dc, &inner, self.white);

            SetBkMode(self.dc, TRANSPARENT as i32);
            let old_font = SelectObject(self.dc, self.font as HGDIOBJ);
            SetTextColor(self.dc, rgb(232, 232, 232));
            let hex = format!("#{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]);
            let hex = wide(&hex);
            TextOutW(
                self.dc,
                self.margin,
                self.text_top,
                hex.as_ptr(),
                (hex.len() - 1) as i32,
            );
            SetTextColor(self.dc, rgb(148, 148, 148));
            let numbers = format!("{}  {}  {}", srgb[0], srgb[1], srgb[2]);
            let numbers = wide(&numbers);
            TextOutW(
                self.dc,
                self.margin,
                self.text_top + 14 * self.scale,
                numbers.as_ptr(),
                (numbers.len() - 1) as i32,
            );
            SelectObject(self.dc, old_font);

            // A solid chip of the sampled colour, so "what will I get" reads
            // without decoding the hex.
            let chip_size = 14 * self.scale;
            let chip = RECT {
                left: self.width - self.margin - chip_size,
                top: self.text_top,
                right: self.width - self.margin,
                bottom: self.text_top + chip_size,
            };
            let chip_brush = solid(srgb[0], srgb[1], srgb[2]);
            FillRect(self.dc, &chip, chip_brush);
            DeleteObject(chip_brush as HGDIOBJ);
            FrameRect(self.dc, &chip, self.border);

            let window = GetDC(self.hwnd);
            if !window.is_null() {
                BitBlt(
                    window,
                    0,
                    0,
                    self.width,
                    self.height,
                    self.dc,
                    0,
                    0,
                    SRCCOPY,
                );
                ReleaseDC(self.hwnd, window);
            }
        }
    }

    fn hide(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        self.visible = false;
        self.last_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.bitmap_old);
            DeleteObject(self.bitmap as HGDIOBJ);
            DeleteDC(self.dc);
            SelectObject(self.source, self.source_old);
            DeleteObject(self.source_bitmap as HGDIOBJ);
            DeleteDC(self.source);
            DeleteObject(self.font as HGDIOBJ);
            DeleteObject(self.panel as HGDIOBJ);
            DeleteObject(self.border as HGDIOBJ);
            DeleteObject(self.accent as HGDIOBJ);
            DeleteObject(self.white as HGDIOBJ);
            DestroyWindow(self.hwnd);
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn solid(red: u8, green: u8, blue: u8) -> HBRUSH {
    unsafe { CreateSolidBrush(rgb(red, green, blue)) }
}

fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | (green as u32) << 8 | (blue as u32) << 16
}

fn intersects(left: &RECT, right: &RECT) -> bool {
    left.left < right.right
        && right.left < left.right
        && left.top < right.bottom
        && right.top < left.bottom
}

static CLASS: Once = Once::new();
static CLASS_OK: AtomicBool = AtomicBool::new(false);
static CLASS_NAME_W: std::sync::OnceLock<Vec<u16>> = std::sync::OnceLock::new();

fn ensure_class() -> bool {
    CLASS.call_once(|| unsafe {
        let instance = GetModuleHandleW(null());
        let class_name = CLASS_NAME_W.get_or_init(|| wide(CLASS_NAME));
        let window_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };
        CLASS_OK.store(RegisterClassW(&window_class) != 0, Ordering::Relaxed);
    });
    CLASS_OK.load(Ordering::Relaxed)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            unsafe { ValidateRect(hwnd, null()) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
