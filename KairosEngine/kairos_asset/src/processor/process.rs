//! The [`Process`] trait family and the context a processor runs in.
//!
//! A processor reads the source asset's bytes, produces a value, and writes the
//! processed bytes. [`Process`] is the maximally flexible, "low level"
//! interface; [`LoadTransformAndSave`] is the high-level implementation most
//! processors should use, composed from an [`AssetLoader`], an
//! [`AssetTransformer`], and an [`AssetSaver`].
//!
//! [`ErasedProcessor`] erases the concrete processor so the registry can store
//! and resolve it by [`processor_name`](crate::meta::processor_name). It is also
//! what deserializes and produces an asset's `.meta`.

use core::marker::PhantomData;

use kairos_ecs::error::KairosError;
use kairos_tasks::{BoxedFuture, ConditionalSendFuture};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::io::{
    AssetReaderError, AssetWriterError, MissingAssetWriterError, MissingProcessedAssetReaderError,
    MissingProcessedAssetWriterError, Reader, Writer,
};
use crate::loader::{AssetLoader, ErasedLoadedAsset};
use crate::meta::{
    AssetAction, AssetMeta, AssetMetaDyn, DeserializeMetaError, ProcessDependencyInfo,
    ProcessedInfo, Settings, loader_name, processor_name,
};
use crate::path::AssetPath;
use crate::server::{AssetLoadError, AssetServer};

use super::registry::short_type_name;
use super::saver::{AssetSaver, SavedAsset};
use super::transformer::{AssetTransformer, IdentityAssetTransformer, TransformedAsset};

/// Asset "processor" logic that reads the source asset's bytes (through the
/// [`ProcessContext`]), processes the value, and writes the final processed
/// bytes to a [`Writer`].
///
/// The written bytes must be loadable by [`Process::OutputLoader`]. This is the
/// "low level", maximally flexible interface; most processors are better served
/// by [`LoadTransformAndSave`].
pub trait Process: Send + Sync + Sized + 'static {
    /// The settings this processor is configured with, stored in the asset's
    /// `.meta` and editable per asset.
    type Settings: Settings + Default + Serialize + for<'a> Deserialize<'a>;

    /// The [`AssetLoader`] that loads the processed output.
    type OutputLoader: AssetLoader;

    /// Processes the asset reached through `context`, writing the processed
    /// bytes to `writer`, and returns the [`AssetLoader::Settings`] the output
    /// loader should be configured with.
    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<
        Output = Result<<Self::OutputLoader as AssetLoader>::Settings, ProcessError>,
    >;
}

/// A flexible [`Process`] implementation: load the source asset with the `L`
/// [`AssetLoader`], transform it through the `T` [`AssetTransformer`], then save
/// it with the `S` [`AssetSaver`].
///
/// Processors that need no transformation can use
/// [`IdentityAssetTransformer`] as `T` and only implement a loader plus a saver.
pub struct LoadTransformAndSave<
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
> {
    transformer: T,
    saver: S,
    marker: PhantomData<fn() -> L>,
}

impl<L: AssetLoader, S: AssetSaver<Asset = L::Asset>> From<S>
    for LoadTransformAndSave<L, IdentityAssetTransformer<L::Asset>, S>
{
    fn from(saver: S) -> Self {
        LoadTransformAndSave::new(IdentityAssetTransformer::new(), saver)
    }
}

impl<L, T, S> LoadTransformAndSave<L, T, S>
where
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
{
    /// Creates a processor from a transformer and a saver.
    pub fn new(transformer: T, saver: S) -> Self {
        Self {
            transformer,
            saver,
            marker: PhantomData,
        }
    }
}

/// Settings for the [`LoadTransformAndSave`] [`Process::Settings`]
/// implementation.
///
/// Each field is the settings of the corresponding stage: `LoaderSettings` for
/// [`AssetLoader::Settings`], `TransformerSettings` for
/// [`AssetTransformer::Settings`], and `SaverSettings` for
/// [`AssetSaver::Settings`].
#[derive(Serialize, Deserialize, Default)]
pub struct LoadTransformAndSaveSettings<LoaderSettings, TransformerSettings, SaverSettings> {
    /// The [`AssetLoader::Settings`](crate::AssetLoader::Settings) for the load step.
    pub loader_settings: LoaderSettings,
    /// The [`AssetTransformer::Settings`] for the transform step.
    pub transformer_settings: TransformerSettings,
    /// The [`AssetSaver::Settings`] for the save step.
    pub saver_settings: SaverSettings,
}

