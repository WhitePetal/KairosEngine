#![expect(
    unsafe_op_in_unsafe_fn,
    reason = "See #11590. To be removed once all applicable unsafe code has an unsafe block with a safety comment."
)]

//! Types for handling [`Bundle`]s.
//!
//! This module contains the [`Bundle`] trait and some other helper types.

mod info;

pub use info::*;
