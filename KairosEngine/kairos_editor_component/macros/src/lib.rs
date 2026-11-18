#![allow(linker_messages)]

//! `#[panel]`: declare a panel and register it in one item.
//!
//! Applied to a panel struct, it generates the struct, the id constant (the
//! attribute's key), `impl PanelRegister` for the struct, the factory that
//! builds one instance, and an `inventory::submit!` entry. Because
//! `Panel: PanelRegister`, a plain `impl Panel for X` without the attribute
//! fails to compile.
//!
//! Panels with fields are built through `Default`, or through the function
//! named by `create = path` (`fn(&PanelContext) -> Self`), which is how About
//! receives the injected identity.
//!
//! The attribute is also the only way to implement `PanelRegister`: its seal
//! lives in a crate-private module, so a panel cannot be registered by hand and
//! panels are defined inside `kairos_editor_component`.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Fields, ItemStruct, Lit, Meta, Token};

#[proc_macro_attribute]
pub fn panel(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = syn::parse_macro_input!(item as ItemStruct);
    match expand(args.into(), item) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// `AboutPanel` -> `about_panel`, so the generated factory keeps the lint quiet.
fn snake_case(ident: &syn::Ident) -> String {
    let mut out = String::new();
    for (index, ch) in ident.to_string().chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn expand(args: TokenStream2, item: ItemStruct) -> syn::Result<TokenStream2> {
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;
    let mut id_name = None;
    let mut panel_id = None;
    let mut create = None;

    for meta in metas {
        match meta {
            Meta::NameValue(name_value) if name_value.path.is_ident("create") => {
                match name_value.value {
                    syn::Expr::Path(expression) => create = Some(expression.path),
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "create takes a function path: fn(&PanelContext) -> Self",
                        ));
                    }
                }
            }
            Meta::NameValue(name_value) => {
                let ident = name_value.path.get_ident().cloned().ok_or_else(|| {
                    syn::Error::new_spanned(&name_value.path, "expected NAME = \"panel.id\"")
                })?;
                let literal = match name_value.value {
                    syn::Expr::Lit(expression) => expression.lit,
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "the panel id must be a string literal",
                        ));
                    }
                };
                let literal = match literal {
                    Lit::Str(literal) => literal,
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "the panel id must be a string literal",
                        ));
                    }
                };
                if id_name.replace(ident).is_some() {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "the panel id is declared twice",
                    ));
                }
                panel_id = Some(literal);
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "expected NAME = \"panel.id\", optionally with create = path",
                ));
            }
        }
    }

    let (Some(id_name), Some(panel_id)) = (id_name, panel_id) else {
        return Err(syn::Error::new(
            Span::call_site(),
            "expected #[panel(NAME = \"panel.id\")]",
        ));
    };
    if id_name.to_string() != id_name.to_string().to_ascii_uppercase() {
        return Err(syn::Error::new_spanned(
            &id_name,
            "the panel constant must be UPPER_SNAKE_CASE",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "generic panels cannot be registered automatically",
        ));
    }

    let ident = &item.ident;
    let factory = format_ident!("__create_panel_{}", snake_case(ident));
    let build = if let Some(create) = create {
        quote! { ::std::boxed::Box::new(#create(context)) }
    } else if matches!(&item.fields, Fields::Unit) {
        quote! { ::std::boxed::Box::new(#ident) }
    } else {
        quote! {
            ::std::boxed::Box::new(<#ident as ::core::default::Default>::default())
        }
    };

    Ok(quote! {
        #item

        pub const #id_name: ::kairos_editor_component::__macro::PanelId =
            ::kairos_editor_component::__macro::PanelId(#panel_id);

        impl ::kairos_editor_component::__sealed::Sealed for #ident {}

        impl ::kairos_editor_component::__macro::PanelRegister for #ident {
            fn panel_id(&self) -> ::kairos_editor_component::__macro::PanelId {
                #id_name
            }
        }

        #[doc(hidden)]
        fn #factory(
            context: &::kairos_editor_component::__macro::PanelContext,
        ) -> ::std::boxed::Box<dyn ::kairos_editor_component::__macro::Panel> {
            #build
        }

        ::kairos_editor_component::__macro::inventory::submit! {
            ::kairos_editor_component::__macro::PanelDef::new(#id_name, #factory)
        }
    })
}
