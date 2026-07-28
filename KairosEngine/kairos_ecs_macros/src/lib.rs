extern crate proc_macro;

mod common;
mod component;
mod tuple;
mod resource;

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

#[proc_macro_derive(Component, attributes(component, require))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(component::derive_component(&mut ast))
}


#[proc_macro_derive(Resource, attributes(component, require))]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    TokenStream::from(resource::derive_resource(&mut ast))
}
