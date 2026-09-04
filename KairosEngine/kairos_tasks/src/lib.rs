//! # kairos_tasks
//!
//! Asynchronous task infrastructure for KairosEngine, modeled on
//! [bevy_tasks](https://docs.rs/bevy_tasks/latest/bevy_tasks/).
//!
//! ## Position in the workspace
//!
//! This crate sits at the **bottom of the dependency graph**:
//!
//! - It must never depend on `kairos_engine` or any other engine crate.
//! - Planned consumers: `kairos_engine::ecs` (multi-threaded schedule
//!   executor) and `kairos_engine::asset_loader` (async asset loading).
//! - Keep dependencies minimal — like bevy_tasks, prefer light primitives
//!   over pulling in a full async runtime.
//!
//! This is the first foundation crate extracted following the workspace-split
//! plan in `docs/ai/rust-crate-facade-and-workspace-split.md`.
//!
//! ## Modules
//!
//! | Module | Mirrors `bevy_tasks` | Contents |
//! |--------|----------------------|----------|
//! | [`task`] | `task` | Awaitable task handles produced by the pool |
//! | [`task_pool`] | `task_pool` | `TaskPool` / `TaskPoolBuilder` + executor backend |
//! | [`scope`] | `scope` | Spawn many tasks, block until all complete |
//! | [`slice`] | `slice` | Parallel iteration over slices (`ParallelSlice`) |
//! | [`parallel`] | `parallel` | Thread-local `Parallel<T>` aggregation |

pub mod parallel;
pub mod scope;
pub mod slice;
pub mod task;
pub mod task_pool;

// TODO: once implemented, re-export the public surface from the crate root
// so consumers write `kairos_tasks::TaskPool` instead of deep paths:
//
// pub use task::Task;
// pub use task_pool::{TaskPool, TaskPoolBuilder};
