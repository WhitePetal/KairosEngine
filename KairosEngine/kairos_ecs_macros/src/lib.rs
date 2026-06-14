extern crate proc_macro;

mod tuple;
mod common;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derive macro for Bundle trait.
///
/// A Bundle is a collection of components that can be
/// spawned together on an entity.
///
/// # Example
///
/// ```ignore
/// #[derive(ComponentTuple)]
/// struct PlayerBundle {
///     transform: Transform,
///     health: Health,
///     name: Name,
/// }
/// ```
#[proc_macro_derive(ComponentTuple)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match tuple::derive(input) {
        Ok(ts) => ts,
        Err(e) => e.to_compile_error(),
    }
    .into()
}
