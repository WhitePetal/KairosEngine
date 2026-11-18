#![allow(linker_messages)]

//! Macros for `kairos_editor_component`.
//!
//! Two macros, both about "declare it once, next to the thing it describes":
//!
//! * [`macro@panel`] declares a panel and registers it in one item: the struct,
//!   the id constant, `PanelRegister`, the factory and the `inventory`
//!   submission. Because `Panel: PanelRegister`, a plain
//!   `impl Panel for X` without the attribute fails to compile, so a panel
//!   cannot exist without being registered.
//! * `#[derive(StyleFields)]` makes a style struct describe itself field by
//!   field, which is what the Preferences panel edits. It walks the struct's
//!   fields and emits `skin::StyleNode` (`collect`/`write` by dotted path) and
//!   `skin::StyleFields` (the entry point `Panel` forwards to).
//!
//! Both exist for the same reason: the legacy editor kept a second, hand-written
//! list next to the real definition (`drawers` order for panels,
//! `style_fields[0..n]` for style values) and the two drifted. A macro cannot
//! forget a field.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Data, DeriveInput, Fields, ItemStruct, Lit, Meta, Token, parse_macro_input};

// ---------------------------------------------------------------------------
// #[panel(NAME = "panel.id", create = path)]
// ---------------------------------------------------------------------------

/// Declares a panel and registers it: the struct, the id constant, the
/// `PanelRegister` impl, the factory and the link-section submission.
///
/// `#[panel(SCENE = "scene")]` on `ScenePanel` generates `pub const SCENE: PanelId`,
/// `impl PanelRegister for ScenePanel` and an `inventory::submit!` entry
/// pointing at a generated factory. A panel with fields is built through
/// `Default`, or through `create = path` (`fn(&PanelContext) -> Self`),
/// which is how About receives the injected identity.
///
/// The seal (`__sealed::Sealed`) is what makes this the only way to
/// implement `PanelRegister`; panels are defined in this crate.
#[proc_macro_attribute]
pub fn panel(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemStruct);
    match expand_panel(args.into(), item) {
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

fn expand_panel(args: TokenStream2, item: ItemStruct) -> syn::Result<TokenStream2> {
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
    let id_text = id_name.to_string();
    if id_text != id_text.to_ascii_uppercase() {
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

// ---------------------------------------------------------------------------
// #[derive(StyleFields)]
// ---------------------------------------------------------------------------

/// See `kairos_editor_component::skin`.
///
/// The derive walks the struct's fields and emits two impls:
///
/// * `skin::StyleNode` — `collect` (append this value or its children to a
///   `Vec<StyleField>`) and `write` (write one leaf by dotted path). Nested
///   structs derive it too, so a group such as `icons` nests without the
///   parent knowing it is a group.
/// * `skin::StyleFields` — the entry point the `Panel` trait forwards to.
///
/// Field kinds come from the field's type (`f32`, `u8`/`u16`/`u32`/`usize`,
/// `bool`, `String`, `Color32`); the macro itself only knows names and
/// accessors. `#[style(skip)]` leaves a field out.
#[proc_macro_derive(StyleFields, attributes(style))]
pub fn style_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_style_fields(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_style_fields(input: DeriveInput) -> syn::Result<TokenStream2> {
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
