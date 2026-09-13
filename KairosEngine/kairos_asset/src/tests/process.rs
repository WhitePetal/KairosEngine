//! Tests for the processor trait family: the registry, [`ProcessContext`],
//! settings handling, and [`LoadTransformAndSave`].

use core::any::type_name;

use futures_lite::future::block_on;
use futures_lite::io::{AsyncWriteExt, Cursor};
use kairos_ecs::error::KairosError;
use kairos_tasks::ConditionalSendFuture;
use serde::{Deserialize, Serialize};

use crate::io::{Reader, VecReader, Writer};
use crate::meta::{
    AssetAction, AssetActionMinimal, AssetMeta, AssetMetaDyn, AssetMetaMinimal, ProcessedInfo,
    Settings,
};
use crate::path::AssetPath;
use crate::processor::{
    AssetSaver, AssetTransformer, ErasedProcessor, GetProcessorError, IdentityAssetTransformer,
    LoadTransformAndSave, LoadTransformAndSaveSettings, MetaTypePathKind, Process, ProcessContext,
    ProcessError, Processors, SavedAsset, TransformedAsset,
};
use crate::{Asset, AssetLoader, AssetServer, LoadContext, VisitAssetDependencies};

/// An asset whose value is the text it was loaded from.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TextAsset(String);

impl Asset for TextAsset {}
impl VisitAssetDependencies for TextAsset {}

/// A loader that returns the reader's bytes as text.
pub(crate) struct TextLoader;

impl AssetLoader for TextLoader {
    type Asset = TextAsset;
    type Settings = ();
    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<TextAsset, std::io::Error>> {
        async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(TextAsset(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }

    fn extensions(&self) -> &[&str] {
        &["txt"]
    }
}

/// A loader that loads a nested asset through the direct path, so the outer
/// load records a `loader_dependency`.
struct DependencyLoader;

impl AssetLoader for DependencyLoader {
    type Asset = TextAsset;
    type Settings = ();
    type Error = KairosError;

    fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext,
    ) -> impl ConditionalSendFuture<Output = Result<TextAsset, KairosError>> {
        async move {
            let mut nested = VecReader::new(b"nested".to_vec());
            let _nested: crate::ErasedLoadedAsset = load_context
                .load_direct_internal(
                    AssetPath::from("nested.txt"),
                    &(),
                    &TextLoader,
                    &mut nested,
                    None,
                )
                .await?;
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            Ok(TextAsset(String::from_utf8_lossy(&bytes).into_owned()))
        }
    }
}

/// Settings with a value, to prove settings survive a `.meta` round-trip.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct TextSettings {
    prefix: String,
}

/// A custom [`Process`] using the low-level API: it loads the source asset
/// through the context and writes the prefixed text.
struct TextProcessor;

impl Process for TextProcessor {
    type Settings = TextSettings;
    type OutputLoader = TextLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async move {
            let loaded = context.load_source_asset::<TextLoader>(&()).await?;
            let value = loaded
                .get::<TextAsset>()
                .expect("the source loader produced a TextAsset");
            let text = format!("{}{}", settings.prefix, value.0);
            writer.write_all(text.as_bytes()).await.map_err(|err| {
                ProcessError::AssetWriterError {
                    path: context.path().clone(),
                    err: err.into(),
                }
            })?;
            Ok(())
        }
    }
}

/// A processor that exercises `load_source_asset`'s dependency recording.
struct DependencyProcessor;

impl Process for DependencyProcessor {
    type Settings = ();
    type OutputLoader = TextLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        _settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async move {
            context.load_source_asset::<DependencyLoader>(&()).await?;
            writer
                .write_all(b"processed")
                .await
                .map_err(|err| ProcessError::AssetWriterError {
                    path: context.path().clone(),
                    err: err.into(),
                })?;
            Ok(())
        }
    }
}

/// A saver that writes the text back out.
struct TextSaver;

impl AssetSaver for TextSaver {
    type Asset = TextAsset;
    type Settings = ();
    type OutputLoader = TextLoader;
    type Error = std::io::Error;

    fn save(
        &self,
        writer: &mut Writer,
        asset: SavedAsset<'_, TextAsset>,
        _settings: &(),
        _asset_path: AssetPath<'_>,
    ) -> impl ConditionalSendFuture<Output = Result<(), std::io::Error>> {
        async move {
            writer.write_all(asset.get().0.as_bytes()).await?;
            Ok(())
        }
    }
}

/// A transformer that upper-cases the text.
struct UpperCaseTransformer;

impl AssetTransformer for UpperCaseTransformer {
    type AssetInput = TextAsset;
    type AssetOutput = TextAsset;
    type Settings = ();
    type Error = core::convert::Infallible;

