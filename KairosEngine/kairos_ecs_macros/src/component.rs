use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub fn derive_component(ast: &mut DeriveInput) -> TokenStream {
    let ident = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();

    quote! {
        impl #impl_generics crate::ecs::component::Component for #ident #ty_generics #where_clause {}
    }
}
