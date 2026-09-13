//! Asset sources: named roots that asset paths resolve against.
//!
//! An [`AssetSource`] owns one [`AssetReader`](crate::io::AssetReader), an
//! optional [`AssetWriter`](crate::io::AssetWriter), an optional processed
//! reader and writer, and the watcher and event-channel slots the processor and
//! hot-reload tracks consume. Sources are named by an [`AssetSourceId`]:
//! [`Default`](AssetSourceId::Default) is the unnamed root that plain paths
//! resolve against, while a [`Name`](AssetSourceId::Name) is written
//! `name://path` in an [`AssetPath`](crate::path::AssetPath).
//!
//! Sources are described up front as [`AssetSourceBuilder`]s inside an
//! [`AssetSourceBuilders`] resource and frozen into [`AssetSources`] once they
//! are known. The default source is rooted at the process working directory
//! rather than bevy's `assets` folder, so today's cwd-relative literals keep
//! resolving to the same files (ADR 0004).

use std::{
    fmt::{self, Display},
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use atomicow::CowArc;

use kairos_collections::FixedHashMap as HashMap;
use kairos_ecs::resource::Resource;
use thiserror::Error;
use tracing::warn;

use crate::io::{
    AssetSourceEvent, AssetWatcher, ErasedAssetReader, ErasedAssetWriter,
    file::{FileAssetReader, FileAssetWriter},
};
use crate::processor::ProcessingState;

use super::processor_gated::ProcessorGatedReader;

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
/// source can be rebuilt (for example when a watcher restarts). Readers and
/// writers are populated by
/// [`platform_default`](AssetSourceBuilder::platform_default); the watcher slot
/// stays empty until the hot-reload watcher lands.
pub struct AssetSourceBuilder {
    /// Builds the unprocessed reader.
    pub reader: Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>,
    /// Builds the unprocessed writer, if any.
    pub writer: Option<Box<dyn FnMut(bool) -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync>>,
    /// Builds the unprocessed watcher, if any.
    pub watcher: Option<
        Box<
            dyn FnMut(async_channel::Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
                + Send
                + Sync,
        >,
    >,
    /// Builds the processed reader, if any.
    pub processed_reader: Option<Box<dyn FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync>>,
    /// Builds the processed writer, if any.
    pub processed_writer:
        Option<Box<dyn FnMut(bool) -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync>>,
    /// Builds the processed watcher, if any.
    pub processed_watcher: Option<
        Box<
            dyn FnMut(async_channel::Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
                + Send
                + Sync,
        >,
    >,
    /// The warning to log when watching is on but the unprocessed slot has no
    /// watcher.
    pub watch_warning: Option<&'static str>,
    /// The warning to log when processed watching is on but the processed slot
    /// has no watcher.
    pub processed_watch_warning: Option<&'static str>,
    /// The unprocessed subtree to skip because it holds this source's processed
    /// output.
    ///
    /// Only set when the processed root sits inside the unprocessed root (which
    /// ADR 0004's working-directory root makes the norm). [`platform_default`]
    /// derives it; the processor consults it so processed output is never
    /// mistaken for a source asset.
    ///
    /// [`platform_default`]: AssetSourceBuilder::platform_default
    pub unprocessed_exclude: Option<PathBuf>,
}

impl AssetSourceBuilder {
    /// Creates a builder whose reader is produced by `reader`.
    pub fn new(reader: impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + 'static) -> Self {
        Self {
            reader: Box::new(reader),
            writer: None,
            watcher: None,
            processed_reader: None,
            processed_writer: None,
            processed_watcher: None,
            watch_warning: None,
            processed_watch_warning: None,
            unprocessed_exclude: None,
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

    /// Sets the unprocessed watcher constructor.
    pub fn with_watcher(
        mut self,
        watcher: impl FnMut(async_channel::Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.watcher = Some(Box::new(watcher));
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

    /// Sets the processed writer constructor.
    pub fn with_processed_writer(
        mut self,
        writer: impl FnMut(bool) -> Option<Box<dyn ErasedAssetWriter>> + Send + Sync + 'static,
    ) -> Self {
        self.processed_writer = Some(Box::new(writer));
        self
    }

    /// Sets the processed watcher constructor.
    pub fn with_processed_watcher(
        mut self,
        watcher: impl FnMut(async_channel::Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.processed_watcher = Some(Box::new(watcher));
        self
    }

    /// Sets the warning logged when watching is on but there is no unprocessed
    /// watcher.
    pub fn with_watch_warning(mut self, warning: &'static str) -> Self {
        self.watch_warning = Some(warning);
        self
    }

    /// Sets the warning logged when processed watching is on but there is no
    /// processed watcher.
    pub fn with_processed_watch_warning(mut self, warning: &'static str) -> Self {
        self.processed_watch_warning = Some(warning);
        self
    }

    /// Excludes `path` and everything under it from the unprocessed scan.
    ///
    /// The path is relative to this source's unprocessed root and is a single
    /// top-level component (so the processed root and the transaction log beside
    /// it share one exclusion). [`platform_default`] sets this automatically when
    /// the processed root is nested under the unprocessed root.
    ///
    /// [`platform_default`]: AssetSourceBuilder::platform_default
    pub(crate) fn with_unprocessed_exclude(mut self, path: impl Into<PathBuf>) -> Self {
        self.unprocessed_exclude = Some(path.into());
        self
    }

    /// Builds the [`AssetSource`] for `id`.
    ///
    /// When `watch` is true the source watches for changes to unprocessed assets;
    /// when `watch_processed` is true it watches for changes to processed assets.
    /// Watching needs a configured watcher constructor: without one the matching
    /// event receiver is left empty.
    pub fn build(
        &mut self,
        id: AssetSourceId<'static>,
        watch: bool,
        watch_processed: bool,
    ) -> AssetSource {
        let reader = self.reader.as_mut()();
        let writer = self.writer.as_mut().and_then(|writer| writer(false));
        let processed_writer = self
            .processed_writer
            .as_mut()
            .and_then(|writer| writer(true));
        let mut source = AssetSource {
            id: id.clone(),
            reader,
            writer,
            processed_reader: self
                .processed_reader
                .as_mut()
                .map(|reader| reader())
                .map(Into::<Arc<_>>::into),
            ungated_processed_reader: None,
            processed_writer,
            watcher: None,
            processed_watcher: None,
            event_receiver: None,
            processed_event_receiver: None,
            unprocessed_exclude: self.unprocessed_exclude.clone(),
        };

        if watch {
            let (sender, receiver) = async_channel::unbounded();
            match self.watcher.as_mut().and_then(|watcher| watcher(sender)) {
                Some(watcher) => {
                    source.watcher = Some(watcher);
                    source.event_receiver = Some(receiver);
                }
                None => {
                    if let Some(warning) = self.watch_warning {
                        warn!("{id} does not have an AssetWatcher configured. {warning}");
                    }
                }
            }
        }

        if watch_processed {
            let (sender, receiver) = async_channel::unbounded();
            match self
                .processed_watcher
                .as_mut()
                .and_then(|watcher| watcher(sender))
            {
                Some(watcher) => {
                    source.processed_watcher = Some(watcher);
                    source.processed_event_receiver = Some(receiver);
                }
                None => {
                    if let Some(warning) = self.processed_watch_warning {
                        warn!("{id} does not have a processed AssetWatcher configured. {warning}");
                    }
                }
            }
        }

        source
    }

    /// A builder rooted at `path` (relative to the process working directory),
    /// with an optional processed root at `processed_path`.
    ///
    /// The readers and writers are [`FileAssetReader`]s and [`FileAssetWriter`]s.
    /// The unprocessed writer is always configured so the source tree is
    /// writable; the processed reader and writer are added only when
    /// `processed_path` is given, which is also what makes
    /// [`AssetSource::should_process`] true. Both watcher slots are configured
    /// with the platform-default watcher and a 300 ms debounce window, so a
    /// source built with `watch` (or `watch_processed`) actually watches; the
    /// matching warning is logged when no watcher backend exists.
    pub fn platform_default(path: &str, processed_path: Option<&str>) -> Self {
        let reader_path = path.to_owned();
        let writer_path = path.to_owned();
        let mut builder = Self::new(move || {
            Box::new(FileAssetReader::new(&reader_path)) as Box<dyn ErasedAssetReader>
        })
        .with_writer(move |create_root| {
            Some(Box::new(FileAssetWriter::new(&writer_path, create_root))
                as Box<dyn ErasedAssetWriter>)
        })
        .with_watcher(AssetSource::get_default_watcher(
            path.to_owned(),
            Duration::from_millis(300),
        ))
        .with_watch_warning(AssetSource::get_default_watch_warning());
        if let Some(processed_path) = processed_path {
            // When the processed root lives under the source root, the processor
            // must not walk into it: ADR 0004 roots the default source at the
            // working directory, and `imported_assets/Default` sits inside it.
            // Excluding the processed path's top-level component also skips the
            // transaction log written beside `Default`.
            if let Some(exclude) = processed_subtree_exclude(path, processed_path) {
                builder = builder.with_unprocessed_exclude(exclude);
            }
            let processed_reader_path = processed_path.to_owned();
            let processed_writer_path = processed_path.to_owned();
            builder = builder
                .with_processed_reader(move || {
                    Box::new(FileAssetReader::new(&processed_reader_path))
                        as Box<dyn ErasedAssetReader>
                })
                .with_processed_writer(move |create_root| {
                    Some(
                        Box::new(FileAssetWriter::new(&processed_writer_path, create_root))
                            as Box<dyn ErasedAssetWriter>,
                    )
                })
                .with_processed_watcher(AssetSource::get_default_watcher(
                    processed_path.to_owned(),
                    Duration::from_millis(300),
                ))
                .with_processed_watch_warning(AssetSource::get_default_watch_warning());
        }
        builder
    }
}

/// The unprocessed subtree to skip when the processed root is nested under the
/// unprocessed root.
///
/// Returns the processed path's first component relative to the unprocessed
/// root — the whole `imported_assets` directory rather than just
/// `imported_assets/Default`, so the transaction log beside `Default` is skipped
/// too — or [`None`] when the roots do not overlap.
fn processed_subtree_exclude(unprocessed: &str, processed: &str) -> Option<PathBuf> {
    let unprocessed = Path::new(unprocessed);
    let relative = if unprocessed.as_os_str().is_empty() {
        Path::new(processed)
    } else {
        Path::new(processed).strip_prefix(unprocessed).ok()?
    };
    relative
        .components()
        .next()
        .map(|component| PathBuf::from(component.as_os_str()))
}

/// Holds the [`AssetSourceBuilder`]s registered before the sources are frozen
/// into [`AssetSources`], plus the default source's builder.
///
/// This is the resource the engine fills in during startup and then builds
/// once; keeping the builders separate means a source can still be replaced
/// before anything reads from it.
#[derive(Resource, Default)]
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
    /// `watch` and `watch_processed` are passed through to every source; see
    /// [`AssetSourceBuilder::build`].
    ///
    /// # Panics
    ///
    /// Panics if no default source has been registered: every path that does
    /// not name a source resolves through the default, so there is no sensible
    /// fallback.
    pub fn build_sources(&mut self, watch: bool, watch_processed: bool) -> AssetSources {
        let mut sources = HashMap::with_capacity(self.sources.len());
        for (id, source) in &mut self.sources {
            let source = source.build(AssetSourceId::Name(id.clone()), watch, watch_processed);
            sources.insert(id.clone(), source);
        }

        AssetSources {
            sources,
            default: self
                .default
                .as_mut()
                .map(|source| source.build(AssetSourceId::Default, watch, watch_processed))
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

    /// The sources that should be processed.
    pub fn iter_processed(&self) -> impl Iterator<Item = &AssetSource> {
        self.iter().filter(|source| source.should_process())
    }

    /// The sources that should be processed, mutably.
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

    /// Gates every processed source's processed reader on `processing_state` so
    /// that readers wait for the processor's output. See
    /// [`AssetSource::gate_on_processor`].
    pub(crate) fn gate_on_processor(&mut self, processing_state: Arc<ProcessingState>) {
        for source in self.iter_processed_mut() {
            source.gate_on_processor(processing_state.clone());
        }
    }
}

/// One resolvable asset root: its id plus the reader, writer, processed reader
/// and writer, watcher, and event-channel slots.
pub struct AssetSource {
    id: AssetSourceId<'static>,
    reader: Box<dyn ErasedAssetReader>,
    writer: Option<Box<dyn ErasedAssetWriter>>,
    processed_reader: Option<Arc<dyn ErasedAssetReader>>,
    /// The ungated version of `processed_reader`.
    ///
    /// The processor reads processed assets through this so it can initialize
    /// without waiting on itself. Nothing populates it yet: the gated
    /// `processed_reader` and this ungated copy are wired by the processor track.
    ungated_processed_reader: Option<Arc<dyn ErasedAssetReader>>,
    processed_writer: Option<Box<dyn ErasedAssetWriter>>,
    watcher: Option<Box<dyn AssetWatcher>>,
    processed_watcher: Option<Box<dyn AssetWatcher>>,
    event_receiver: Option<async_channel::Receiver<AssetSourceEvent>>,
    processed_event_receiver: Option<async_channel::Receiver<AssetSourceEvent>>,
    /// The unprocessed subtree that holds this source's processed output and must
    /// not be scanned as source content. See
    /// [`AssetSourceBuilder::unprocessed_exclude`].
    unprocessed_exclude: Option<PathBuf>,
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

    /// The ungated processed reader, if one is configured.
    ///
    /// The processor consumes this to seed itself without waiting on its own
    /// output; nothing else should read through it. It is populated by
    /// [`AssetSource::gate_on_processor`], which moves the original reader here
    /// and installs the gated copy as `processed_reader`.
    #[inline]
    pub(crate) fn ungated_processed_reader(&self) -> Option<&dyn ErasedAssetReader> {
        self.ungated_processed_reader.as_deref()
    }

    /// The processed writer, if one is configured.
    #[inline]
    pub fn processed_writer(
        &self,
    ) -> Result<&dyn ErasedAssetWriter, MissingProcessedAssetWriterError> {
        self.processed_writer
            .as_deref()
            .ok_or_else(|| MissingProcessedAssetWriterError(self.id.clone_owned()))
    }

    /// The unprocessed watcher, if this source is watching for changes.
    #[inline]
    pub fn watcher(&self) -> Option<&dyn AssetWatcher> {
        self.watcher.as_deref()
    }

    /// The processed watcher, if this source is watching processed assets.
    #[inline]
    pub fn processed_watcher(&self) -> Option<&dyn AssetWatcher> {
        self.processed_watcher.as_deref()
    }

    /// The unprocessed source-event receiver, if this source is watching for
    /// changes.
    #[inline]
    pub fn event_receiver(&self) -> Option<&async_channel::Receiver<AssetSourceEvent>> {
        self.event_receiver.as_ref()
    }

    /// The processed source-event receiver, if this source is watching processed
    /// assets.
    #[inline]
    pub fn processed_event_receiver(&self) -> Option<&async_channel::Receiver<AssetSourceEvent>> {
        self.processed_event_receiver.as_ref()
    }

    /// Whether this source's assets should be processed.
    ///
    /// A source is processed once it has somewhere to write processed output;
    /// a processed reader alone does not make it a processor source.
    #[inline]
    pub fn should_process(&self) -> bool {
        self.processed_writer.is_some()
    }

    /// The unprocessed subtree that holds this source's processed output, if any.
    ///
    /// The path is relative to the unprocessed root; the processor skips it so
    /// processed output is never reprocessed as source content.
    #[inline]
    pub fn unprocessed_exclude(&self) -> Option<&Path> {
        self.unprocessed_exclude.as_deref()
    }

    /// Whether `path` (relative to the unprocessed root) lies inside the
    /// processed subtree that must not be scanned as source content.
    #[inline]
    pub(crate) fn is_excluded_from_unprocessed(&self, path: &Path) -> bool {
        self.unprocessed_exclude
            .as_deref()
            .is_some_and(|exclude| path.starts_with(exclude))
    }

    /// The warning to log for the current platform when watching is enabled but
    /// no watcher could be built.
    pub fn get_default_watch_warning() -> &'static str {
        #[cfg(target_os = "android")]
        return "Android does not currently support watching assets.";
        #[cfg(all(not(target_os = "android"), not(feature = "file_watcher")))]
        return "Consider enabling the `file_watcher` feature.";
        #[cfg(all(not(target_os = "android"), feature = "file_watcher"))]
        return "Consider adding the asset root under the working directory.";
    }

    /// A builder function for the platform's default [`AssetWatcher`]. `path` is
    /// the relative path to the asset root and `file_debounce_wait_time` is the
    /// window used to debounce duplicate events: larger windows reduce
    /// duplicates but increase the delay before a change is processed, while a
    /// window that is too small can surface an event before the filesystem has
    /// applied the change.
    ///
    /// Returns [`None`] when this platform (or build) has no watching backend,
    /// or when the root does not exist. A failure to create a watcher is logged
    /// and degrades to [`None`] rather than panicking, so watching stays
    /// best-effort.
    #[cfg_attr(
        any(not(feature = "file_watcher"), target_os = "android"),
        expect(
            unused_variables,
            reason = "`path` and `file_debounce_wait_time` are unused when there is no backend"
        )
    )]
    pub fn get_default_watcher(
        path: String,
        file_debounce_wait_time: Duration,
    ) -> impl FnMut(async_channel::Sender<AssetSourceEvent>) -> Option<Box<dyn AssetWatcher>>
    + Send
    + Sync
    {
        move |sender: async_channel::Sender<AssetSourceEvent>| {
            #[cfg(all(feature = "file_watcher", not(target_os = "android")))]
            {
                let full_path = super::file::get_base_path().join(path.clone());
                if !full_path.exists() {
                    warn!(
                        "Skip creating file watcher because path {full_path:?} does not exist."
                    );
                    return None;
                }
                match super::file::FileWatcher::new(
                    full_path.clone(),
                    sender,
                    file_debounce_wait_time,
                ) {
                    Ok(watcher) => Some(Box::new(watcher)),
                    Err(error) => {
                        warn!("Failed to create file watcher from path {full_path:?}: {error:?}");
                        None
                    }
                }
            }
            #[cfg(any(not(feature = "file_watcher"), target_os = "android"))]
            {
                None
            }
        }
    }

    /// Wraps this source's processed reader in a `ProcessorGatedReader`,
    /// moving the original into `ungated_processed_reader` for the processor's
    /// own reads.
    ///
    /// A source with no processed reader is left alone. Call this at most once
    /// per source (the processor does, in [`AssetProcessor::new`]): gating an
    /// already-gated reader would move the gated copy into
    /// `ungated_processed_reader` and leave the processor waiting on itself.
    ///
    /// [`AssetProcessor::new`]: crate::processor::AssetProcessor::new
    pub(crate) fn gate_on_processor(&mut self, processing_state: Arc<ProcessingState>) {
        if let Some(reader) = self.processed_reader.take() {
            let id = self.id();
            let gated = Arc::new(ProcessorGatedReader::new(
                id,
                reader.clone(),
                processing_state,
            ));
            self.ungated_processed_reader = Some(reader);
            self.processed_reader = Some(gated);
        }
    }
}

/// Returned by [`AssetSources::get`] when no source has the requested id.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not exist")]
pub struct MissingAssetSourceError(pub AssetSourceId<'static>);

/// Returned by [`AssetSource::writer`] when the source has no writer.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not have an AssetWriter.")]
pub struct MissingAssetWriterError(pub AssetSourceId<'static>);

/// Returned by [`AssetSource::processed_reader`] when the source has no
/// processed reader.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not have a processed AssetReader.")]
pub struct MissingProcessedAssetReaderError(pub AssetSourceId<'static>);

/// Returned by [`AssetSource::processed_writer`] when the source has no
/// processed writer.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("Asset Source '{0}' does not have a processed AssetWriter.")]
pub struct MissingProcessedAssetWriterError(pub AssetSourceId<'static>);

const MISSING_DEFAULT_SOURCE: &str =
    "A default AssetSource is required. Add one to `AssetSourceBuilders`";
