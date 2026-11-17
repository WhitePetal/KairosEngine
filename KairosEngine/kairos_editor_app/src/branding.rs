//! The app's desktop branding: the icon the OS shows for the app itself.
//!
//! The icon in [`RuntimeConfig`](kairos_engine::runtime::RuntimeConfig) is
//! winit's *window* icon. Two desktops do not use it for the tile the user
//! recognizes the app by:
//!
//! * macOS ignores per-window icons outright (`set_window_icon` is a no-op).
//!   The Dock, Cmd-Tab and Force Quit show the *application* icon, which only
//!   AppKit can replace.
//! * Windows draws the taskbar button and Alt-Tab entry from `ICON_BIG`, which
//!   winit sets through `WindowAttributesExtWindows::with_taskbar_icon`, never
//!   from the window icon.
//!
//! [`brand_app`] closes both gaps from the same packaged asset. It runs once,
//! from the layer's `window_created` — on the event-loop thread, which is the
//! main thread AppKit demands.

use winit::window::{Icon, Window};

use crate::paths::PATH_ENGINE_ICON;

/// Loads the packaged engine icon as a winit window icon.
///
/// This is the icon winit puts on the window itself; [`brand_app`] covers the
/// desktop surfaces winit cannot reach. A missing or malformed asset is logged
/// and leaves the platform's default in place: an unbranded window is not a
/// reason to fail the boot.
pub(crate) fn load_icon() -> Option<Icon> {
    fn load() -> Result<Icon, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(PATH_ENGINE_ICON)?;
        let image = image::load_from_memory(&bytes)?.into_rgba8();
        let (width, height) = image.dimensions();

        Ok(Icon::from_rgba(image.into_raw(), width, height)?)
    }

    match load() {
        Ok(icon) => Some(icon),
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "cannot load the app icon {PATH_ENGINE_ICON}: {error}"
            );
            None
        }
    }
}

/// Gives the desktop the app's own icon; see the [module docs](self) for why
/// the window icon is not enough.
///
/// macOS: the Dock (and Cmd-Tab, and Force Quit) — the window is not involved,
/// because AppKit has no per-window icon.
#[cfg(target_os = "macos")]
pub(crate) fn brand_app(_window: &Window) {
    macos::set_dock_icon(PATH_ENGINE_ICON);
}

/// Gives the desktop the app's own icon; see the [module docs](self) for why
/// the window icon is not enough.
///
/// Windows: the taskbar's big icon. The small one (title bar, window menu) is
/// already set from `RuntimeConfig::icon` at window creation.
#[cfg(target_os = "windows")]
pub(crate) fn brand_app(window: &Window) {
    use winit::platform::windows::WindowExtWindows;

    window.set_taskbar_icon(load_icon());
}

/// Gives the desktop the app's own icon; see the [module docs](self) for why
/// the window icon is not enough.
///
/// X11 takes the window icon from `RuntimeConfig::icon` and draws the
/// application tile from it; Wayland has no window or application icon to set.
/// Either way there is nothing to do here.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) fn brand_app(_window: &Window) {}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::Path;

    use objc2::{AnyThread as _, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSString;

    /// Points the Dock tile — and with it Cmd-Tab and Force Quit — at `path`.
    ///
    /// AppKit resolves and loads the file itself, so the image keeps its own
    /// representations instead of a bitmap winit already flattened.
    pub(super) fn set_dock_icon(path: impl AsRef<Path>) {
        let Some(mtm) = MainThreadMarker::new() else {
            kairos_log::warn!(
                target: "kairos_editor",
                "the Dock icon can only be set from the main thread"
            );
            return;
        };

        // Absolute, so the warning below names the directory the cwd-relative
        // asset was looked up in: a bare relative path does not explain a miss.
        let path = path.as_ref();
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let ns_path = NSString::from_str(&path.to_string_lossy());

        let Some(image) = NSImage::initWithContentsOfFile(NSImage::alloc(), &ns_path) else {
            kairos_log::warn!(
                target: "kairos_editor",
                "cannot load the Dock icon from {}",
                path.display()
            );
            return;
        };

        let app = NSApplication::sharedApplication(mtm);

        // SAFETY: `sharedApplication` returns the process's application object,
        // which AppKit owns and keeps alive; `image` is a valid `NSImage`.
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}
