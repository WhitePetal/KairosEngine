//! A collection of helper types and functions for working on macros within the Bevy ecosystem.

extern crate alloc;
extern crate proc_macro;

pub mod fq_std;
mod kairos_manifest;
mod label;
mod shape;

pub use kairos_manifest::*;
pub use label::*;
pub use shape::*;
