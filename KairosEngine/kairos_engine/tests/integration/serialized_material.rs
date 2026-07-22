use std::path::{Path, PathBuf};

use kairos_engine::{
    asset_loader::assets::{
        AssetsServer, SerializedMaterialAssetsSystem,
    },
};
use tempfile::TempDir;

fn create_mat_toml(dir: &Path, name: &str, shader_path: &str) -> PathBuf {
    let path = dir.join(format!("{name}.mat"));
    let toml = format!(
        r#"
source_path = "{path}"
shader_path = "{shader_path}"

[render_state]
depth_test = "LessEqual"
depth_write = true
cull_mod = "Back"
topology = "TriangleList"
"#,
        path = path.display(),
        shader_path = shader_path
    );
    std::fs::write(&path, &toml).unwrap();
    path
}

// ============================================================
// SerializedMaterialAssetsSystem tests
// ============================================================

#[tokio::test]
async fn serialized_material_loads_from_mat_file() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "test_material", "res/shaders/default.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let loaded = assets.get::<SerializedMaterialAssetsSystem>(&handle);
    assert!(loaded.is_some(), "SerializedMaterial should be loaded");
    let material = loaded.unwrap();
    assert_eq!(
        material.shader_path.to_string_lossy(),
        "res/shaders/default.wgsl"
    );
    assert!(material.texture_path.is_none());
    assert_eq!(material.source_path, mat_path);
}

#[tokio::test]
async fn serialized_material_loads_with_texture() {
    let tmp = TempDir::new().unwrap();
    let mat_path = tmp.path().join("textured.mat");
    let toml = format!(
        r#"
source_path = "{path}"
shader_path = "res/shaders/default.wgsl"
texture_path = "res/textures/checker.texture"

[render_state]
depth_test = "LessEqual"
depth_write = true
cull_mod = "Back"
topology = "TriangleList"
"#,
        path = mat_path.display()
    );
    std::fs::write(&mat_path, &toml).unwrap();

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let loaded = assets.get::<SerializedMaterialAssetsSystem>(&handle);
    assert!(loaded.is_some());
    let material = loaded.unwrap();
    assert_eq!(
        material.shader_path.to_string_lossy(),
        "res/shaders/default.wgsl"
    );
    assert_eq!(
        material.texture_path.as_ref().unwrap().to_string_lossy(),
        "res/textures/checker.texture"
    );
}

#[tokio::test]
async fn serialized_material_loads_multiple_independent() {
    let tmp = TempDir::new().unwrap();
    let mat_a = create_mat_toml(tmp.path(), "material_a", "shaders/a.wgsl");
    let mat_b = create_mat_toml(tmp.path(), "material_b", "shaders/b.wgsl");

    let mut assets = AssetsServer::new();
    let handle_a = assets.load::<SerializedMaterialAssetsSystem>(&mat_a);
    let handle_b = assets.load::<SerializedMaterialAssetsSystem>(&mat_b);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let loaded_a = assets
        .get::<SerializedMaterialAssetsSystem>(&handle_a)
        .unwrap();
    let loaded_b = assets
        .get::<SerializedMaterialAssetsSystem>(&handle_b)
        .unwrap();
    assert_eq!(loaded_a.shader_path.to_string_lossy(), "shaders/a.wgsl");
    assert_eq!(loaded_b.shader_path.to_string_lossy(), "shaders/b.wgsl");
}

#[tokio::test]
async fn serialized_material_nonexistent_file_returns_loading_handle() {
    let mut assets = AssetsServer::new();
    let fake_path = Path::new("./nonexistent.mat").to_path_buf();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&fake_path);

    // Before handle(), the asset should still be Loading (not yet available)
    assert!(assets.get::<SerializedMaterialAssetsSystem>(&handle).is_none());

    // After handle(), the load task will have failed, but the slot stays in Loading
    // (the asset system doesn't clear failed loads — that's acceptable behavior)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();
}
