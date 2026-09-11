//! The engine-side asset systems (text, font, syntax, toml) plus the
//! re-exported generic asset machinery, so paths like
//! `crate::asset_loader::assets::asset::AssetIndex` keep resolving.
//!
//! The graphics asset systems that used to live here (`mesh`, `material`,
//! `shader`, `texture`, `serialized_material`) moved to `kairos_graphics`, and
//! the audio systems (`AudioAsset`, `PcmData`, `AudioExt`) moved to the
//! next-generation core in `crate::audio`.

mod font;
mod syntax;
mod text;
mod toml;

pub use kairos_asset::assets::asset::*;

pub use font::FontAssetsSystem;
pub use syntax::SyntaxAssetsSystem;
pub use text::TextAssetsSystem;
pub use toml::TomlTableAssetsSystem;
