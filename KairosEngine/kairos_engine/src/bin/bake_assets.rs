//! Regenerates the processed assets under `imported_assets/Default`.
//!
//! Run it from the workspace root, where the default asset source is rooted
//! (ADR 0004) and where the editor's host runs:
//!
//! ```text
//! cargo run --bin bake_assets
//! ```
//!
//! The runtime host reads those assets by path, so the tree this writes is what
//! the editor loads `.glb` / `.png` sources from.

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("kairos_engine=info,warn"),
    )
    .init();

    kairos_engine::asset_pipeline::bake_assets();
}
