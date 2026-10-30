//! Icon identity + a pluggable provider.
//!
//! The widget kit must not know about image loading, GPU textures or asset
//! paths, so it only names an `IconId`. The host supplies an `IconProvider`
//! that turns ids into glyphs (prototype) or painted textures (production).

/// Theme-independent identity of an icon. The set is intentionally tiny and
/// fully enumerable: a widget kit that can name arbitrary icons tends to grow
/// asset-loader coupling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconId {
    Folder,
    FolderOpen,
    File,
    Scene,
    Material,
    Texture,
    Script,
    Search,
    Plus,
    Trash,
    Save,
    Revert,
    Close,
    ChevronRight,
    ChevronDown,
    Settings,
    Check,
    Warning,
    Info,
    Dot,
}

/// Resolves `IconId`s to something paintable.
pub trait IconProvider: Send + Sync {
    /// Unicode glyph used as a placeholder icon. Production providers would
    /// paint a cached texture here instead of returning a glyph.
    fn glyph(&self, id: IconId) -> &'static str;
}

/// Prototype provider: no image loading at all, just glyphs that the bundled
/// egui fonts can rasterize.
pub struct GlyphIcons;

impl IconProvider for GlyphIcons {
    fn glyph(&self, id: IconId) -> &'static str {
        match id {
            IconId::Folder => "📁",
            IconId::FolderOpen => "📂",
            IconId::File => "📄",
            IconId::Scene => "🎬",
            IconId::Material => "🎨",
            IconId::Texture => "🖼",
            IconId::Script => "📜",
            IconId::Search => "🔍",
            IconId::Plus => "＋",
            IconId::Trash => "🗑",
            IconId::Save => "💾",
            IconId::Revert => "↺",
            IconId::Close => "✕",
            IconId::ChevronRight => "▸",
            IconId::ChevronDown => "▾",
            IconId::Settings => "⚙",
            IconId::Check => "✓",
            IconId::Warning => "⚠",
            IconId::Info => "ℹ",
            IconId::Dot => "·",
        }
    }
}

/// Provider used when the host installs no custom one.
pub static FALLBACK_ICONS: GlyphIcons = GlyphIcons;
