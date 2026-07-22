use std::path::Path;

use kairos_engine::kairos_editor::ui::inspector::material::resolve_texture_load_path;
use tempfile::TempDir;

// ============================================================
// resolve_texture_load_path (issue #34 — texture fallback)
// ============================================================

#[test]
fn existing_texture_path_is_used_as_is() {
    let tmp = TempDir::new().unwrap();
    let texture = tmp.path().join("real.texture");
    std::fs::write(&texture, b"data").unwrap();
    let fallback = Path::new("res/textures/white.texture");

    assert_eq!(resolve_texture_load_path(&texture, fallback), texture);
}

#[test]
fn missing_texture_path_falls_back_to_white_texture() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("missing.texture");
    let fallback = Path::new("res/textures/white.texture");

    assert_eq!(resolve_texture_load_path(&missing, fallback), fallback);
}

#[test]
fn missing_fallback_path_does_not_recurse() {
    let tmp = TempDir::new().unwrap();
    // Both the assigned path and the fallback are missing: the fallback is
    // returned unchanged. The mapping is single-pass — no recursive
    // degradation, no panic, no infinite loop.
    let missing = tmp.path().join("missing.texture");
    let missing_fallback = tmp.path().join("white.texture");

    assert_eq!(
        resolve_texture_load_path(&missing, &missing_fallback),
        missing_fallback
    );
}