    fn transform(
        &self,
        asset: TransformedAsset<Self::AssetInput>,
        _settings: &Self::Settings,
    ) -> impl ConditionalSendFuture<Output = Result<TransformedAsset<Self::AssetOutput>, Self::Error>>
    {
        async move {
            let upper = asset.get().0.to_uppercase();
            Ok(asset.replace_asset(TextAsset(upper)))
        }
    }
}

/// Two processors in different modules with the same short name (`Duplicate`).
mod alpha {
    pub(crate) struct Duplicate;
}

mod beta {
    pub(crate) struct Duplicate;
}

impl Process for alpha::Duplicate {
    type Settings = ();
    type OutputLoader = TextLoader;

    fn process(
        &self,
        _context: &mut ProcessContext,
        _settings: &Self::Settings,
        _writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async move { Ok(()) }
    }
}

impl Process for beta::Duplicate {
    type Settings = ();
    type OutputLoader = TextLoader;

    fn process(
        &self,
        _context: &mut ProcessContext,
        _settings: &Self::Settings,
        _writer: &mut Writer,
    ) -> impl ConditionalSendFuture<Output = Result<(), ProcessError>> {
        async move { Ok(()) }
    }
}

/// Runs `processor` over `source`, returning the processed bytes and info.
async fn run_processor<P: Process>(
    server: &AssetServer,
    processor: &P,
    settings: &P::Settings,
    source: &[u8],
) -> (Vec<u8>, ProcessedInfo) {
    let path = AssetPath::from("source.txt");
    let mut info = ProcessedInfo::default();
    let mut output = Cursor::new(Vec::new());
    {
        let mut context = ProcessContext::new(
            server,
            &path,
            Box::new(VecReader::new(source.to_vec())),
            &mut info,
        );
        let processor: &dyn ErasedProcessor = processor;
        block_on(processor.process(&mut context, settings, &mut output))
            .expect("processing succeeds");
    }
    (output.into_inner(), info)
}

fn server() -> AssetServer {
    let server = AssetServer::new();
    server.register_loader(TextLoader);
    server.register_loader(DependencyLoader);
    server
}

#[test]
fn a_custom_processor_reads_the_source_asset() {
    let server = server();
    let (bytes, info) = block_on(run_processor(
        &server,
        &TextProcessor,
        &TextSettings {
            prefix: "> ".into(),
        },
        b"hello",
    ));
    assert_eq!(bytes, b"> hello");
    // The source loader declared no loader dependencies, so there are none.
    assert!(info.process_dependencies.is_empty());
}

#[test]
fn load_source_asset_records_loader_dependencies_as_process_dependencies() {
    let server = server();
    let (bytes, info) = block_on(run_processor(&server, &DependencyProcessor, &(), b"body"));
    assert_eq!(bytes, b"processed");
    assert_eq!(info.process_dependencies.len(), 1);
    let dependency = &info.process_dependencies[0];
    assert_eq!(dependency.path, AssetPath::from("nested.txt"));
    assert_eq!(
        dependency.full_hash, [0u8; 32],
        "without processed info the dependency records the zero hash"
    );
}

#[test]
fn the_registry_resolves_full_and_short_names() {
    let mut processors = Processors::default();
    processors.register_processor(TextProcessor);

    let by_full = processors
        .get_processor(type_name::<TextProcessor>())
        .expect("registered by full name");
    assert_eq!(by_full.type_path(), type_name::<TextProcessor>());

    let by_short = processors
        .get_processor("TextProcessor")
        .expect("registered by short name");
    assert_eq!(by_short.type_path(), type_name::<TextProcessor>());

    assert!(matches!(
        processors.get_processor("NotRegistered"),
        Err(GetProcessorError::Missing(name)) if name == "NotRegistered"
    ));
}

#[test]
fn ambiguous_short_names_are_rejected() {
    let mut processors = Processors::default();
    processors.register_processor(alpha::Duplicate);
    processors.register_processor(beta::Duplicate);

    // The short name now matches two processors.
    match processors.get_processor("Duplicate") {
        Err(GetProcessorError::Ambiguous {
            processor_short_name,
            ambiguous_processor_names,
        }) => {
            assert_eq!(processor_short_name, "Duplicate");
            assert_eq!(ambiguous_processor_names.len(), 2);
        }
        Err(other) => panic!("expected an ambiguity error, got {other}"),
        Ok(_) => panic!("expected an ambiguity error, got a processor"),
    }

    // The full names stay unambiguous.
    assert!(
        processors
            .get_processor(type_name::<alpha::Duplicate>())
            .is_ok()
    );
    assert!(
        processors
            .get_processor(type_name::<beta::Duplicate>())
            .is_ok()
    );
}

#[test]
fn default_processors_are_addressed_by_extension() {
    let mut processors = Processors::default();
    processors.register_processor(TextProcessor);
    processors.set_default_processor::<TextProcessor>("txt");

    let default = processors
        .get_default_processor("txt")
        .expect("a default for txt");
    assert_eq!(default.type_path(), type_name::<TextProcessor>());
    assert!(processors.get_default_processor("png").is_none());
}

