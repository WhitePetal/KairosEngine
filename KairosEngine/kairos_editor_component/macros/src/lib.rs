//! Derive macros for `kairos_editor_component`.
//!
//! `#[derive(StyleFields)]` makes a style struct describe itself field by field,
//! which is what the Preferences panel edits. It walks the struct's fields and
//! emits two impls:
//!
//! * `skin::StyleNode` — `collect` (append this value or its children to a
//!   `Vec<StyleField>`) and `write` (write one leaf by dotted path). Nested
//!   structs derive it too, so a group such as `icons` nests without the
//!   parent knowing it is a group.
//! * `skin::StyleFields` — the entry point the `Panel` trait forwards to.
//!
//! Field *kinds* come from the field's type (`f32`, `u8`/`u16`/`u32`/`usize`,
//! `bool`, `String`, `Color32`); the macro itself only knows names and
//! accessors. That is why adding a token to a style struct adds a row to
//! Preferences with no second list to keep in sync — the defect the legacy
//! `get_style_fileds()`/`update_style()` pair had.
//!
//! `#[style(skip)]` leaves a field out (a style struct that carries a
//! non-token field such as a cached texture handle).

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// See the crate docs.
#[proc_macro_derive(StyleFields, attributes(style))]
pub fn style_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let data = match &input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "StyleFields can only be derived for a struct",
            ));
        }
    };
    let fields = match &data.fields {
        Fields::Named(fields) => &fields.named,
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "every style field must be named",
            ));
        }
    };

    let mut collects = Vec::new();
    let mut writes = Vec::new();
    for field in fields {
        if is_skipped(field) {
            continue;
        }
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let key = ident.to_string();
        collects.push(quote! {
            {
                let child = if path.is_empty() {
                    ::std::string::String::from(#key)
                } else {
                    ::std::format!("{path}.{}", #key)
                };
                ::kairos_editor_component::skin::StyleNode::collect(&self.#ident, &child, out);
            }
        });
        writes.push(quote! {
            #key => ::kairos_editor_component::skin::StyleNode::write(&mut self.#ident, tail, value),
        });
    }

    Ok(quote! {
        impl ::kairos_editor_component::skin::StyleNode for #name {
            fn collect(
                &self,
                path: &str,
                out: &mut ::std::vec::Vec<::kairos_editor_component::skin::StyleField>,
            ) {
                #(#collects)*
            }

            fn write(
                &mut self,
                path: &str,
                value: &::kairos_editor_core::StyleValue,
            ) -> bool {
                let (head, tail) = match path.split_once('.') {
                    ::std::option::Option::Some((head, tail)) => (head, tail),
                    ::std::option::Option::None => (path, ""),
                };
                match head {
                    #(#writes)*
                    _ => false,
                }
            }
        }

        impl ::kairos_editor_component::skin::StyleFields for #name {
            fn style_fields(&self) -> ::std::vec::Vec<::kairos_editor_component::skin::StyleField> {
                let mut out = ::std::vec::Vec::new();
                ::kairos_editor_component::skin::StyleNode::collect(self, "", &mut out);
                out
            }

            fn set_style_field(
                &mut self,
                path: &str,
                value: &::kairos_editor_core::StyleValue,
            ) -> bool {
                ::kairos_editor_component::skin::StyleNode::write(self, path, value)
            }
        }
    })
}

fn is_skipped(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| {
        if !attr.path().is_ident("style") {
            return false;
        }
        let mut skipped = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                skipped = true;
            }
            Ok(())
        });
        skipped
    })
}