/// An error encountered while processing an asset.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ProcessError {
    /// No processor is registered under the name from the `.meta`.
    #[error("The processor '{0}' does not exist")]
    MissingProcessor(String),
    /// The `.meta`'s short processor name matches several processors.
    #[error(
        "The processor '{processor_short_name}' is ambiguous between several processors: {ambiguous_processor_names:?}"
    )]
    AmbiguousProcessor {
        /// The requested short name.
        processor_short_name: String,
        /// The full names of the conflicting processors.
        ambiguous_processor_names: Vec<&'static str>,
    },
    /// Reading the source asset failed.
    #[error("Encountered an AssetReader error for '{path}': {err}")]
    AssetReaderError {
        /// The asset being processed.
        path: AssetPath<'static>,
        /// The underlying reader error.
        err: AssetReaderError,
    },
    /// Writing the processed asset failed.
    #[error("Encountered an AssetWriter error for '{path}': {err}")]
    AssetWriterError {
        /// The asset being processed.
        path: AssetPath<'static>,
        /// The underlying writer error.
        err: AssetWriterError,
    },
    /// The source has no writer.
    #[error("{0}")]
    MissingAssetWriterError(MissingAssetWriterError),
    /// The source has no processed reader.
    #[error("{0}")]
    MissingProcessedAssetReaderError(MissingProcessedAssetReaderError),
    /// The source has no processed writer.
    #[error("{0}")]
    MissingProcessedAssetWriterError(MissingProcessedAssetWriterError),
    /// Reading the source `.meta` failed.
    #[error("Failed to read asset metadata for {path}: {err}")]
    ReadAssetMetaError {
        /// The asset being processed.
        path: AssetPath<'static>,
        /// The underlying reader error.
        err: AssetReaderError,
    },
    /// The `.meta` could not be deserialized.
    #[error("{0}")]
    DeserializeMetaError(DeserializeMetaError),
    /// A source asset could not be loaded.
    #[error("{0}")]
    AssetLoadError(AssetLoadError),
    /// The settings handed to a processor had the wrong type for it.
    #[error(
        "The wrong meta type was passed into a processor. This is probably an internal implementation error."
    )]
    WrongMetaType,
    /// The saver failed.
    #[error("Encountered an error while saving the asset: {0}")]
    AssetSaveError(KairosError),
    /// The transformer failed.
    #[error("Encountered an error while transforming the asset: {0}")]
    AssetTransformError(KairosError),
    /// A processor was requested for an asset with no extension.
    #[error("Assets without extensions are not supported.")]
    ExtensionRequired,
}

impl From<AssetLoadError> for ProcessError {
    fn from(error: AssetLoadError) -> Self {
        Self::AssetLoadError(error)
    }
}

impl<
    L: AssetLoader,
    T: AssetTransformer<AssetInput = L::Asset>,
    S: AssetSaver<Asset = T::AssetOutput>,
> Process for LoadTransformAndSave<L, T, S>
{
    type Settings = LoadTransformAndSaveSettings<L::Settings, T::Settings, S::Settings>;
    type OutputLoader = S::OutputLoader;

    fn process(
        &self,
        context: &mut ProcessContext,
        settings: &Self::Settings,
        writer: &mut Writer,
    ) -> impl ConditionalSendFuture<
        Output = Result<<Self::OutputLoader as AssetLoader>::Settings, ProcessError>,
    > {
        async move {
            let pre_transformed = TransformedAsset::<L::Asset>::from_loaded(
                context
                    .load_source_asset::<L>(&settings.loader_settings)
                    .await?,
            )
            .expect("the source loader produced the transformer's input type");

            let post_transformed = self
                .transformer
                .transform(pre_transformed, &settings.transformer_settings)
                .await
                .map_err(|error| ProcessError::AssetTransformError(error.into()))?;

            let saved = SavedAsset::<T::AssetOutput>::from_transformed(&post_transformed);

            self.saver
                .save(
                    writer,
                    saved,
                    &settings.saver_settings,
                    context.path().clone(),
                )
                .await
                .map_err(|error| ProcessError::AssetSaveError(error.into()))
        }
    }
}

/// How a processor should be named when written into a `.meta` sidecar.
pub enum MetaTypePathKind {
    /// Use the short name (the final path segment).
    Short,
    /// Use the fully-qualified name.
    Long,
}

/// A type-erased [`Process`], for storing processors without knowing their
/// concrete type.
pub trait ErasedProcessor: Send + Sync + 'static {
    /// Type-erased [`Process::process`]: runs the processor and returns the
    /// processed asset's `.meta`.
    fn process<'a>(
        &'a self,
        context: &'a mut ProcessContext,
        settings: &'a dyn Settings,
        writer: &'a mut Writer,
    ) -> BoxedFuture<'a, Result<Box<dyn AssetMetaDyn>, ProcessError>>;

    /// Deserializes the `.meta` bytes describing this processor's settings.
    fn deserialize_meta(&self, meta: &[u8]) -> Result<Box<dyn AssetMetaDyn>, DeserializeMetaError>;

    /// The processor's fully-qualified name, matching the name a `.meta`
    /// sidecar records.
    fn type_path(&self) -> &'static str;

    /// The processor's short name: its name's final path segment.
    fn short_type_path(&self) -> &'static str;

    /// The `.meta` to use when an asset has none: name this processor and use
    /// its default settings.
    fn default_meta(&self, processor_path_kind: MetaTypePathKind) -> Box<dyn AssetMetaDyn>;
}

