//! Keeping the pointer on the icon egui asked for.
//!
//! egui publishes the cursor icon in `PlatformOutput`; forwarding it to the
//! window is the integration's job. egui-winit only forwards a **change**, and
//! the backends differ in whether that is enough:
//!
//! * Windows stores the icon and answers every `WM_SETCURSOR` with it, so the
//!   pointer keeps the icon while it rests;
//! * X11 sets the icon as the window's cursor attribute, and Wayland re-sends it
//!   when the pointer enters the surface, so both keep it too;
//! * macOS stores it in a view-wide cursor rect that AppKit only re-applies while
//!   the pointer keeps moving. In a window that repaints continuously a resting
//!   pointer falls back to the arrow, and the icon is visible only on the frame
//!   in which it changed (a pointer move, a click) -- which reads as "the cursor
//!   flashes for one frame".
//!
//! So only macOS needs the icon re-applied on every pass, which is what the
//! platform module below does. The switch `KAIROS_NATIVE_CURSOR=0` turns it off
//! for a side-by-side run against stock egui-winit.

/// Apply this pass's cursor icon to the window's pointer.
///
/// `inside` is whether the pointer is over the editor's content; while it is
/// over the title bar or the resize frame the window system owns the cursor.
pub fn apply(inside: bool, icon: egui::CursorIcon) {
    #[cfg(target_os = "macos")]
    macos::apply(inside, icon);
    #[cfg(not(target_os = "macos"))]
    let _ = (inside, icon);
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2_app_kit::NSCursor;

    /// Set AppKit's cursor for egui's icon. Icons the editor never shows are left
    /// to egui-winit's own (change-triggered) path.
    pub(super) fn apply(inside: bool, icon: egui::CursorIcon) {
        if !inside || !enabled() {
            return;
        }
        if let Some(cursor) = cursor(icon) {
            cursor.set();
        }
    }

    /// Whether the per-pass AppKit cursor runs. `KAIROS_NATIVE_CURSOR=0` turns it
    /// off so a run can be compared against stock egui-winit. Defaults to on.
    fn enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| {
            let enabled = std::env::var("KAIROS_NATIVE_CURSOR").map_or(true, |value| value != "0");
            kairos_log::info!(target: "kairos_editor", "native cursor: {enabled}");
            enabled
        })
    }

    fn cursor(icon: egui::CursorIcon) -> Option<Retained<NSCursor>> {
        use egui::CursorIcon as Icon;

        Some(match icon {
            Icon::Default => NSCursor::arrowCursor(),
            Icon::Text | Icon::VerticalText => NSCursor::IBeamCursor(),
            Icon::PointingHand => NSCursor::pointingHandCursor(),
            Icon::Crosshair => NSCursor::crosshairCursor(),
            Icon::Grab => NSCursor::openHandCursor(),
            Icon::Grabbing => NSCursor::closedHandCursor(),
            // Unity's scrub handle reads as a divider drag: the editor's only
            // left/right cursor. AppKit's dedicated resize cursors are deprecated
            // in favour of these direction-aware divider cursors.
            Icon::ResizeHorizontal => NSCursor::columnResizeCursor(),
            Icon::ResizeVertical => NSCursor::rowResizeCursor(),
            Icon::ContextMenu => NSCursor::contextualMenuCursor(),
            Icon::Copy => NSCursor::dragCopyCursor(),
            Icon::NotAllowed | Icon::NoDrop => NSCursor::operationNotAllowedCursor(),
            _ => return None,
        })
    }
}
