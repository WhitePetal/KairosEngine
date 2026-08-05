//! `#[derive(Resource)]` 的展开逻辑。

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Result};

/// 生成 `#[derive(Resource)]` 展开后的代码。
///
/// `Resource` 是 `Component` 的子 trait,因此这里同时生成 `Component` 与
/// `Resource` 两个 impl,`#[derive(Resource)]` 可独立使用。
pub fn derive_resource_impl(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics ::kairos_engine::ecs::component::Component for #ident #ty_generics #where_clause {
            const STORAGE_TYPE: ::kairos_engine::ecs::component::StorageType =
                ::kairos_engine::ecs::component::StorageType::Table;
            type Mutability = ::kairos_engine::ecs::component::Mutable;
        }
        impl #impl_generics ::kairos_engine::ecs::resource::Resource for #ident #ty_generics #where_clause {}
    })
}
