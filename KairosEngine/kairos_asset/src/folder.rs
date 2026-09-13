//! The [`LoadedFolder`] asset: the handles a recursive folder load produces.
//!
//! This mirrors `bevy_asset`'s `folder` module. A folder is itself an asset, so
//! it is tracked and reloaded like any other; the difference is that its value
//! is the set of handles to the assets it discovered.

use crate::asset::{Asset, VisitAssetDependencies};
use crate::handle::UntypedHandle;
use crate::id::UntypedAssetId;

/// A "loaded folder" containing handles for all assets stored in a folder.
///
/// Produced by [`AssetServer::load_folder`](crate::AssetServer::load_folder)
/// and inserted into its [`Assets`](crate::Assets) store once the walk
/// finishes. Because the handles it holds are its dependencies, the folder's
/// [`RecursiveDependencyLoadState`](crate::RecursiveDependencyLoadState)
/// reports whether every asset it discovered has loaded too.
pub struct LoadedFolder {
    /// The handles of all assets stored in the folder.
    pub handles: Vec<UntypedHandle>,
}

impl Asset for LoadedFolder {}

impl VisitAssetDependencies for LoadedFolder {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.handles.visit_dependencies(visit);
    }
}
