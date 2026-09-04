//! Parallel iteration over slices.
//!
//! Mirrors `bevy_tasks::slice` (`ParallelSlice` / `ParallelSliceMut`).
//!
//! ## Planned API
//!
//! - `ParallelSlice<T>` on `&[T]`:
//!   - `par_chunk_map(pool, mapper)` / `par_splat_map(pool, mapper)` and their
//!     `_fut` (async mapper) variants, each returning `Vec<R>`.
//! - `ParallelSliceMut<T>` on `&mut [T]`:
//!   - `par_chunk_map_mut(pool, mapper)` — in-place transform.
//!
//! ## Design notes
//!
//! - Chunking: split the slice into `pool.thread_num()` contiguous chunks so
//!   every worker writes to disjoint memory with no locks or atomics.
//! - `par_chunk_map` runs one mapper per *chunk* (map a sub-slice to one
//!   value); `par_splat_map` runs one mapper per *element*, where each mapper
//!   may produce multiple outputs.
