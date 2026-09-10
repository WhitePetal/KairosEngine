use kairos_ecs_macro_logic::component::{DeriveComponent, StorageAttribute, StorageTy};
use kairos_macro_utils::fq_std::FQOption;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Path};

pub fn derive_resource(ast: &mut DeriveInput) -> TokenStream {
    let kairos_ecs: Path = crate::kairos_ecs_path();
    let mut derive_component = match DeriveComponent::parse(ast, StorageAttribute::Disallowed) {
        Ok(value) => value,
        Err(e) => return e.into_compile_error(),
    };

    let struct_name = &ast.ident;
    let (_, type_generics, _) = &ast.generics.split_for_impl();

    // We add the component_id existence check here to avoid recursive init during required components initialization.
    derive_component.additional_requires.push(quote! {
        let resource_component_id = if let #FQOption::Some(id) = required_components.components_registrator().component_id::<#struct_name #type_generics>() {
            id
        } else {
            required_components.components_registrator().register_component::<#struct_name #type_generics>()
        };
        required_components.register_required::<#kairos_ecs::resource::IsResource>(move || #kairos_ecs::resource::IsResource::new(resource_component_id));
    });

    let component_impl =
        match derive_component.impl_component(ast, &kairos_ecs, StorageTy::SparseSet) {
            Ok(value) => value,
            Err(err) => err.into_compile_error(),
        };

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    quote! {
        #component_impl
        impl #impl_generics #kairos_ecs::resource::Resource for #struct_name #type_generics #where_clause {
        }
    }
}