impl<P: Process> ErasedProcessor for P {
    fn process<'a>(
        &'a self,
        context: &'a mut ProcessContext,
        settings: &'a dyn Settings,
        writer: &'a mut Writer,
    ) -> BoxedFuture<'a, Result<Box<dyn AssetMetaDyn>, ProcessError>> {
        Box::pin(async move {
            let settings = settings
                .downcast_ref::<P::Settings>()
                .ok_or(ProcessError::WrongMetaType)?;
            let loader_settings = <P as Process>::process(self, context, settings, writer).await?;
            let output_meta: Box<dyn AssetMetaDyn> = Box::new(AssetMeta::<
                <P::OutputLoader as AssetLoader>::Settings,
                (),
            >::new(
                AssetAction::Load {
                    loader: loader_name::<P::OutputLoader>().to_string(),
                    settings: loader_settings,
                },
            ));
            Ok(output_meta)
        })
    }

    fn deserialize_meta(&self, meta: &[u8]) -> Result<Box<dyn AssetMetaDyn>, DeserializeMetaError> {
        let meta: AssetMeta<(), P::Settings> = AssetMeta::deserialize(meta)?;
        Ok(Box::new(meta))
    }

    fn type_path(&self) -> &'static str {
        processor_name::<P>()
    }

    fn short_type_path(&self) -> &'static str {
        short_type_name(processor_name::<P>())
    }

    fn default_meta(&self, processor_path_kind: MetaTypePathKind) -> Box<dyn AssetMetaDyn> {
        let processor = match processor_path_kind {
            MetaTypePathKind::Short => self.short_type_path(),
            MetaTypePathKind::Long => self.type_path(),
        };
        Box::new(AssetMeta::<(), P::Settings>::new(AssetAction::Process {
            processor: processor.to_string(),
            settings: P::Settings::default(),
        }))
    }
}

/// The scoped view a [`Process`] is given while it runs.
///
/// This must only expose processor input that is represented in the asset's
/// hash: reading an asset value through [`ProcessContext::load_source_asset`]
/// records it as a process dependency, so a change to it re-runs processing.
pub struct ProcessContext<'a> {
    /// The processed info being built for this asset. The context only ever
    /// appends to its `process_dependencies`.
    pub(crate) new_processed_info: &'a mut ProcessedInfo,
    /// The server used to resolve loaders and load source asset values.
    server: &'a AssetServer,
    /// The path of the asset being processed.
    path: &'a AssetPath<'static>,
    /// The source bytes reader for the asset being processed.
    reader: Box<dyn Reader + 'a>,
}

impl<'a> ProcessContext<'a> {
    // Constructed by the `AssetProcessor` (S5) and by tests.
    #[allow(dead_code)]
    pub(crate) fn new(
        server: &'a AssetServer,
        path: &'a AssetPath<'static>,
        reader: Box<dyn Reader + 'a>,
        new_processed_info: &'a mut ProcessedInfo,
    ) -> Self {
        Self {
            server,
            path,
            reader,
            new_processed_info,
        }
    }

    /// Loads the source asset with the `L` [`AssetLoader`] and `settings`.
    ///
    /// Every dependency the load read (its `loader_dependencies`) is recorded
    /// as a process dependency, since the processed output depends on those
    /// values.
    pub async fn load_source_asset<L: AssetLoader>(
        &mut self,
        settings: &L::Settings,
    ) -> Result<ErasedLoadedAsset, AssetLoadError> {
        let server = self.server;
        let path = self.path;
        let loader = server.get_asset_loader_with_type_name(loader_name::<L>()).await?;
        let loaded_asset = server
            .load_with_settings_loader_and_reader(
                path,
                settings,
                &*loader,
                &mut *self.reader,
                false,
                true,
            )
            .await?;
        for (dependency_path, full_hash) in &loaded_asset.loader_dependencies {
            self.new_processed_info
                .process_dependencies
                .push(ProcessDependencyInfo {
                    full_hash: *full_hash,
                    path: dependency_path.clone(),
                });
        }
        Ok(loaded_asset)
    }

    /// The path of the asset being processed.
    #[inline]
    pub fn path(&self) -> &AssetPath<'static> {
        self.path
    }

    /// The reader for the asset's raw source bytes.
    #[inline]
    pub fn asset_reader(&mut self) -> &mut dyn Reader {
        &mut *self.reader
    }
}