#[test]
fn default_meta_names_the_processor_and_carries_default_settings() {
    let mut processors = Processors::default();
    processors.register_processor(TextProcessor);
    let processor = processors
        .get_processor("TextProcessor")
        .expect("registered");

    let short = processor.default_meta(MetaTypePathKind::Short);
    assert!(short.process_settings().is_some());
    let minimal = AssetMetaMinimal::deserialize(&short.serialize()).expect("valid ron");
    assert!(matches!(
        minimal.asset,
        AssetActionMinimal::Process { processor } if processor == "TextProcessor"
    ));

    let long = processor.default_meta(MetaTypePathKind::Long);
    let minimal = AssetMetaMinimal::deserialize(&long.serialize()).expect("valid ron");
    assert!(matches!(
        minimal.asset,
        AssetActionMinimal::Process { processor } if processor == type_name::<TextProcessor>()
    ));
}

#[test]
fn settings_round_trip_through_a_meta_sidecar() {
    let mut processors = Processors::default();
    processors.register_processor(TextProcessor);
    let processor = processors
        .get_processor(type_name::<TextProcessor>())
        .expect("registered");

    let meta = AssetMeta::<(), TextSettings>::new(AssetAction::Process {
        processor: type_name::<TextProcessor>().to_string(),
        settings: TextSettings {
            prefix: "// ".into(),
        },
    });
    let bytes = AssetMetaDyn::serialize(&meta);

    let parsed = processor.deserialize_meta(&bytes).expect("deserializes");
    assert_eq!(
        parsed
            .process_settings()
            .and_then(|s| s.downcast_ref::<TextSettings>()),
        Some(&TextSettings {
            prefix: "// ".into()
        })
    );
}

#[test]
fn a_processor_with_wrong_settings_is_rejected() {
    let mut processors = Processors::default();
    processors.register_processor(TextProcessor);
    let processor = processors
        .get_processor(type_name::<TextProcessor>())
        .expect("registered");

    // `TextProcessor` expects `TextSettings`, not `()`.
    let path = AssetPath::from("source.txt");
    let mut info = ProcessedInfo::default();
    let server = server();
    let mut context = ProcessContext::new(
        &server,
        &path,
        Box::new(VecReader::new(Vec::new())),
        &mut info,
    );
    let mut output = Cursor::new(Vec::new());
    let wrong: &dyn Settings = &();
    let result = block_on(processor.process(&mut context, wrong, &mut output));
    assert!(matches!(result, Err(ProcessError::WrongMetaType)));
}

#[test]
fn load_transform_and_save_runs_all_three_stages() {
    let server = server();
    let processor: LoadTransformAndSave<TextLoader, UpperCaseTransformer, TextSaver> =
        LoadTransformAndSave::new(UpperCaseTransformer, TextSaver);

    let (bytes, _info) = block_on(run_processor(
        &server,
        &processor,
        &LoadTransformAndSaveSettings::<(), (), ()>::default(),
        b"hello",
    ));
    assert_eq!(bytes, b"HELLO");
}

#[test]
fn load_transform_and_save_from_saver_uses_the_identity_transform() {
    let server = server();
    let processor: LoadTransformAndSave<
        TextLoader,
        IdentityAssetTransformer<TextAsset>,
        TextSaver,
    > = TextSaver.into();

    let (bytes, _info) = block_on(run_processor(
        &server,
        &processor,
        &LoadTransformAndSaveSettings::<(), (), ()>::default(),
        b"hello",
    ));
    assert_eq!(bytes, b"hello");
}

#[test]
fn processed_output_meta_is_a_load_action_for_the_output_loader() {
    let server = server();
    let processor: LoadTransformAndSave<
        TextLoader,
        IdentityAssetTransformer<TextAsset>,
        TextSaver,
    > = TextSaver.into();

    let path = AssetPath::from("source.txt");
    let mut info = ProcessedInfo::default();
    let mut context = ProcessContext::new(
        &server,
        &path,
        Box::new(VecReader::new(b"hi".to_vec())),
        &mut info,
    );
    let mut output = Cursor::new(Vec::new());
    let processor: &dyn ErasedProcessor = &processor;
    let settings = LoadTransformAndSaveSettings::<(), (), ()>::default();
    let meta = block_on(processor.process(&mut context, &settings, &mut output))
        .expect("processing succeeds");

    // The processed side always carries a `Load` action, never a `Process`.
    let minimal = AssetMetaMinimal::deserialize(&meta.serialize()).expect("valid ron");
    assert!(matches!(
        minimal.asset,
        AssetActionMinimal::Load { loader } if loader == type_name::<TextLoader>()
    ));
}
