use std::path::Path;

use kairos_engine::asset_loader::assets::{AssetsServer, SerializedMaterialAssetsSystem};

// ============================================================
// SerializedMaterialAssetsSystem tests
// ============================================================

#[tokio::test]
async fn serialized_material_nonexistent_file_returns_loading_handle() {
    let mut assets = AssetsServer::new();
    let fake_path = Path::new("./nonexistent.mat").to_path_buf();
    let handle = assets.load::<SerializedMaterialAssetsSystem>(&fake_path);

    // Before handle(), the asset should still be Loading (not yet available)
    assert!(
        assets
            .get::<SerializedMaterialAssetsSystem>(&handle)
            .is_none()
    );

    // After handle(), the load task will have failed, but the slot stays in Loading
    // (the asset system doesn't clear failed loads — that's acceptable behavior)
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assets.handle();
}
