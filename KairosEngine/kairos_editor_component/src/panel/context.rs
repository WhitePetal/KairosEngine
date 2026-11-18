//! Construction-time context for panels, and the editor identity it carries.
//!
//! A panel object is built by [`PanelCatalog`](super::PanelCatalog) the first
//! time its kind opens, and the factory receives this context then. It is
//! deliberately not a second [`Frame`](crate::Frame): a frame is the read-only
//! per-frame snapshot a panel draws from, while this context carries only what
//! the composition root must inject while *constructing* the object. Every
//! field is constant for the process lifetime and no action can change it, so
//! it is not `EditorState`.

/// The editor's product identity, supplied by the composition root.
///
/// Plain data: no egui, no IO, no paths. The app (`kairos_editor_app`) owns
/// what the editor calls itself; this crate must not depend on it (cargo
/// enforces the layering), so the value travels in a [`PanelContext`] and the
/// panel that displays it keeps a copy. A version literal compiled into a
/// component is exactly how the legacy `kairos_editor` window ended up
/// disagreeing with its own title bar; see
/// `docs/plan/editor-ui-architecture/about-panel.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorInfo {
    pub name: &'static str,
    pub version: &'static str,
    /// One sentence describing the product; the About panel wraps it.
    pub description: &'static str,
    /// SPDX expression, e.g. `MIT OR Apache-2.0`.
    pub license: &'static str,
    /// Project home. Empty hides the row.
    pub repository: &'static str,
    /// Issue/discussion board. Empty hides the row.
    pub discussions: &'static str,
    /// Third-party acknowledgements page. Empty hides the row.
    pub credits: &'static str,
}

impl EditorInfo {
    /// Identity with no optional links, for tests and embedders that only know
    /// a name and a version.
    pub const fn new(name: &'static str, version: &'static str) -> Self {
        Self {
            name,
            version,
            description: "",
            license: "",
            repository: "",
            discussions: "",
            credits: "",
        }
    }

    /// One-line identity for the clipboard: what a bug report should quote.
    pub fn banner(&self) -> String {
        format!("{} {} ({})", self.name, self.version, Self::build_label())
    }

    /// Debug/Release and target of the running binary.
    ///
    /// Computed from this crate's own `cfg!`/`std::env::consts`: the component
    /// is compiled into the same process and profile as the app, so this is the
    /// process's property, not a second source of truth. A panel that shows it
    /// (About's "Build" row) asks here rather than recomputing the string.
    pub fn build_label() -> String {
        let profile = if cfg!(debug_assertions) {
            "Debug"
        } else {
            "Release"
        };
        format!(
            "{profile} · {}-{}",
            std::env::consts::ARCH,
            std::env::consts::OS
        )
    }
}

/// What a panel may know when it is constructed.
///
/// `PanelHost::sync` hands this to the factory of every panel it builds, and
/// that is the only moment panels are constructed: a panel that is never opened
/// never gets one. Add a field here when another composition-root value becomes
/// construction-time input; panels read it in their factory and keep only what
/// they need, so a factory captures nothing and stays a plain `fn`.
#[derive(Clone, Copy, Debug)]
pub struct PanelContext {
    /// The identity of the running editor.
    pub editor: EditorInfo,
}

impl PanelContext {
    pub const fn new(editor: EditorInfo) -> Self {
        Self { editor }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_identity_hides_every_optional_link() {
        let info = EditorInfo::new("KairosEngine", "0.2.0");
        assert_eq!(info.name, "KairosEngine");
        assert_eq!(info.version, "0.2.0");
        assert!(info.repository.is_empty());
        assert!(info.credits.is_empty());
    }

    #[test]
    fn the_banner_names_the_build_and_the_target() {
        let info = EditorInfo::new("KairosEngine", "0.2.0");
        let banner = info.banner();
        assert!(banner.starts_with("KairosEngine 0.2.0 ("), "{banner}");
        assert!(banner.contains(std::env::consts::ARCH), "{banner}");
        assert!(banner.contains(std::env::consts::OS), "{banner}");
    }
}
