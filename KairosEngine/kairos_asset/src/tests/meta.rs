//! Tests for the `.meta` sidecar types.

use crate::meta::{
    AssetAction, AssetActionMinimal, AssetMeta, AssetMetaCheck, AssetMetaDyn, AssetMetaMinimal,
    META_FORMAT_VERSION, Settings, loader_name, loader_settings_meta_transform,
};
use crate::path::AssetPath;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct TestSettings {
    value: u32,
}

impl Default for TestSettings {
    fn default() -> Self {
        Self { value: 7 }
    }
}

fn load_meta(settings: TestSettings) -> AssetMeta<TestSettings, ()> {
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

    let parsed = AssetMeta::<TestSettings, ()>::deserialize(&bytes).unwrap();
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
    let meta = AssetMeta::<TestSettings, ()>::new(AssetAction::Ignore);
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = AssetMeta::<TestSettings, ()>::deserialize(&bytes).unwrap();
    assert!(parsed.asset.is_ignore());
    assert_eq!(parsed.asset.loader_name(), None);
    assert_eq!(parsed.asset.processor_name(), None);
}

#[test]
fn process_only_carries_its_type_surface() {
    let meta = AssetMeta::<(), TestSettings>::new(AssetAction::Process {
        processor: "kairos::MyProcessor".to_string(),
        settings: TestSettings { value: 3 },
    });
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = AssetMeta::<(), TestSettings>::deserialize(&bytes).unwrap();
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
    assert_eq!(loader_name::<TestSettings>(), core::any::type_name::<TestSettings>());
    assert!(loader_name::<TestSettings>().contains("TestSettings"));
}

#[test]
fn loader_settings_downcast_and_transform() {
    let mut meta: Box<dyn AssetMetaDyn> = Box::new(load_meta(TestSettings { value: 1 }));

    assert!(meta.loader_settings().unwrap().is::<TestSettings>());
    assert_eq!(
        meta.loader_settings().unwrap().downcast_ref::<TestSettings>(),
        Some(&TestSettings { value: 1 })
    );

    let transform = loader_settings_meta_transform(|settings: &mut TestSettings| {
        settings.value = 99;
    });
    transform(meta.as_mut());

    assert_eq!(
        meta.loader_settings().unwrap().downcast_ref::<TestSettings>(),
        Some(&TestSettings { value: 99 })
    );
}

#[test]
fn meta_dyn_recovers_the_concrete_meta() {
    use core::any::Any;

    let meta: Box<dyn AssetMetaDyn> = Box::new(load_meta(TestSettings { value: 5 }));
    let any: &dyn Any = &*meta;
    let recovered = any
        .downcast_ref::<AssetMeta<TestSettings, ()>>()
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

    let mut paths = std::collections::HashSet::new();
    paths.insert(AssetPath::parse("a/b.ron"));
    assert_eq!(
        AssetMetaCheck::Paths(paths),
        AssetMetaCheck::Paths(
            std::iter::once(AssetPath::parse("a/b.ron")).collect()
        )
    );
    assert_ne!(AssetMetaCheck::Always, AssetMetaCheck::Never);
}
