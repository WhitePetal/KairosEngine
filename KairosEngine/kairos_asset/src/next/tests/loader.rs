//! Tests for the loader face: [`AssetLoader`], [`LoadContext`], dependencies,
//! labeled sub-assets, and load state.

use std::{path::Path, sync::Arc};

use futures_lite::future::block_on;
use kairos_tasks::ConditionalSendFuture;

use crate::next::io::{Reader, VecReader};
use crate::next::path::AssetPath;
use crate::next::{
    AssetLoadError, AssetLoader, AssetServer, DependencyLoadState, ErasedAssetLoader,
    ErasedLoadedAsset, Handle, LoadContext, LoadState, LoadedAsset,
    RecursiveDependencyLoadState, UntypedAssetId, VisitAssetDependencies,
};

/// An asset whose value is the bytes it was loaded from.
#[derive(Debug, PartialEq, Eq)]
struct ByteAsset(Vec<u8>);

impl crate::next::Asset for ByteAsset {}
impl VisitAssetDependencies for ByteAsset {}

/// A different asset type, used to prove labeled assets may differ from the
/// root.
#[derive(Debug, PartialEq, Eq)]
struct OtherAsset(u32);

impl crate::next::Asset for OtherAsset {}
impl VisitAssetDependencies for OtherAsset {}

/// A value that embeds a dependency its loader never declared.
struct EmbeddingAsset {
    inner: Handle<ByteAsset>,
}

impl crate::next::Asset for EmbeddingAsset {}
impl VisitAssetDependencies for EmbeddingAsset {
    fn visit_dependencies(&self, visit: &mut impl FnMut(UntypedAssetId)) {
        self.inner.visit_dependencies(visit);
    }
}

/// A loader that returns the reader's bytes.
struct ByteLoader;

impl AssetLoader for ByteLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(ByteAsset(bytes))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["bytes"]
    }
}

/// A loader that declares a dependency through its context before returning.
struct DeclaringLoader;

impl AssetLoader for DeclaringLoader {
    type Asset = ByteAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<ByteAsset, std::io::Error>> {
        let _dependency: Handle<ByteAsset> = load_context.load("dep.bytes");
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(ByteAsset(bytes))
        }
    }
}

fn server() -> AssetServer {
    let server = AssetServer::new();
    let mut infos = server.write_infos();
    infos.register_handle_provider::<ByteAsset>();
    infos.register_handle_provider::<OtherAsset>();
    drop(infos);
    server
}

fn context<'a>(server: &'a AssetServer, path: &'static str) -> LoadContext<'a> {
    LoadContext::new(server, AssetPath::from(path), true, false)
}

fn load_bytes(loader: &dyn ErasedAssetLoader, server: &AssetServer, bytes: &[u8]) -> ErasedLoadedAsset {
    let mut reader = VecReader::new(bytes.to_vec());
    let settings = ();
    let context = context(server, "asset.bytes");
    block_on(loader.load(&mut reader, &settings, context)).expect("the load succeeds")
}

#[test]
fn loader_produces_its_asset() {
    let server = server();
    let loaded = load_bytes(&ByteLoader, &server, b"hello");
    assert_eq!(
        loaded.take::<ByteAsset>(),
        Some(ByteAsset(b"hello".to_vec()))
    );
}

#[test]
fn loader_extensions_and_erased_type_metadata() {
    let loader = ByteLoader;
    let erased: &dyn ErasedAssetLoader = &loader;
    assert_eq!(erased.extensions(), &["bytes"]);
    assert_eq!(erased.type_name(), core::any::type_name::<ByteLoader>());
    assert_eq!(erased.type_id(), core::any::TypeId::of::<ByteLoader>());
    assert_eq!(erased.asset_type_name(), core::any::type_name::<ByteAsset>());
    assert_eq!(
        erased.asset_type_id(),
        core::any::TypeId::of::<ByteAsset>()
    );
}

