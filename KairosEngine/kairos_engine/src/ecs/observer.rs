//! Observers are a push-based tool for responding to [`Event`]s. The [`Observer`] component holds a [`System`] that runs whenever a matching [`Event`]
//! is triggered.
//!
//! See [`Event`] and [`Observer`] for in-depth documentation and usage examples.

mod centralized_storage;
mod runner;
mod system_param;

pub use centralized_storage::*;
pub use runner::*;
pub use system_param::*;
