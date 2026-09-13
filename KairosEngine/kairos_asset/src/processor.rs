//! Asset processing: the in-memory view of the processed asset space.
//!
//! The processor itself — the background task, the [`Process`](crate::meta)
//! trait family, the transaction log, and the gated reader — lands in later
//! slices. This module carries the base those slices build on:
//! [`ProcessorAssetInfos`], the per-asset dependency graph, and [`ProcessStatus`],
//! the three states an asset's processed output can be in.
//!
//! [`ProcessedInfo`](crate::meta::ProcessedInfo) recorded here is used **only**
//! to decide whether a reprocessing pass can be skipped: if the asset's own hash
//! and every dependency's `full_hash` are unchanged, the processor leaves the
//! existing output alone. It is never a readiness signal — a reader waiting on a
//! processed asset waits on the gated reader, which does not consult these
//! hashes.

mod info;

// Re-exported for the crate's own use (tests now, the `AssetProcessor` later).
#[allow(unused_imports)]
pub(crate) use info::{ProcessStatus, ProcessorAssetInfos};
