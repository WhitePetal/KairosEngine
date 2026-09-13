//! Tests for the `.meta` sidecar types.

use crate::io::{Reader, VecReader, Writer};
use crate::loader::{AssetLoader, LoadContext};
use crate::meta::{
    AssetAction, AssetActionMinimal, AssetHash, AssetMeta, AssetMetaCheck, AssetMetaDyn,
    AssetMetaMinimal, META_FORMAT_VERSION, Settings, get_asset_hash, get_full_asset_hash,
    loader_name, loader_settings_meta_transform,
};
use crate::path::AssetPath;
use crate::processor::{Process, ProcessContext, ProcessError};
use futures_lite::future::block_on;
use kairos_tasks::ConditionalSendFuture;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct TestSettings {
    value: u32,
}

impl Default for TestSettings {
    fn default() -> Self {
        Self { value: 7 }
    }
}

/// A loader whose settings are [`TestSettings`].
///
/// `AssetMeta<L, P>` is parameterised by the loader and processor *types*, so the
/// meta tests need names for them. Neither is ever called.
struct TestLoader;

impl AssetLoader for TestLoader {
    type Asset = ();
    type Settings = TestSettings;
    type Error = std::io::Error;

    fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        async { unreachable!("the meta tests never load anything") }
    }
}

/// A processor whose settings are [`TestSettings`], for the process-only meta.
struct TestProcessor;

impl Process for TestProcessor {
    type Settings = TestSettings;
    type OutputLoader = ();

    fn process(
        &self,
        _context: &mut ProcessContext,
        _settings: &Self::Settings,
        _writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async { unreachable!("the meta tests never process anything") }
    }
}

fn load_meta(settings: TestSettings) -> AssetMeta<TestLoader, ()> {
    AssetMeta::new(AssetAction::Load {
        loader: loader_name::<TestSettings>().to_string(),
        settings,
    })
}

#[test]
fn meta_constants_are_stable() {
    assert_eq!(META_FORMAT_VERSION, "1.0");

    let meta = load_meta(TestSettings::default());
    assert_eq!(meta.meta_format_version, META_FORMAT_VERSION);
    assert!(meta.processed_info.is_none());
}

#[test]
fn typed_meta_round_trips_through_ron() {
    let meta = load_meta(TestSettings { value: 42 });
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = AssetMeta::<TestLoader, ()>::deserialize(&bytes).unwrap();
    assert_eq!(parsed.meta_format_version, META_FORMAT_VERSION);
    match parsed.asset {
        AssetAction::Load { loader, settings } => {
            assert_eq!(loader, loader_name::<TestSettings>());
            assert_eq!(settings, TestSettings { value: 42 });
        }
        _ => panic!("expected a Load action"),
    }
}

#[test]
fn ignore_is_a_real_variant() {
    let meta = AssetMeta::<TestLoader, ()>::new(AssetAction::Ignore);
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = AssetMeta::<TestLoader, ()>::deserialize(&bytes).unwrap();
    assert!(parsed.asset.is_ignore());
    assert_eq!(parsed.asset.loader_name(), None);
    assert_eq!(parsed.asset.processor_name(), None);
}

#[test]
fn process_only_carries_its_type_surface() {
    let meta = AssetMeta::<(), TestProcessor>::new(AssetAction::Process {
        processor: "kairos::MyProcessor".to_string(),
        settings: TestSettings { value: 3 },
    });
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = AssetMeta::<(), TestProcessor>::deserialize(&bytes).unwrap();
    assert_eq!(parsed.asset.processor_name(), Some("kairos::MyProcessor"));
    assert_eq!(parsed.asset.loader_name(), None);
}

#[test]
fn minimal_meta_names_the_loader_without_settings() {
    let meta = load_meta(TestSettings::default());
    let bytes = AssetMetaDyn::serialize(&meta);

    let minimal = AssetMetaMinimal::deserialize(&bytes).unwrap();
    match minimal.asset {
        AssetActionMinimal::Load { loader } => {
            assert_eq!(loader, loader_name::<TestSettings>());
        }
        _ => panic!("expected a Load action"),
    }
}

