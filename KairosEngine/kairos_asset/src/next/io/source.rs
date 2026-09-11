//! Asset sources: named roots that asset paths resolve against.
//!
//! An [`AssetSource`] owns one [`AssetReader`](crate::next::io::AssetReader),
//! an optional [`AssetWriter`](crate::next::io::AssetWriter), and an optional
//! processed reader. Sources are named by an [`AssetSourceId`]:
//! [`Default`](AssetSourceId::Default) is the unnamed root that plain paths
//! resolve against, while a [`Name`](AssetSourceId::Name) is written
//! `name://path` in an [`AssetPath`](crate::next::path::AssetPath).
//!
//! Sources are described up front as [`AssetSourceBuilder`]s inside an
//! [`AssetSourceBuilders`] resource and frozen into [`AssetSources`] once they
//! are known. The default source is rooted at the process working directory
//! rather than bevy's `assets` folder, so today's cwd-relative literals keep
//! resolving to the same files (ADR 0004).

use std::{
    collections::HashMap,
    fmt::{self, Display},
    hash::{Hash, Hasher},
};

use atomicow::CowArc;

use crate::next::io::{ErasedAssetReader, ErasedAssetWriter, file::FileAssetReader};

/// Names an [`AssetSource`].
///
/// [`Default`](AssetSourceId::Default) is the source used by paths that do not
/// spell one out; [`Name`](AssetSourceId::Name) is the `name://` form.
#[derive(Clone, Debug, Default)]
pub enum AssetSourceId<'a> {
    /// The default asset source.
    #[default]
    Default,
    /// A named asset source.
    Name(CowArc<'a, str>),
}

impl<'a> AssetSourceId<'a> {
    /// Builds an id from an optional source name.
    pub fn new(source: Option<impl Into<CowArc<'a, str>>>) -> AssetSourceId<'a> {
        match source {
            Some(source) => AssetSourceId::Name(source.into()),
            None => AssetSourceId::Default,
        }
    }

    /// The source name, or [`None`] for [`AssetSourceId::Default`].
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AssetSourceId::Default => None,
            AssetSourceId::Name(name) => Some(name),
        }
    }

    /// Converts this into an owned id, cloning a borrowed name.
    pub fn into_owned(self) -> AssetSourceId<'static> {
        match self {
            AssetSourceId::Default => AssetSourceId::Default,
            AssetSourceId::Name(name) => AssetSourceId::Name(name.into_owned()),
        }
    }

    /// Clones this into an owned id. Equivalent to `.clone().into_owned()`.
    #[inline]
    pub fn clone_owned(&self) -> AssetSourceId<'static> {
        self.clone().into_owned()
    }
}

impl Display for AssetSourceId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            None => write!(f, "AssetSourceId::Default"),
            Some(name) => write!(f, "AssetSourceId::Name({name})"),
        }
    }
}

impl From<&'static str> for AssetSourceId<'static> {
    fn from(value: &'static str) -> Self {
        AssetSourceId::Name(value.into())
    }
}

impl<'a, 'b> From<&'a AssetSourceId<'b>> for AssetSourceId<'b> {
    fn from(value: &'a AssetSourceId<'b>) -> Self {
        value.clone()
    }
}

impl From<Option<&'static str>> for AssetSourceId<'static> {
    fn from(value: Option<&'static str>) -> Self {
        match value {
            Some(value) => AssetSourceId::Name(value.into()),
            None => AssetSourceId::Default,
        }
    }
}

impl From<String> for AssetSourceId<'static> {
    fn from(value: String) -> Self {
        AssetSourceId::Name(value.into())
    }
}

impl Hash for AssetSourceId<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq for AssetSourceId<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(&other.as_str())
    }
}

impl Eq for AssetSourceId<'_> {}

/// A blueprint for one [`AssetSource`].
///
/// Each slot holds a repeatable constructor rather than a built value, so a
/// source can be rebuilt (for example when a watcher restarts). Only the
/// unprocessed and processed readers are populated by
/// [`platform_default`](AssetSourceBuilder::platform_default) today; the writer
/// slot stays empty until the asset processor lands.
pub struct AssetSourceBuilder {
    /// Builds the unprocessed reader.
    pub reader: Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>,
    /// Builds the unprocessed writer, if any.
    pub writer: Option<Box<dyn FnMut(bool) -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync>>,
    /// Builds the processed reader, if any.
    pub processed_reader: Option<Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>>,
}

