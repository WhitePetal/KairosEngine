use std::path::{Path, PathBuf};
use std::sync::Arc;

use kairos_engine::asset_loader::assets::{AssetsServer, SerializedMaterialAssetsSystem};
use kairos_engine::graphics::compare_function::CompareFunction;
use kairos_engine::graphics::render_state::{CullMode, PrimitiveTopology, RenderState};
use kairos_engine::kairos_editor::ui::inspector::material::MaterialInspector;
use parking_lot::Mutex;
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

type SharedShaderPath = Arc<Mutex<Option<PathBuf>>>;
type SharedTexturePath = Arc<Mutex<Option<Option<PathBuf>>>>;
type SharedRenderState = Arc<Mutex<Option<RenderState>>>;

fn loaded_state() -> (SharedShaderPath, SharedTexturePath, SharedRenderState) {
    (
        Arc::new(Mutex::new(Some(PathBuf::from("res/shaders/new.wgsl")))),
        Arc::new(Mutex::new(Some(Some(PathBuf::from(
            "res/textures/new.texture",
        ))))),
        Arc::new(Mutex::new(Some(RenderState {
            depth_test: Some(CompareFunction::Less),
            depth_write: false,
            cull_mod: CullMode::Front,
            blend_mod: None,
            topology: PrimitiveTopology::TriangleStrip,
        }))),
    )
}

// ============================================================
// MaterialInspector::save_material (issue #36 — Apply 持久化)
// ============================================================

/// Apply 将当前编辑状态写回 .mat 文件。
/// 对应验收条件：Apply → 验证 shader_path/texture_path/render_state 字段写回磁盘。
#[tokio::test]
async fn save_material_writes_current_state_to_mat_file() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "material", "res/shaders/old.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();
    assert!(
        assets
            .get::<SerializedMaterialAssetsSystem>(&handle)
            .is_some(),
        "SerializedMaterial should be loaded"
    );

    let (shader_path, texture_path, render_state) = loaded_state();
    let saved = MaterialInspector::save_material(
        &mut assets,
        &mat_path,
        &handle,
        &shader_path,
        &texture_path,
        &render_state,
    );
    assert!(saved, "save_material should report success");

    // toml_value_equals 等价验证：磁盘文件的字段已更新
    let content = std::fs::read_to_string(&mat_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert_eq!(
        value["shader_path"].as_str().unwrap(),
        "res/shaders/new.wgsl"
    );
    assert_eq!(
        value["texture_path"].as_str().unwrap(),
        "res/textures/new.texture"
    );
    assert_eq!(value["render_state"]["cull_mod"].as_str().unwrap(), "Front");
    assert_eq!(
        value["render_state"]["topology"].as_str().unwrap(),
        "TriangleStrip"
    );
    assert_eq!(
        value["render_state"]["depth_test"].as_str().unwrap(),
        "Less"
    );
    assert!(!value["render_state"]["depth_write"].as_bool().unwrap());
}

/// 保存成功后，资产系统缓存的 SerializedMaterial 与磁盘保持一致，
/// 避免重新打开 Inspector 时读到过期数据。
#[tokio::test]
async fn save_material_updates_cached_serialized_material() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "material", "res/shaders/old.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let (shader_path, texture_path, render_state) = loaded_state();
    let saved = MaterialInspector::save_material(
        &mut assets,
        &mat_path,
        &handle,
        &shader_path,
        &texture_path,
        &render_state,
    );
    assert!(saved);

    let cached = assets
        .get::<SerializedMaterialAssetsSystem>(&handle)
        .expect("cached SerializedMaterial should exist");
    assert_eq!(cached.shader_path, PathBuf::from("res/shaders/new.wgsl"));
    assert_eq!(
        cached.texture_path,
        Some(PathBuf::from("res/textures/new.texture"))
    );
    assert_eq!(cached.render_state.cull_mod, CullMode::Front);
    assert_eq!(cached.render_state.topology, PrimitiveTopology::TriangleStrip);
    assert_eq!(cached.render_state.depth_test, Some(CompareFunction::Less));
    assert!(!cached.render_state.depth_write);
}

/// 清除纹理（texture_path = None）后 Apply，磁盘文件不应再有 texture_path 字段。
#[tokio::test]
async fn save_material_writes_empty_texture_slot() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "material", "res/shaders/old.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let shader_path: SharedShaderPath =
        Arc::new(Mutex::new(Some(PathBuf::from("res/shaders/new.wgsl"))));
    let texture_path: SharedTexturePath = Arc::new(Mutex::new(Some(None)));
    let render_state: SharedRenderState = Arc::new(Mutex::new(Some(RenderState::default())));

    let saved = MaterialInspector::save_material(
        &mut assets,
        &mat_path,
        &handle,
        &shader_path,
        &texture_path,
        &render_state,
    );
    assert!(saved);

    let content = std::fs::read_to_string(&mat_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert!(
        value.get("texture_path").is_none(),
        "cleared texture should not be written, got: {content}"
    );
}

/// 保存失败（目标目录不存在）时不崩溃、返回 false，调用方据此保留 dirty。
#[tokio::test]
async fn save_material_returns_false_on_write_failure() {
    let tmp = TempDir::new().unwrap();
    let bad_path = tmp.path().join("missing_dir").join("material.mat");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&bad_path);

    let (shader_path, texture_path, render_state) = loaded_state();
    let saved = MaterialInspector::save_material(
        &mut assets,
        &bad_path,
        &handle,
        &shader_path,
        &texture_path,
        &render_state,
    );
    assert!(!saved, "write failure should be reported as false");
    assert!(!bad_path.exists(), "no file should be created on failure");
}

/// 编辑状态尚未从资产系统加载完成时不写盘、返回 false。
#[tokio::test]
async fn save_material_returns_false_when_state_unloaded() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "material", "res/shaders/old.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let shader_path: SharedShaderPath = Arc::new(Mutex::new(None));
    let texture_path: SharedTexturePath = Arc::new(Mutex::new(None));
    let render_state: SharedRenderState = Arc::new(Mutex::new(None));

    let saved = MaterialInspector::save_material(
        &mut assets,
        &mat_path,
        &handle,
        &shader_path,
        &texture_path,
        &render_state,
    );
    assert!(!saved);

    // 磁盘文件保持原样
    let content = std::fs::read_to_string(&mat_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert_eq!(
        value["shader_path"].as_str().unwrap(),
        "res/shaders/old.wgsl"
    );
}
