//! Shared thread pool that owns worker threads and schedules spawned tasks.
//!
//! Mirrors `bevy_tasks::task_pool` (`TaskPool`, `TaskPoolBuilder`).
//!
//! ## Planned API
//!
//! - `TaskPoolBuilder`:
//!   - `num_threads(n)`, `stack_size(bytes)`, `thread_name(name)`,
//!     `on_thread_spawn(callback)`, then `build() -> TaskPool`.
//! - `TaskPool`:
//!   - `new()` / `Default` — one worker per logical core
//!     (`std::thread::available_parallelism`);
//!   - `thread_num()` — current worker count;
//!   - `spawn(future) -> Task<T>` — schedule onto the pool's executor;
//!   - `spawn_on_external_thread(future) -> Task<T>` — caller-driven task,
//!     polled via the executor `tick()` mechanism;
//!   - entry points for [`crate::scope`] and [`crate::slice`].
//!
//! ## Design notes
//!
//! - CPU-bound worker model, like bevy_tasks: tasks are `Send + 'static`;
//!   no async I/O runtime is provided here.
//! - Async I/O remains the job of `kairos_engine::asset_loader` (tokio).
//!   Decide later whether kairos_tasks only *schedules futures* (which may
//!   internally use tokio) or should also expose its own channel types.
//! - The executor backend and its `tick()` live here but stay private;
//!   only the pool API is public.