impl AssetSourceBuilder {
    /// Creates a builder whose reader is produced by `reader`.
    pub fn new(
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        Self {
            reader: Box::new(reader),
            writer: None,
            processed_reader: None,
        }
    }

    /// Replaces the unprocessed reader constructor.
    pub fn with_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.reader = Box::new(reader);
        self
    }

    /// Sets the unprocessed writer constructor.
    pub fn with_writer(
        mut self,
        writer: impl FnMut(bool) -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.writer = Some(Box::new(writer));
        self
    }

    /// Sets the processed reader constructor.
    pub fn with_processed_reader(
        mut self,
        reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static,
    ) -> Self {
        self.processed_reader = Some(Box::new(reader));
        self
    }

    /// Builds the [`AssetSource`] for `id`.
    pub fn build(&mut self, id: AssetSourceId<'static>) -> AssetSource {
        AssetSource {
            id,
            reader: self.reader.as_mut()(),
            writer: self.writer.as_mut().and_then(|writer| writer(false)),
            processed_reader: self.processed_reader.as_mut().map(|reader| reader()),
        }
    }

    /// A builder rooted at `path` (relative to the process working directory),
    /// with an optional processed root at `processed_path`.
    ///
    /// The reader is a [`FileAssetReader`]. No writer is configured: writing
    /// assets back is the processor's job, and it is deferred.
    pub fn platform_default(path: &str, processed_path: Option<&str>) -> Self {
        let reader_path = path.to_owned();
        let mut builder = Self::new(move || {
            Box::new(FileAssetReader::new(&reader_path)) as Box<dyn ErasedAssetReader>
        });
        if let Some(processed_path) = processed_path {
            let processed_path = processed_path.to_owned();
            builder = builder.with_processed_reader(move || {
                Box::new(FileAssetReader::new(&processed_path)) as Box<dyn ErasedAssetReader>
            });
        }
        builder
    }
}

/// Holds the [`AssetSourceBuilder`]s registered before the sources are frozen
/// into [`AssetSources`], plus the default source's builder.
///
/// This is the resource the engine fills in during startup and then builds
/// once; keeping the builders separate means a source can still be replaced
/// before anything reads from it.
#[derive(Default)]
pub struct AssetSourceBuilders {
    sources: HashMap<CowArc<'static, str>, AssetSourceBuilder>,
    default: Option<AssetSourceBuilder>,
}

impl AssetSourceBuilders {
    /// Registers `source` under `id`, replacing any earlier builder.
    pub fn insert(&mut self, id: impl Into<AssetSourceId<'static>>, source: AssetSourceBuilder) {
        match id.into() {
            AssetSourceId::Default => {
                self.default = Some(source);
            }
            AssetSourceId::Name(name) => {
                self.sources.insert(name, source);
            }
        }
    }

    /// The builder registered under `id`, if any.
    pub fn get_mut<'a, 'b>(
        &'a mut self,
        id: impl Into<AssetSourceId<'b>>,
    ) -> Option<&'a mut AssetSourceBuilder> {
        match id.into() {
            AssetSourceId::Default => self.default.as_mut(),
            AssetSourceId::Name(name) => self.sources.get_mut(name.as_ref()),
        }
    }

    /// Initializes the default builder with a file reader rooted at `path`,
    /// unless one was already set.
    pub fn init_default_source(&mut self, path: &str, processed_path: Option<&str>) {
        self.default
            .get_or_insert_with(|| AssetSourceBuilder::platform_default(path, processed_path));
    }

    /// Freezes the registered builders into [`AssetSources`].
    ///
    /// # Panics
    ///
    /// Panics if no default source has been registered: every path that does
    /// not name a source resolves through the default, so there is no sensible
    /// fallback.
    pub fn build_sources(&mut self) -> AssetSources {
        let mut sources = HashMap::with_capacity(self.sources.len());
        for (id, source) in &mut self.sources {
            let source = source.build(AssetSourceId::Name(id.clone()));
            sources.insert(id.clone(), source);
        }

        AssetSources {
            sources,
            default: self
                .default
                .as_mut()
                .map(|source| source.build(AssetSourceId::Default))
                .expect(MISSING_DEFAULT_SOURCE),
        }
    }
}

