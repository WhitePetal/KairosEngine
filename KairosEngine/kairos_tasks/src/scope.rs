//! Scoped spawning: start many tasks, block until all of them complete.
//!
//! Mirrors `bevy_tasks::scope`.
//!
//! ## Planned API
//!
//! - `scope(|s: &Scope<'_, '_, T>| ...) -> T`:
//!   runs the closure on the current thread, then blocks until every task
//!   spawned inside the scope has finished, and returns the collected `T`.
//! - `Scope<'scope, 'env, T>`:
//!   - `spawn(future)` — schedule `future` onto the pool;
//!   - each spawn returns a handle that can be awaited from within the scope;
//!   - `'env` data is shared by reference, `'scope` data is moved by value;
//!   - the caller is allowed to `!Send`-share through the scope, so the
//!     synchronization used here (completion tracking, result collection)
//!     must be designed accordingly.
//!
//! ## Design notes
//!
//! - Keep the completion barrier cheap: a `SmallVec`-style list of
//!   completions plus one notification, not a mutex per task.
