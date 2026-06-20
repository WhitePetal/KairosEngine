use std::borrow::Cow;

use quote::quote;
use syn::{DeriveInput, Error, Result};

use proc_macro2::TokenStream as TokenStream2;

use crate::common::{member_as_idents, struct_fields};

pub fn derive(input: DeriveInput) -> Result<TokenStream2> {
    let ident = input.ident;
    let data = match input.data {
        syn::Data::Struct(data_struct) => data_struct,
        _ => {
            return Err(Error::new_spanned(
                ident,
                "derive(Bundle) does not support enums or unions",
            ));
        }
    };
    let (tys, field_members) = struct_fields(&data.fields);
    let fields_idents = member_as_idents(&field_members);
    let generics = add_additional_bounds_to_generic_params(input.generics);

    let dyn_bundle_code = gen_dynamic_tuple_impl(&ident, &generics, &field_members, &tys);
    let bundle_code = if tys.is_empty() {
        gen_unit_struct_tuple_impl(ident, &generics)
    } else {
        gen_tuple_impl(&ident, &generics, &field_members, &fields_idents, &tys)
    };
    let mut ts = dyn_bundle_code;
    ts.extend(bundle_code);
    Ok(ts)
}

fn add_additional_bounds_to_generic_params(mut generics: syn::Generics) -> syn::Generics {
    generics.type_params_mut().for_each(|tp| {
        tp.bounds
            .push(syn::TypeParamBound::Trait(make_component_trait_bound()));
    });
    generics
}

fn make_component_trait_bound() -> syn::TraitBound {
    syn::TraitBound {
        paren_token: None,
        modifier: syn::TraitBoundModifier::None,
        lifetimes: None,
        path: syn::parse_quote!(crate::ecs::component::Component),
    }
}

fn gen_dynamic_tuple_impl(
    ident: &syn::Ident,
    generics: &syn::Generics,
    field_members: &[syn::Member],
    tys: &[&syn::Type],
) -> TokenStream2 {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        unsafe impl #impl_generics crate::ecs::component_tuple::DynamicComponentTuple for #ident #ty_generics #where_clause {
            #[allow(non_camel_case_types)]
            fn has<__ecs__T: crate::ecs::component::Component>(&self) -> bool {
                false #(|| ::core::any::TypeId::of::<#tys>() == ::core::any::TypeId::of::<__ecs__T>())*
            }

            fn key(&self) -> ::core::option::Option<crate::ecs::component_tuple::ComponentTupleKey> {
                ::core::option::Option::Some(crate::ecs::component_tuple::ComponentTupleKey::from(::core::any::TypeId::of::<Self>()))
            }

            #[allow(non_camel_case_types)]
            fn with_ids<__ecs__T, __ecs__F: ::core::ops::FnOnce(&[::core::any::TypeId]) -> __ecs__T>(&self, f: __ecs__F) -> __ecs__T {
                <Self as crate::ecs::component_tuple::ComponentTuple>::with_static_ids(f)
            }

            fn type_infos(&self) -> Box<[crate::ecs::table::ComponentTypeInfo]> {
                <Self as crate::ecs::component_tuple::ComponentTuple>::with_static_type_info(|info| info.iter().copied().collect::<Box<[_]>>())
            }

            #[allow(clippy::forget_copy, clippy::forget_non_drop, non_camel_case_types)]
            unsafe fn put<__ecs__F: ::core::ops::FnMut(*mut u8, crate::ecs::table::ComponentTypeInfo)>(mut self, mut f: __ecs__F) {
                #(
                    f((&mut self.#field_members as *mut #tys).cast::<u8>(), crate::ecs::table::ComponentTypeInfo::of::<#tys>());
                    ::core::mem::forget(self.#field_members);
                )*
            }
        }
    }
}

fn gen_unit_struct_tuple_impl(ident: syn::Ident, generics: &syn::Generics) -> TokenStream2 {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        unsafe impl #impl_generics crate::ecs::component_tuple::ComponentTuple for #ident #ty_generics #where_clause {
            #[allow(non_camel_case_types)]
            fn with_static_ids<__ecs__T, __ecs__F: ::core::ops::FnOnce(&[::core::any::TypeId]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                f(&[])
            }

            #[allow(non_camel_case_types)]
            fn with_static_type_info<__ecs__T, __ecs__F: ::core::ops::FnOnce(&[crate::ecs::table::ComponentTypeInfo]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                f(&[])
            }

            #[allow(non_camel_case_types)]
            unsafe fn get<__ecs__F: ::core::ops::FnMut(crate::ecs::table::ComponentTypeInfo) -> ::core::option::Option<::core::ptr::NonNull<u8>>>(f: __ecs__F) -> ::core::result::Result<Self, crate::ecs::component::MissingComponent> {
                ::core::result::Result::Ok(Self {})
            }
        }
    }
}

fn gen_tuple_impl(
    ident: &syn::Ident,
    generics: &syn::Generics,
    field_members: &[syn::Member],
    field_idents: &[Cow<syn::Ident>],
    tys: &[&syn::Type],
) -> TokenStream2 {
    let num_tys = tys.len();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let with_static_ids_inner = quote! {
        {
            let mut tys = [#((::core::mem::align_of::<#tys>(), ::core::any::TypeId::of::<#tys>())), *];
            tys.sort_unstable_by(|x, y| {
                ::core::cmp::Ord::cmp(&x.0, &y.0).reverse().then(::core::cmp::Ord::cmp(&x.1, &y.1))
            });
            let mut ids = [::core::any::TypeId::of::<()>(); #num_tys];
            for (id, info) in ::core::iter::Iterator::zip(ids.iter_mut(), tys.iter()) {
                *id = info.1;
            }
            ids
        }
    };
    let with_static_ids_body = if generics.params.is_empty() {
        quote! {
            static ELEMENTS: ::std::sync::LazyLock<[::core::any::TypeId; #num_tys]> = ::std::sync::LazyLock::new(|| {
                #with_static_ids_inner
            });
            f(&*ELEMENTS)
        }
    } else {
        quote! {
            f(&#with_static_ids_inner)
        }
    };
    quote! {
        unsafe impl #impl_generics crate::ecs::component_tuple::ComponentTuple for #ident #ty_generics #where_clause {
            #[allow(non_camel_case_types)]
            fn with_static_ids<__ecs__T, __ecs__F: ::core::ops::FnOnce(&[::core::any::TypeId]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                #with_static_ids_body
            }

            #[allow(non_camel_case_types)]
            fn with_static_type_info<__ecs__T, __ecs__F: ::core::ops::FnOnce(&[crate::ecs::table::ComponentTypeInfo]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                let mut infos: [crate::ecs::table::ComponentTypeInfo; #num_tys] = [#(crate::ecs::table::ComponentTypeInfo::of::<#tys>()), *];
                infos.sort_unstable();
                f(&infos)
            }

            #[allow(non_camel_case_types, unsafe_op_in_unsafe_fn)]
            unsafe fn get<__ecs__F: ::core::ops::FnMut(crate::ecs::table::ComponentTypeInfo) -> ::core::option::Option<::core::ptr::NonNull<u8>>>(
                mut f: __ecs__F,
            ) -> ::core::result::Result<Self, crate::ecs::component::MissingComponent> {
                #(
                    let #field_idents = f(crate::ecs::table::ComponentTypeInfo::of::<#tys>())
                        .ok_or_else(crate::ecs::component::MissingComponent::new::<#tys>)?
                        .cast::<#tys>()
                        .as_ptr();
                )*
                ::core::result::Result::Ok(Self { #( #field_members: #field_idents.read(), )* })
            }
        }
    }
}
