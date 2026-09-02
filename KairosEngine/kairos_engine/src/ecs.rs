pub mod archetype;
pub mod batching;
pub mod bundle;
pub mod change_detection;
pub mod component;
pub mod entity;
pub mod entity_disabling;
pub mod error;
pub mod event;
pub mod hierarchy;
pub mod intern;
pub mod label;
pub mod lifecycle;
pub mod message;
pub mod name;
pub mod never;
pub mod observer;
pub mod query;
pub mod relationship;
pub mod resource;
pub mod schedule;
pub mod storage;
pub mod system;
pub mod template;
pub mod traversal;
pub mod world;

/// Exports used by macros.
///
/// These are not meant to be used directly and are subject to breaking changes.
#[doc(hidden)]
pub mod __macro_exports {
    // Cannot directly use `alloc::vec::Vec` in macros, as a crate may not have
    // included `extern crate alloc;`. This re-export ensures we have access
    // to `Vec` in `no_std` and `std` contexts.
    pub use crate::debug::DebugCheckedUnwrap;
    pub use crate::ptr::{MovingPtr, OwningPtr, deconstruct_moving_ptr};
}
