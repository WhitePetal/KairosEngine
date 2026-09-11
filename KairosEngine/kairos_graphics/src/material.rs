use std::path::PathBuf;

use kairos_asset::next::{
    Asset, AssetLoader, AssetWorldExt, Handle, LoadContext, Reader, UntypedAssetId,
    VisitAssetDependencies,
};
use kairos_ecs::error::KairosError;
use kairos_ecs::world::World;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::{
    consts::{MATERIAL_ASSETS_CAPACITY, SERIALIZED_MATERIAL_ASSETS_CAPACITY},
    render_state::RenderState,
    shader::ShaderAsset,
    texture::Texture,
};

#[cfg(test)]
mod test;

mod serialize;

/// The on-disk form of a `.mat` file: the shader and texture it names by path,
/// plus the fixed-function render state.
///
/// This stays a separate asset from [`Material`] because the two are different
/// things: this is the editable, persistable document (paths and state), while
/// [`Material`] is the resolved runtime value (handles and state). The loader
/// turns one into the other; the material inspector edits this one and saves it
/// back to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedMaterial {
    pub source_path: PathBuf,
    pub shader_path: PathBuf,
    pub render_state: RenderState,
    pub texture_path: Option<PathBuf>,
}

/// The resolved runtime material the render pipeline draws with: the shader and
/// texture handles are strong handles into the next-generation asset stores, so
/// a live [`Material`] keeps its dependencies loaded.
#[derive(Debug, Clone)]
pub struct Material {
    pub shader: Option<Handle<ShaderAsset>>,
    pub render_state: RenderState,
    pub texture: Option<Handle<Texture>>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            shader: None,
            render_state: RenderState::default(),
            texture: None,
        }
    }
}

impl Asset for Material {}
impl VisitAssetDependencies for Material {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.shader.visit_dependencies(visit);
        self.texture.visit_dependencies(visit);
    }
}

/// Loads a `.mat` document and resolves the shader and texture it names into
/// handles, declaring both as dependencies through [`LoadContext::load`].
#[derive(Debug)]
pub struct MaterialLoader;

impl AssetLoader for MaterialLoader {
    type Asset = Material;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Material, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let serialized: SerializedMaterial = toml::from_slice(&bytes)?;

            let shader = load_context.load::<ShaderAsset>(serialized.shader_path.clone());
            let texture = serialized
                .texture_path
                .as_ref()
                .map(|path| load_context.load::<Texture>(path.clone()));

            Ok(Material {
                shader: Some(shader),
                render_state: serialized.render_state,
                texture,
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["mat"]
    }
}

impl Asset for SerializedMaterial {}
impl VisitAssetDependencies for SerializedMaterial {}

/// Reads a `.mat` file into its editable [`SerializedMaterial`] form, without
/// resolving the shader or texture.
#[derive(Debug)]
pub struct SerializedMaterialLoader;

impl AssetLoader for SerializedMaterialLoader {
    type Asset = SerializedMaterial;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<SerializedMaterial, KairosError>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let mut serialized: SerializedMaterial = toml::from_slice(&bytes)?;
            // The `.mat` file is the source of truth for this field, but a file
            // need not carry it; fall back to the path the asset server knows.
            if serialized.source_path.as_os_str().is_empty() {
                serialized.source_path = load_context.path().path().to_path_buf();
            }
            Ok(serialized)
        }
    }

    /// This loader is resolved by asset type, never by extension: the `.mat`
    /// extension is claimed by [`MaterialLoader`], and both are always loaded
    /// with an explicit asset type.
    fn extensions(&self) -> &[&str] {
        &[]
    }
}

/// Registers the [`Material`] and [`SerializedMaterial`] assets and their
/// loaders with the core.
///
/// Must run after [`kairos_asset::next::install`], which creates the
/// `AssetServer` and the `AssetStages` this reads.
pub fn install(world: &mut World) {
    world.init_asset_with_capacity::<Material>(MATERIAL_ASSETS_CAPACITY);
    world.init_asset_with_capacity::<SerializedMaterial>(SERIALIZED_MATERIAL_ASSETS_CAPACITY);
    world.register_asset_loader(MaterialLoader);
    world.register_asset_loader(SerializedMaterialLoader);
}