#[test]
fn declared_dependency_is_recorded() {
    let server = server();
    let loaded = load_bytes(&DeclaringLoader, &server, b"body");

    // Asking for the same path/type again resolves to the same handle.
    let declared: Handle<ByteAsset> =
        server.get_or_create_path_handle(AssetPath::from("dep.bytes"));
    assert_eq!(loaded.dependencies.len(), 1);
    assert!(loaded.dependencies.contains(&declared.id().untyped()));
}

#[test]
fn finish_fills_in_embedded_but_undeclared_dependencies() {
    let server = server();
    let embedded: Handle<ByteAsset> =
        server.get_or_create_path_handle(AssetPath::from("embedded.bytes"));

    // The loader never called `load`; the handle is only embedded in the value.
    let loaded = context(&server, "root.bytes").finish(EmbeddingAsset {
        inner: embedded.clone(),
    });

    assert_eq!(loaded.dependencies.len(), 1);
    assert!(loaded.dependencies.contains(&embedded.id().untyped()));
}

#[test]
fn finish_merges_declared_and_embedded_dependencies() {
    let server = server();
    let embedded: Handle<ByteAsset> =
        server.get_or_create_path_handle(AssetPath::from("embedded.bytes"));

    let mut context = context(&server, "root.bytes");
    let declared: Handle<ByteAsset> = context.load("declared.bytes");
    let loaded = context.finish(EmbeddingAsset {
        inner: embedded.clone(),
    });

    assert_eq!(loaded.dependencies.len(), 2);
    assert!(loaded.dependencies.contains(&declared.id().untyped()));
    assert!(loaded.dependencies.contains(&embedded.id().untyped()));
}

#[test]
fn labeled_assets_get_independent_path_handles() {
    let server = server();
    let mut context = context(&server, "root.bytes");

    let sub = context.add_labeled_asset("sub", OtherAsset(7));

    // The same label always resolves to the same handle.
    let sub_again: Handle<OtherAsset> = context.get_label_handle("sub");
    assert_eq!(sub.id(), sub_again.id());
    assert!(context.has_labeled_asset("sub"));

    // A labeled asset of a different type is a distinct handle.
    let sub_bytes: Handle<ByteAsset> = context.get_label_handle("sub");
    assert_ne!(sub_bytes.id().untyped(), sub.id().untyped());

    let loaded = context.finish(ByteAsset(vec![1]));

    // The labeled asset hangs off the root path as `root.bytes#sub`.
    let info_guard = server.read_infos();
    let info = info_guard.get(sub.id().untyped()).expect("labeled info");
    let path = info.path.as_ref().expect("labeled path");
    assert_eq!(path.path(), Path::new("root.bytes"));
    assert_eq!(path.label(), Some("sub"));
    drop(info_guard);

    // The labeled assets are reachable from the loaded asset by label and id.
    assert_eq!(loaded.iter_labels().collect::<Vec<_>>(), vec!["sub"]);
    assert_eq!(
        loaded.get_labeled("sub").and_then(|a| a.get::<OtherAsset>()),
        Some(&OtherAsset(7))
    );
    assert_eq!(
        loaded
            .get_labeled_by_id(sub.id())
            .and_then(|a| a.get::<OtherAsset>()),
        Some(&OtherAsset(7))
    );

    // `get_label_handle` records the label as a dependency of the root.
    assert!(loaded.dependencies.contains(&sub.id().untyped()));
}

#[test]
fn each_label_is_independent() {
    let server = server();
    let mut context = context(&server, "root.bytes");

    let a = context.add_labeled_asset("a", OtherAsset(1));
    let b = context.add_labeled_asset("b", OtherAsset(2));
    assert_ne!(a.id(), b.id());

    let loaded = context.finish(ByteAsset(vec![]));
    let mut labels: Vec<_> = loaded.iter_labels().collect();
    labels.sort_unstable();
    assert_eq!(labels, vec!["a", "b"]);
}

