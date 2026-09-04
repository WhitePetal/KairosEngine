//! Awaitable task handles produced by the task pool.
//!
//! Mirrors `bevy_tasks::task` (`Task<T>`).
//!
//! ## Planned items
//!
//! - `Task<T>`: a `Send` future returned by `TaskPool::spawn` /
//!   `TaskPool::spawn_on_external_thread` that:
//!   - can be polled *after* completion (the output is stored internally),
//!     so it doubles as a shared completion signal;
//!   - reports `is_finished()` without polling the inner future;
//!   - aborts / detaches the underlying work when dropped, depending on the
//!     chosen backend semantics.
//! - A small `unsafe`-free bridge between the executor's internal task type
//!   and the public `Task<T>` wrapper. All `unsafe` (if any) stays here.
//!
//! ## Open decisions
//!
//! - Backend: wrap an executor crate (async-executor / async-task style), or
//!   implement the future queue on `std` only?
//! - Abort semantics: cooperative cancellation (bevy-style) vs. forcible
//!   cancellation on drop?
