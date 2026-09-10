//! The engine-side asset systems (audio, text, font, syntax, toml) plus the
//! re-exported generic asset machinery, so paths like
//! `crate::asset_loader::assets::asset::AssetIndex` keep resolving.
//!
//! The graphics asset systems that used to live here (`mesh`, `material`,
//! `shader`, `texture`, `serialized_material`) moved to `kairos_graphics`.

mod audio;
mod font;
mod syntax;
mod text;
mod toml;

pub use kairos_asset::assets::asset::*;

pub use audio::{AudioAssetHandle, AudioAssetsSystem, AudioExtAssetsSystem, PcmAssetsSystem};
pub use font::FontAssetsSystem;
pub use syntax::SyntaxAssetsSystem;
pub use text::TextAssetsSystem;
pub use toml::TomlTableAssetsSystem;
