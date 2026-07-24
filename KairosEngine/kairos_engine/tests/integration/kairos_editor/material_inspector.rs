use std::path::{Path, PathBuf};
use std::sync::Arc;

use kairos_engine::asset_loader::assets::{
    AssetsServer, MaterialAssetsSystem, SerializedMaterialAssetsSystem,
};
use kairos_engine::graphics::compare_function::CompareFunction;
use kairos_engine::graphics::material::SerializedMaterial;
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

type SharedSerializedMaterial = Arc<Mutex<Option<SerializedMaterial>>>;

/// 用户编辑后的目标状态（与 create_mat_toml 写入磁盘的初始状态不同）。
fn edited_material(source_path: &Path) -> SerializedMaterial {
    SerializedMaterial {
        source_path: source_path.to_path_buf(),
        shader_path: PathBuf::from("res/shaders/new.wgsl"),
        render_state: RenderState {
            depth_test: Some(CompareFunction::Less),
            depth_write: false,
            cull_mod: CullMode::Front,
            blend_mod: None,
            topology: PrimitiveTopology::TriangleStrip,
        },
        texture_path: Some(PathBuf::from("res/textures/new.texture")),
    }
}

fn shared_state(serialized: SerializedMaterial) -> SharedSerializedMaterial {
    Arc::new(Mutex::new(Some(serialized)))
}

// ============================================================
// MaterialInspector::save_material (issue #36 — Apply 持久化)
// ============================================================

/// 清除纹理（texture_path = None）后 Apply，磁盘文件不应再有 texture_path 字段。
#[tokio::test]
async fn save_material_writes_empty_texture_slot() {
    let tmp = TempDir::new().unwrap();
    let mat_path = create_mat_toml(tmp.path(), "material", "res/shaders/old.wgsl");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    let mut edited = edited_material(&mat_path);
    edited.texture_path = None;
    let shared = shared_state(edited);
    MaterialInspector::save_material(&mut assets, &handle, &shared);

    let content = std::fs::read_to_string(&mat_path).unwrap();
    let value: toml::Value = toml::from_str(&content).unwrap();
    assert!(
        value.get("texture_path").is_none(),
        "cleared texture should not be written, got: {content}"
    );
}

/// 保存失败（目标目录不存在）时不崩溃、不创建文件，调用方据此保留 dirty。
#[tokio::test]
async fn save_material_does_not_create_file_on_write_failure() {
    let tmp = TempDir::new().unwrap();
    let bad_path = tmp.path().join("missing_dir").join("material.mat");

    let mut assets = AssetsServer::new();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&bad_path);

    let shared = shared_state(edited_material(&bad_path));
    MaterialInspector::save_material(&mut assets, &handle, &shared);

    assert!(!bad_path.exists(), "no file should be created on failure");
}

// ============================================================
// MaterialInspector::discard_changes（Discard 还原 bugfix）
// ============================================================

/// 持久化状态未加载（.mat 读取失败）时 Discard 不崩溃、不改动任何状态。
#[tokio::test]
async fn discard_changes_no_crash_when_serialized_unloaded() {
    let tmp = TempDir::new().unwrap();
    let mat_path = tmp.path().join("missing.mat");

    let mut assets = AssetsServer::new();
    let serialized_handle = assets.load::<SerializedMaterialAssetsSystem>(&mat_path);
    let material_handle = assets.load::<MaterialAssetsSystem>(&mat_path);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();

    assert!(
        assets
            .get::<SerializedMaterialAssetsSystem>(&serialized_handle)
            .is_none(),
        "serialized should stay unloaded for a missing file"
    );
    MaterialInspector::discard_changes(&mut assets, &serialized_handle, &material_handle);
}

// ============================================================
// 3D 预览 Style TOML 配置（issue #37）
// ============================================================

fn workspace_root() -> &'static Path {
    // CARGO_MANIFEST_DIR = kairos_engine/，Preferences/ 在工作区根（上一级）
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
}

fn material_style_toml() -> toml::Value {
    let path = workspace_root().join("Preferences/Styles/Inspectors/Material.toml");
    let content = std::fs::read_to_string(&path).expect("read Material.toml style");
    toml::from_str(&content).expect("parse Material.toml style")
}

/// 验收条件：预览网格下拉栏中的每个配置路径都必须真实存在，
/// 否则下拉栏会给用户一个永远加载不出来的选项。
#[test]
fn material_inspector_preview_meshes_exist_on_disk() {
    let root = workspace_root();
    let value = material_style_toml();
    let meshes = value["preview_meshes"]
        .as_array()
        .expect("preview_meshes must be a list");
    for mesh in meshes {
        let mesh = mesh
            .as_str()
            .expect("preview_meshes entries must be strings");
        assert!(root.join(mesh).exists(), "preview mesh not found: {mesh}");
    }
}