/// The frozen set of [`AssetSource`]s the asset server reads from.
pub struct AssetSources {
    sources: HashMap<CowArc<'static, str>, AssetSource>,
    default: AssetSource,
}

impl AssetSources {
    /// The source named by `id`.
    pub fn get<'a, 'b>(
        &'a self,
        id: impl Into<AssetSourceId<'b>>,
    ) -> Result<&'a AssetSource, MissingAssetSourceError> {
        match id.into().into_owned() {
            AssetSourceId::Default => Ok(&self.default),
            AssetSourceId::Name(name) => self
                .sources
                .get(name.as_ref())
                .ok_or(MissingAssetSourceError(AssetSourceId::Name(name))),
        }
    }

    /// Every source, default last.
    pub fn iter(&self) -> impl Iterator<Item = &AssetSource> {
        self.sources.values().chain(Some(&self.default))
    }

    /// Every source mutably, default last.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut AssetSource> {
        self.sources.values_mut().chain(Some(&mut self.default))
    }

    /// The sources that have a processed reader.
    pub fn iter_processed(&self) -> impl Iterator<Item = &AssetSource> {
        self.iter().filter(|source| source.should_process())
    }

    /// The sources that have a processed reader, mutably.
    pub fn iter_processed_mut(&mut self) -> impl Iterator<Item = &mut AssetSource> {
        self.iter_mut().filter(|source| source.should_process())
    }

    /// The id of every source, default last.
    pub fn ids(&self) -> impl Iterator<Item = AssetSourceId<'static>> + '_ {
        self.sources
            .keys()
            .map(|id| AssetSourceId::Name(id.clone()))
            .chain(Some(AssetSourceId::Default))
    }
}

/// One resolvable asset root: its id plus the reader, writer, and processed
/// reader slots.
pub struct AssetSource {
    id: AssetSourceId<'static>,
    reader: Box<dyn ErasedAssetReader>,
    writer: Option<Box<dyn ErasedAssetWriter>>,
    processed_reader: Option<Box<dyn ErasedAssetReader>>,
}

impl AssetSource {
    /// This source's id.
    #[inline]
    pub fn id(&self) -> AssetSourceId<'static> {
        self.id.clone()
    }

    /// The unprocessed reader.
    #[inline]
    pub fn reader(&self) -> &dyn ErasedAssetReader {
        &*self.reader
    }

    /// The unprocessed writer, if one is configured.
    #[inline]
    pub fn writer(&self) -> Result<&dyn ErasedAssetWriter, MissingAssetWriterError> {
        self.writer
            .as_deref()
            .ok_or_else(|| MissingAssetWriterError(self.id.clone_owned()))
    }

    /// The processed reader, if one is configured.
    #[inline]
    pub fn processed_reader(
        &self,
    ) -> Result<&dyn ErasedAssetReader, MissingProcessedAssetReaderError> {
        self.processed_reader
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetReaderError(self.id.clone_owned()))
    }

    /// Whether this source has processed assets to read.
    #[inline]
    pub fn should_process(&self) -> bool {
        self.processed_reader.is_some()
    }
}

/// Returned by [`AssetSources::get`] when no source has the requested id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingAssetSourceError(pub AssetSourceId<'static>);

impl Display for MissingAssetSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Asset Source '{}' does not exist", self.0)
    }
}

impl std::error::Error for MissingAssetSourceError {}

/// Returned by [`AssetSource::writer`] when the source has no writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingAssetWriterError(pub AssetSourceId<'static>);

impl Display for MissingAssetWriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Asset Source '{}' does not have an AssetWriter.", self.0)
    }
}

impl std::error::Error for MissingAssetWriterError {}

/// Returned by [`AssetSource::processed_reader`] when the source has no
/// processed reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingProcessedAssetReaderError(pub AssetSourceId<'static>);

impl Display for MissingProcessedAssetReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Asset Source '{}' does not have a processed AssetReader.",
            self.0
        )
    }
}

impl std::error::Error for MissingProcessedAssetReaderError {}

const MISSING_DEFAULT_SOURCE: &str =
    "A default AssetSource is required. Add one to `AssetSourceBuilders`";
