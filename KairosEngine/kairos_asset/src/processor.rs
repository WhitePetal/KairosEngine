//! Asset processing: the [`Process`] trait family and the processor registry.
//!
//! A processor turns a source asset into a processed asset with a different
//! representation — a texture compressed, a mesh packed, a format converted.
//! This module carries both the extension points a processor is written against
//! and the background body ([`AssetProcessor`]) that drives them:
//!
//! - [`Process`] — the low-level "read the source, write the processed bytes"
//!   trait; [`LoadTransformAndSave`] is the high-level implementation that
//!   loads with an [`AssetLoader`](crate::AssetLoader), transforms through an
//!   [`AssetTransformer`], and saves through an [`AssetSaver`].
//! - [`ProcessContext`] — the scoped view a processor is handed: it reads the
//!   source asset (recording every asset value it touches as a process
//!   dependency) and exposes the reader for the raw bytes.
//! - [`ErasedProcessor`] — the type-erased handle the registry stores and the
//!   processor resolves `.meta` names against.
//! - [`Processors`] — the registration surface: name, short-name, and
//!   default-by-extension lookups.
//!
//! It also carries the base the processor body builds on: [`ProcessorAssetInfos`],
//! the per-asset dependency graph, and [`ProcessStatus`], the three states an
//! asset's processed output can be in.
//!
//! [`ProcessedInfo`](crate::meta::ProcessedInfo) recorded here is used **only**
//! to decide whether a reprocessing pass can be skipped: if the asset's own hash
//! and every dependency's `full_hash` are unchanged, the processor leaves the
//! existing output alone. It is never a readiness signal — a reader waiting on a
//! processed asset waits on the gated reader, which does not consult these
//! hashes.
//!
//! Processor names follow the [`processor_name`](crate::meta::processor_name)
//! convention (`std::any::type_name`), matching loaders; kairos deliberately has
//! no `TypePath`.

mod asset_processor;
mod info;
mod log;
mod process;
mod registry;
mod saver;
mod transformer;

pub use asset_processor::{
    AssetProcessor, AssetProcessorData, InitializeError, ProcessResult, ProcessorState,
};
// The gated reader consumes `ProcessStatus`, and `AssetSource::gate_on_processor`
// takes the `ProcessingState`.
pub(crate) use asset_processor::ProcessingState;
pub use log::{
    FileTransactionLogFactory, LogEntry, LogEntryError, ProcessorTransactionLog,
    ProcessorTransactionLogFactory, ReadLogError, SetTransactionLogFactoryError, ValidateLogError,
};
pub(crate) use info::ProcessStatus;
// The crate's tests build a `ProcessorAssetInfos` directly.
#[allow(unused_imports)]
pub(crate) use info::ProcessorAssetInfos;
pub use registry::{GetProcessorError, Processors};

pub use process::{
    ErasedProcessor, LoadTransformAndSave, LoadTransformAndSaveSettings, MetaTypePathKind, Process,
    ProcessContext, ProcessError,
};
pub use saver::{AssetSaver, ErasedAssetSaver, SavedAsset};
pub use transformer::{AssetTransformer, IdentityAssetTransformer, TransformedAsset};
