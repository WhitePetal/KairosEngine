use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for Bundle trait.
///
/// A Bundle is a collection of components that can be
/// spawned together on an entity.
///
/// # Example
///
/// ```ignore
/// #[derive(Bundle)]
/// struct PlayerBundle {
///     transform: Transform,
///     health: Health,
///     name: Name,
/// }
/// ```
#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    let _input = parse_macro_input!(input as DeriveInput);
    // TODO: implement Bundle derive
    todo!("implement Bundle derive macro")
}
