//! Regenerate `Preferences/Styles/*.ron` from the code-authored style defaults.
//!
//! Run from anywhere:
//!
//! ```sh
//! cargo run -p kairos_editor_component --example export_styles
//! ```
//!
//! This is the "serialize the code default style" helper: change a `dark()` /
//! `light()` default, run this once, and the RON files under `Preferences/Styles`
//! are re-seeded. The editor itself only loads these files (and falls back to the
//! same code defaults when they are absent), so this tool never runs at startup.

use std::path::Path;

use kairos_editor_component::panels::ConsoleStyle;
use kairos_editor_widgets::fonts::FontsConfig;
use kairos_editor_widgets::style::StyleFile;

fn main() -> Result<(), String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "example must live inside the workspace".to_owned())?;
    let styles = workspace.join("Preferences/Styles");

    let console = styles.join("ConsoleStyle.ron");
    StyleFile::<ConsoleStyle>::defaults().save_to(&console)?;
    println!("wrote {}", console.display());

    let fonts = styles.join("EditorFonts.ron");
    FontsConfig::default().save_to(&fonts)?;
    println!("wrote {}", fonts.display());

    Ok(())
}
