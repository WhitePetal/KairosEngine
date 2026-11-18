//! File-backed UI images (icons, previews) for the editor chrome.
//!
//! Unlike `include_bytes!` assets, these files live under `Preferences/Textures`
//! and are loaded at runtime, so they can be replaced without rebuilding the
//! editor and they do not bloat the binary. Byte loading and texture caching are
//! delegated to egui's image-loader stack (`egui_extras::install_image_loaders`,
//! installed once by the host at startup).
//!
//! `preload` is called from a component's `Panel::init` and starts the
//! asynchronous load; `paint` draws the cached texture. While an image is still
//! pending, `paint` draws nothing: dense editor chrome should not show loading
//! spinners inside rows, and egui requests a repaint when the load finishes.
//!
//! Preloading is not required for correctness: `egui::Image` / `ui.image`
//! would also start the load lazily on first paint. It is the lifecycle choice
//! from ADR 0010: a panel starts reading its own resources in `init`, before
//! the first draw, so opening the panel does not spend its first frames with
//! missing chrome icons.
//!
//! Editor UI resources are deliberately independent of `kairos_asset`: they are
//! not project assets, have no import/meta/processing step, and must be
//! available before the engine asset server exists. Project asset previews (once
//! the editor is hosted by the engine) are a separate path through asset handles.

use egui::{Color32, Context, Rect, Ui};

/// Convert an editor-relative image path into an absolute `file://` URI.
///
/// `egui_extras::FileLoader` resolves relative `file://` paths against the
/// process working directory, but on Windows `file://rel/path` is parsed as a
/// UNC path. Converting to an absolute path first keeps both platforms on the
/// same code path (the same fix the legacy editor used).
pub fn file_uri(relative: &str) -> String {
    let path = std::path::Path::new(relative);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    };

    #[cfg(target_os = "windows")]
    {
        format!(
            "file:///{}",
            absolute.display().to_string().replace('\\', "/")
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("file://{}", absolute.display())
    }
}

/// Start the asynchronous load of an image, e.g. from `Panel::init`. The next
/// frames paint it once egui's loader finishes.
///
/// This only warms the loader and texture cache for [`paint`]'s URI; it is an
/// optimization/lifecycle hook, not a prerequisite for drawing (a lazy
/// `egui::Image` would load the same URI on its first paint).
pub fn preload(ctx: &Context, relative: &str) {
    let _ = egui::Image::new(egui::ImageSource::Uri(file_uri(relative).into()))
        .load_for_size(ctx, egui::vec2(32.0, 32.0));
}

pub fn get_image<'a>(relative: &str) -> egui::Image<'a> {
    egui::Image::new(egui::ImageSource::Uri(file_uri(relative).into()))
}

/// Paint an image at `rect`. Missing or still-loading images paint nothing.
pub fn paint(ui: &Ui, rect: Rect, relative: &str, tint: Color32) {
    get_image(relative)
        .tint(tint)
        .show_loading_spinner(false)
        .paint_at(ui, rect);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_become_absolute_file_uris() {
        let uri = file_uri("Preferences/Textures/console/info.png");
        assert!(uri.starts_with("file://"), "{uri}");
        assert!(
            uri.ends_with("Preferences/Textures/console/info.png"),
            "{uri}"
        );

        let path = uri.strip_prefix("file://").expect("file uri");
        // Windows keeps its drive after the scheme: `file:///D:/...` strips to
        // `/D:/...`, which is rooted but not absolute (`Path::is_absolute` needs
        // the drive prefix). Drop the slash the URI adds, not the drive.
        #[cfg(windows)]
        let path = path.trim_start_matches('/');
        assert!(std::path::Path::new(path).is_absolute(), "{uri}");
    }
}