#[test]
fn loader_name_is_the_fully_qualified_type_name() {
    assert_eq!(
        loader_name::<TestSettings>(),
        core::any::type_name::<TestSettings>()
    );
    assert!(loader_name::<TestSettings>().contains("TestSettings"));
}

#[test]
fn loader_settings_downcast_and_transform() {
    let mut meta: Box<dyn AssetMetaDyn> = Box::new(load_meta(TestSettings { value: 1 }));

    assert!(meta.loader_settings().unwrap().is::<TestSettings>());
    assert_eq!(
        meta.loader_settings()
            .unwrap()
            .downcast_ref::<TestSettings>(),
        Some(&TestSettings { value: 1 })
    );

    let transform = loader_settings_meta_transform(|settings: &mut TestSettings| {
        settings.value = 99;
    });
    transform(meta.as_mut());

    assert_eq!(
        meta.loader_settings()
            .unwrap()
            .downcast_ref::<TestSettings>(),
        Some(&TestSettings { value: 99 })
    );
}

#[test]
fn meta_dyn_recovers_the_concrete_meta() {
    use core::any::Any;

    let meta: Box<dyn AssetMetaDyn> = Box::new(load_meta(TestSettings { value: 5 }));
    let any: &dyn Any = &*meta;
    let recovered = any
        .downcast_ref::<AssetMeta<TestLoader, ()>>()
        .expect("AssetMetaDyn is Any, so the concrete meta is recoverable");
    assert_eq!(recovered.meta_format_version, META_FORMAT_VERSION);
}

#[test]
fn settings_is_blanket_implemented() {
    fn assert_settings<T: Settings>() {}
    assert_settings::<u32>();
    assert_settings::<TestSettings>();
    assert_settings::<()>();
}

#[test]
fn meta_check_defaults_to_always() {
    assert_eq!(AssetMetaCheck::default(), AssetMetaCheck::Always);

    let mut paths = kairos_collections::FixedHashSet::new();
    paths.insert(AssetPath::parse("a/b.ron"));
    assert_eq!(
        AssetMetaCheck::Paths(paths),
        AssetMetaCheck::Paths(std::iter::once(AssetPath::parse("a/b.ron")).collect())
    );
    assert_ne!(AssetMetaCheck::Always, AssetMetaCheck::Never);
}

fn hash_of(meta: &[u8], body: &[u8]) -> AssetHash {
    let mut reader = VecReader::new(body.to_vec());
    block_on(get_asset_hash(meta, &mut reader)).expect("hashing a reader succeeds")
}

#[test]
fn asset_hash_is_stable_for_the_same_input() {
    assert_eq!(hash_of(b"meta", b"body"), hash_of(b"meta", b"body"));
}

#[test]
fn asset_hash_changes_with_either_input() {
    let baseline = hash_of(b"meta", b"body");
    assert_ne!(baseline, hash_of(b"other", b"body"));
    assert_ne!(baseline, hash_of(b"meta", b"other"));
    // Length alone matters too: a prefix is not the same asset.
    assert_ne!(baseline, hash_of(b"meta", b"bod"));
}

#[test]
fn full_hash_folds_dependency_hashes_in_order() {
    let asset = [1u8; 32];
    let a = [2u8; 32];
    let b = [3u8; 32];

    let no_deps = get_full_asset_hash(asset, std::iter::empty());
    assert_eq!(no_deps, get_full_asset_hash(asset, std::iter::empty()));
    // A different asset hash changes the full hash.
    assert_ne!(no_deps, get_full_asset_hash([9u8; 32], std::iter::empty()));
    // Adding a dependency changes the full hash.
    assert_ne!(no_deps, get_full_asset_hash(asset, std::iter::once(a)));
    // Order is significant.
    assert_ne!(
        get_full_asset_hash(asset, [a, b].into_iter()),
        get_full_asset_hash(asset, [b, a].into_iter())
    );
    // A changed dependency changes the full hash.
    assert_ne!(
        get_full_asset_hash(asset, [a, b].into_iter()),
        get_full_asset_hash(asset, [a, [4u8; 32]].into_iter())
    );
}