#[test]
fn load_state_tracks_the_three_states_and_failure() {
    let server = server();
    let handle: Handle<ByteAsset> =
        server.get_or_create_path_handle(AssetPath::from("a.bytes"));
    let id = handle.id().untyped();

    assert!(matches!(
        server.read_infos().load_state(id),
        Some(LoadState::NotLoaded)
    ));
    assert!(!server.read_infos().is_loaded(id));

    server.write_infos().set_load_state(id, LoadState::Loading);
    assert!(server.read_infos().load_state(id).unwrap().is_loading());

    server.write_infos().set_load_state(id, LoadState::Loaded);
    assert!(server.read_infos().is_loaded(id));
    // The dependency states are still `NotLoaded`.
    assert!(!server.read_infos().is_loaded_with_dependencies(id));

    server
        .write_infos()
        .set_load_state(id, LoadState::Failed(Arc::new(AssetLoadError::AssetMetaReadError)));
    let state = server.read_infos().load_state(id).unwrap();
    assert!(state.is_failed());
    assert!(!state.is_loaded());
    assert!(!server.read_infos().is_loaded(id));
    assert!(!server.read_infos().is_loaded_with_dependencies(id));
}

#[test]
fn dependency_load_states_report_themselves() {
    assert!(DependencyLoadState::Loading.is_loading());
    assert!(DependencyLoadState::Loaded.is_loaded());
    assert!(!DependencyLoadState::NotLoaded.is_loaded());
    assert!(
        DependencyLoadState::Failed(Arc::new(AssetLoadError::AssetMetaReadError)).is_failed()
    );

    assert!(RecursiveDependencyLoadState::Loading.is_loading());
    assert!(RecursiveDependencyLoadState::Loaded.is_loaded());
    assert!(
        RecursiveDependencyLoadState::Failed(Arc::new(AssetLoadError::AssetMetaReadError))
            .is_failed()
    );
}

#[test]
fn same_path_reuses_a_handle_per_asset_type() {
    let server = server();

    let a: Handle<ByteAsset> = server.get_or_create_path_handle(AssetPath::from("a.bytes"));
    let b: Handle<ByteAsset> = server.get_or_create_path_handle(AssetPath::from("a.bytes"));
    assert_eq!(a.id(), b.id());

    let other: Handle<OtherAsset> = server.get_or_create_path_handle(AssetPath::from("a.bytes"));
    assert_ne!(a.id().untyped(), other.id().untyped());

    assert_eq!(
        server
            .read_infos()
            .get_handles_untyped(&AssetPath::from("a.bytes"))
            .len(),
        2
    );
}

#[test]
fn erased_loaded_asset_round_trips_its_type() {
    let loaded: ErasedLoadedAsset = LoadedAsset::new_with_dependencies(ByteAsset(vec![3])).into();

    assert_eq!(loaded.asset_type_id(), core::any::TypeId::of::<ByteAsset>());
    assert_eq!(loaded.asset_type_name(), core::any::type_name::<ByteAsset>());
    assert_eq!(loaded.get::<OtherAsset>(), None);
    assert_eq!(loaded.get::<ByteAsset>(), Some(&ByteAsset(vec![3])));

    // A failed downcast hands the value back unchanged.
    let loaded = match loaded.downcast::<OtherAsset>() {
        Ok(_) => panic!("downcasting to the wrong type should fail"),
        Err(loaded) => loaded,
    };

    let loaded = loaded.downcast::<ByteAsset>().expect("the right type");
    assert_eq!(loaded.take(), ByteAsset(vec![3]));
}

#[test]
fn empty_path_returns_the_default_handle() {
    let server = server();
    let mut context = context(&server, "root.bytes");
    let handle: Handle<ByteAsset> = context.load("");
    assert!(handle.is_uuid());
    assert_eq!(handle.id(), crate::next::AssetId::default());
}
