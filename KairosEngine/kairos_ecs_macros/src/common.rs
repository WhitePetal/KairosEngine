use std::borrow::Cow;

use proc_macro2::Span;

pub fn struct_fields(fileds: &syn::Fields) -> (Vec<&syn::Type>, Vec<syn::Member>) {
    match fileds {
        syn::Fields::Named(fields_named) => fields_named
            .named
            .iter()
            .map(|f| (&f.ty, syn::Member::Named(f.ident.clone().unwrap())))
            .unzip(),
        syn::Fields::Unnamed(fields_unnamed) => fields_unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (
                    &f.ty,
                    syn::Member::Unnamed(syn::Index {
                        index: i as u32,
                        span: Span::call_site(),
                    }),
                )
            })
            .unzip(),
        syn::Fields::Unit => (Vec::new(), Vec::new()),
    }
}

pub fn member_as_idents(members: &[syn::Member]) -> Vec<Cow<'_, syn::Ident>> {
    members
        .iter()
        .map(|member| match member {
            syn::Member::Named(ident) => Cow::Borrowed(ident),
            syn::Member::Unnamed(index) => Cow::Owned(syn::Ident::new(
                &format!("tuple_field_{}", index.index),
                index.span,
            )),
        })
        .collect()
}
