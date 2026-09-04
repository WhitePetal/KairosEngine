//! Thread-local collections for lock-free concurrent aggregation.
//!
//! Mirrors `bevy_tasks::parallel` (`Parallel<T>`).
//!
//! ## Migration target
//!
//! The engine crate currently carries a port of this type at
//! `kairos_engine/src/parallel_queue.rs` — `Parallel<T>` with
//! `scope_or`, `borrow_local_mut_or`, `scope`, `drain`, `drain_into`, ...
//!
//! When wiring `kairos_tasks` into the engine:
//!
//! 1. move that implementation here (`git mv`, then expose from this module
//!    and re-export from the crate root);
//! 2. update engine call sites from `crate::parallel_queue::Parallel` to
//!    `kairos_tasks::parallel::Parallel`;
//! 3. delete `kairos_engine/src/parallel_queue.rs` and its `lib.rs` entry.
//!
//! ## Planned items
//!
//! - `Parallel<T>` backed by `thread_local::ThreadLocal`, keeping the current
//!   API surface (per-thread mutation + `drain*`/`iter_mut` collection).
