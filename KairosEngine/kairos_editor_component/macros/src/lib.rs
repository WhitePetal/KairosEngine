#![allow(linker_messages)]

//! Macros for `kairos_editor_component`.
//!
//! Two macros, both about "declare it once, next to the thing it describes":
//!
//! * [`macro@panel`] on `impl Panel for YourPanel` declares the panel's id,
//!   registers it (`PanelRegister` + `inventory` submission), generates the
//!   factory and — when the panel has a skin — the `Panel` skin methods.
//! * `#[derive(StyleRow)]` makes a style struct draw its own Preferences page:
//!   the row walk, the erased page, and the whole-value write. The optional
//!   `#[SharedStyle(...)]` helper declaration additionally seals the type as a
//!   `SharedStyle` and submits its `SharedStyleDef`.
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
use syn::{
    Data, DeriveInput, Fields, ItemImpl, Lit, LitStr, Meta, Token, parse_macro_input, parse_quote,
};

// ---------------------------------------------------------------------------
// #[panel(ID = "panel.id", create = path, skin = Style)]
// ---------------------------------------------------------------------------

/// Declares and registers a panel on its `impl Panel` block.
///
/// ```ignore
/// #[crate::panel(SCENE = "scene")]
/// impl Panel for ScenePanel { ... }
///
/// #[crate::panel(ABOUT = "about", create = about_panel, skin = AboutPanelStyle)]
/// impl Panel for AboutPanel { ... }
/// ```
///
/// Generates:
///
/// * `pub const SCENE: PanelId` in the module;
/// * `impl PanelRegister for ScenePanel` (the only way to implement it: the
///   seal lives in a crate-private module) and the `inventory::submit!` entry
///   that `panels::catalog()` collects;
/// * the factory: `Default::default()`, or the function named by
///   `create = path` (`fn(&PanelContext) -> Self`) for a panel that needs the
///   injected identity;
/// * with `skin = StyleType`, the `Panel` methods that let Preferences
///   edit this panel's `Preferences/Styles/*.ron` file: `skin_value` (the live
///   style borrowed as a self-drawing page) and `skin_file` (the RON file it
///   persists to), which the `Panel` trait's provided `skin_page`/`skin_meta`
///   turn into a page snapshot; `replace_skin` (take a whole value the page
///   hands back), `reset_skin` (restore the theme's code defaults) and
///   `save_skin` (persist). The style is expected in a field named `style`; the
///   type must implement `Style + StyleRow`.
///
/// Putting the attribute on the impl rather than the struct is what lets it
/// generate methods inside the trait impl; a second `impl Panel` block is not
/// allowed in Rust.
#[proc_macro_attribute]
pub fn panel(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemImpl);
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

fn expand_panel(args: TokenStream2, mut item: ItemImpl) -> syn::Result<TokenStream2> {
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;
    let mut id_name = None;
    let mut panel_id = None;
    let mut create = None;
    let mut skin = None;

    for meta in metas {
        let Meta::NameValue(name_value) = meta else {
            return Err(syn::Error::new_spanned(
                meta,
                "expected NAME = \"panel.id\"",
            ));
        };
        let Some(name) = name_value.path.get_ident().cloned() else {
            return Err(syn::Error::new_spanned(
                &name_value.path,
                "expected a single identifier",
            ));
        };
        let name_text = name.to_string();
        match name_text.as_str() {
            "create" => match name_value.value {
                syn::Expr::Path(expression) => create = Some(expression.path),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "create takes a function path: fn(&PanelContext) -> Self",
                    ));
                }
            },
            "skin" => match name_value.value {
                syn::Expr::Path(expression) => skin = Some(expression.path),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "skin takes a style type path, e.g. skin = ConsoleStyle",
                    ));
                }
            },
            _ => {
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
                if id_name.replace(name).is_some() {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "the panel id is declared twice",
                    ));
                }
                panel_id = Some(literal);
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

    // The attribute goes on the trait impl: that is where the generated
    // methods live, and the trait impl is what makes the type a panel.
    let is_panel_impl = item
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .is_some_and(|segment| segment.ident == "Panel");
    if !is_panel_impl {
        return Err(syn::Error::new_spanned(
            &item,
            "put #[panel] on the trait impl (impl Panel for YourPanel), not on the struct",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "generic panels cannot be registered automatically",
        ));
    }
    let ident = match &*item.self_ty {
        syn::Type::Path(type_path) if type_path.qself.is_none() => type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.clone())
            .ok_or_else(|| syn::Error::new_spanned(&item.self_ty, "expected a named type"))?,
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "expected a named panel type",
            ));
        }
    };

    if let Some(style) = skin {
        // The two halves the `Panel` trait turns into a page: the live value
        // (borrowed; the owner is the only one who has it) and the file its
        // type persists to. The page heading is the panel's own `title`, so
        // nothing about the page is restated here.
        item.items.push(parse_quote! {
            fn skin_value(
                &self,
            ) -> ::std::option::Option<&dyn ::kairos_editor_component::style_row::StylePage> {
                ::std::option::Option::Some(&self.style)
            }
        });
        item.items.push(parse_quote! {
            fn skin_file(&self) -> ::std::option::Option<&'static str> {
                ::std::option::Option::Some(
                    <#style as ::kairos_editor_widgets::style::Style>::FILE,
                )
            }
        });
        item.items.push(parse_quote! {
            fn replace_skin(
                &mut self,
                skin: ::std::boxed::Box<dyn ::std::any::Any>,
            ) -> bool {
                match skin.downcast::<#style>() {
                    ::std::result::Result::Ok(next) => {
                        self.style = *next;
                        true
                    }
                    ::std::result::Result::Err(_) => false,
                }
            }
        });
        item.items.push(parse_quote! {
            fn reset_skin(&mut self, ctx: &::egui::Context) -> bool {
                self.style = if ctx.theme() == ::egui::Theme::Light {
                    <#style as ::kairos_editor_widgets::style::Style>::light()
                } else {
                    <#style as ::kairos_editor_widgets::style::Style>::dark()
                };
                true
            }
        });
        item.items.push(parse_quote! {
            fn save_skin(
                &self,
                ctx: &::egui::Context,
            ) -> ::std::result::Result<(), ::std::string::String> {
                ::kairos_editor_component::skin::save_skin::<#style>(&self.style, ctx.theme())
            }
        });
    }

    let factory = format_ident!("__create_panel_{}", snake_case(&ident));
    let build = if let Some(create) = create {
        quote! { ::std::boxed::Box::new(#create(context)) }
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
// #[derive(StyleRow)]
// ---------------------------------------------------------------------------

/// See `kairos_editor_component::style_row`.
///
/// Declares a style: the type draws its own Preferences page.
///
/// The derive walks the struct's fields and emits:
///
/// * `style_row::StyleRow` — `row(&mut self, name, sink)`: one `sink.group`
///   per struct and one `StyleRow::row` per field, in declaration order. Nested
///   structs derive it too, so a group such as `icons` nests without the parent
///   knowing it is a group. `#[style(skip)]` leaves a field out of the rows; the
///   field stays part of the value.
/// * `style_row::StylePage` and `kairos_editor_core::style::StyleBlob` — the
///   erased page the Preferences panel holds, and the whole-value write it hands
///   back to the owner. Both need the type to be `Clone + PartialEq`.
///
/// Field values come from the field's type (`f32`/`f64`, `i8`..`i64`/`isize`,
/// `u8`/`u16`/`u32`/`u64`/`usize`, `bool`, `String`, `Color`, `ColorProperty`,
/// `Vec<T>`); the macro itself only knows names and accessors.
///
/// On an enum it generates the enum *token* protocol instead: the active id,
/// how to rebuild a variant, and a `StyleRow` that draws the enum's own menu row
/// followed by the active variant's named fields under a payload scope. The
/// variant table is strum's `VariantNames::VARIANTS`, so the enum must also
/// derive `strum::VariantNames`; the macros do not invent a second name list.
/// Unit variants add nothing; a single-field tuple variant is transparent: its
/// payload's fields are the enum's own rows, with no header row for the unnamed
/// field. A tuple with more than one field is rejected because its values have
/// no names to address.
///
/// # Shared styles
///
/// `#[SharedStyle(CONST = "style.id", title = "Page title")]` additionally
/// declares the type as a shared style:
///
/// * emits `pub const CONST: SharedStyleId`, so the id lives next to the style;
/// * implements the crate-private `SharedStyleSealed` erasure trait (`as_any`,
///   `replace`, `page`), so `SharedStyle` cannot be implemented without this
///   attribute;
/// * submits a `SharedStyleDef` to the inventory table. The submission
///   requires `SharedStyle`, so the attribute without the trait impl does not
///   compile either.
///
/// `SharedStyle` is a helper attribute of this derive, not a separate macro:
/// the derive supplies the self-drawing page, and the helper supplies the id and
/// the Preferences page title. `#[style(skip)]` stays the field-level helper.
///
/// The type then writes only the `preload` hook:
///
/// ```ignore
/// #[derive(StyleRow)]
/// #[SharedStyle(TOOLBAR = "toolbar", title = "Toolbar")]
/// struct ToolBarStyle { /* ... */ }
///
/// impl SharedStyle for ToolBarStyle {
///     fn preload(&self, ctx: &egui::Context) { /* ... */ }
/// }
/// ```
#[proc_macro_derive(StyleRow, attributes(style, SharedStyle))]
pub fn style_row(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_style_row(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_style_row(input: DeriveInput) -> syn::Result<TokenStream2> {
    match &input.data {
        Data::Struct(data) => expand_struct_style_row(&input, data),
        Data::Enum(data) => expand_enum_style_row(&input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input,
            "StyleRow can only be derived for a struct or an enum",
        )),
    }
}

/// A style struct: one row per token, nested structs are groups.
fn expand_struct_style_row(
    input: &DeriveInput,
    data: &syn::DataStruct,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let fields = match &data.fields {
        Fields::Named(fields) => &fields.named,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "every style field must be named",
            ));
        }
    };

    let shared = shared_decl(input)?;
    let shared_items = match shared {
        Some(SharedDecl {
            constant,
            id,
            title,
        }) => quote! {
            impl ::kairos_editor_component::__sealed::SharedStyleSealed for #name {
                fn as_any(&self) -> &dyn ::std::any::Any {
                    self
                }

                fn replace(
                    &mut self,
                    skin: ::std::boxed::Box<dyn ::std::any::Any>,
                ) -> bool {
                    match skin.downcast::<#name>() {
                        ::std::result::Result::Ok(next) => {
                            *self = *next;
                            true
                        }
                        ::std::result::Result::Err(_) => false,
                    }
                }

                fn page(
                    &self,
                ) -> ::std::boxed::Box<
                    dyn ::kairos_editor_component::style_row::StylePage,
                > {
                    ::std::boxed::Box::new(self.clone())
                }
            }

            pub const #constant: ::kairos_editor_component::__macro::SharedStyleId =
                ::kairos_editor_component::__macro::SharedStyleId(#id);

            ::kairos_editor_component::__macro::inventory::submit! {
                ::kairos_editor_component::__macro::SharedStyleDef::new::<#name>(
                    #constant,
                    #title,
                )
            }
        },
        None => quote! {},
    };

    let mut rows = Vec::new();
    for field in fields {
        if is_skipped(field) {
            continue;
        }
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let key = ident.to_string();
        rows.push(quote! {
            ::kairos_editor_component::style_row::StyleRow::row(&mut self.#ident, #key, sink);
        });
    }

    Ok(quote! {
        impl ::kairos_editor_component::style_row::StyleRow for #name {
            fn row(
                &mut self,
                name: &str,
                sink: &mut impl ::kairos_editor_component::style_row::RowSink,
            ) {
                sink.group(name, |sink| {
                    #(#rows)*
                });
            }
        }

        // A style is also a page value: the editor draws its own typed copy and
        // hands the whole thing back through `Skin` (see `StyleBlob`). The
        // stamp needs `Clone` and `PartialEq`, which every style derives.
        impl ::kairos_editor_core::style::StyleBlob for #name {
            fn clone_blob(&self) -> ::std::boxed::Box<dyn ::kairos_editor_core::style::StyleBlob> {
                ::std::boxed::Box::new(self.clone())
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn into_any(self: ::std::boxed::Box<Self>) -> ::std::boxed::Box<dyn ::std::any::Any> {
                self
            }

            fn equals(&self, other: &dyn ::kairos_editor_core::style::StyleBlob) -> bool {
                other.as_any().downcast_ref::<Self>() == ::std::option::Option::Some(self)
            }

            fn debug_blob(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(::std::any::type_name::<Self>())
            }
        }

        impl ::kairos_editor_component::style_row::StylePage for #name {
            fn draw_page(&mut self, rows: &mut ::kairos_editor_widgets::form::FormRows<'_>) {
                ::kairos_editor_component::style_row::StyleRow::row(self, "", rows);
            }

            fn clone_box(
                &self,
            ) -> ::std::boxed::Box<dyn ::kairos_editor_component::style_row::StylePage> {
                ::std::boxed::Box::new(self.clone())
            }
        }

        #shared_items
    })
}

/// A style enum: a menu row plus the active variant's payload rows.
///
/// The enum's own row carries the variant names as field metadata; the active
/// variant's named fields are collected as ordinary tokens below its path, so a
/// payload of any type (f32, Color, a nested struct, a list) is drawn by the
/// widget its value variant names. Unit variants and struct-like variants share
/// this one expansion: a unit variant simply has no payload rows.
fn expand_enum_style_row(input: &DeriveInput, data: &syn::DataEnum) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if shared_decl(input)?.is_some() {
        return Err(syn::Error::new_spanned(
            input,
            "a shared style must be a struct: an enum token is a field of a style, not a style itself",
        ));
    }

    let mut variant_ids = Vec::new();
    let mut with_variant_arms = Vec::new();
    let mut row_arms = Vec::new();

    for variant in &data.variants {
        if let Some(attr) = variant
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("style"))
        {
            return Err(syn::Error::new_spanned(
                attr,
                "an enum variant no longer takes #[style(...)]: the menu uses the names from strum::VariantNames",
            ));
        }
        let ident = &variant.ident;
        let id = ident.to_string();

        match &variant.fields {
            Fields::Unit => {
                variant_ids.push(quote! { Self::#ident => #id, });
                with_variant_arms.push(quote! {
                    #id => ::std::option::Option::Some(Self::#ident),
                });
                row_arms.push(quote! { Self::#ident => {}, });
            }
            Fields::Named(fields) => {
                // Only the token fields are collected and written; a
                // #[style(skip)] payload field is still bound in the pattern
                // and still built by with_variant, but it is not a row.
                let token_fields: Vec<&syn::Field> = fields
                    .named
                    .iter()
                    .filter(|field| !is_skipped(field))
                    .collect();
                let all_fields: Vec<&syn::Ident> = fields
                    .named
                    .iter()
                    .filter_map(|field| field.ident.as_ref())
                    .collect();
                let bindings: Vec<TokenStream2> = all_fields
                    .iter()
                    .map(|field| {
                        if token_fields
                            .iter()
                            .any(|token| token.ident.as_ref() == Some(*field))
                        {
                            quote! { #field }
                        } else {
                            quote! { #field: _ }
                        }
                    })
                    .collect();

                variant_ids.push(quote! { Self::#ident { .. } => #id, });

                let defaults = all_fields.iter().map(|field| {
                    quote! { #field: ::core::default::Default::default() }
                });
                with_variant_arms.push(quote! {
                    #id => ::std::option::Option::Some(Self::#ident { #(#defaults),* }),
                });

                // The payload rows hang under the menu button, which is a
                // layout scope, not a path: the bindings already name the
                // tokens, and the sink knows the enum's own path.
                let rows = token_fields.iter().map(|field| {
                    let field = field.ident.as_ref().expect("a named payload field");
                    let key = field.to_string();
                    quote! {
                        ::kairos_editor_component::style_row::StyleRow::row(#field, #key, sink);
                    }
                });
                if token_fields.is_empty() {
                    row_arms.push(quote! {
                        Self::#ident { #(#bindings),* } => {},
                    });
                } else {
                    row_arms.push(quote! {
                        Self::#ident { #(#bindings),* } => {
                            sink.payload(name, |sink| {
                                #(#rows)*
                            });
                        }
                    });
                }
            }
            Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        fields,
                        "a tuple payload must have exactly one field: a single struct payload is transparent, while several values need names to be addressed",
                    ));
                }
                let field = fields.unnamed.first().expect("one tuple field");
                variant_ids.push(quote! { Self::#ident(_) => #id, });
                with_variant_arms.push(quote! {
                    #id => ::std::option::Option::Some(
                        Self::#ident(::core::default::Default::default()),
                    ),
                });
                if is_skipped(field) {
                    row_arms.push(quote! { Self::#ident(_) => {}, });
                } else {
                    // A tuple payload is transparent: it enters the payload
                    // scope with an empty name, so its fields are the enum's
                    // own rows (no header, no extra path segment).
                    row_arms.push(quote! {
                        Self::#ident(payload) => {
                            sink.payload(name, |sink| {
                                ::kairos_editor_component::style_row::StyleRow::row(
                                    payload,
                                    "",
                                    sink,
                                );
                            });
                        }
                    });
                }
            }
        }
    }

    Ok(quote! {
        impl ::kairos_editor_component::skin::StyleEnum for #name {
            fn variant_id(&self) -> &'static str {
                match self {
                    #(#variant_ids)*
                }
            }

            fn with_variant(id: &str) -> ::std::option::Option<Self> {
                match id {
                    #(#with_variant_arms)*
                    _ => ::std::option::Option::None,
                }
            }
        }

        impl ::kairos_editor_component::style_row::StyleRow for #name {
            fn row(
                &mut self,
                name: &str,
                sink: &mut impl ::kairos_editor_component::style_row::RowSink,
            ) {
                sink.variant(name, self);
                match self {
                    #(#row_arms)*
                }
            }
        }

    })
}

struct SharedDecl {
    constant: syn::Ident,
    id: LitStr,
    title: LitStr,
}

/// Reads the optional `#[SharedStyle(CONST = "style.id", title = "Page title")]`.
///
/// It is a helper attribute of the `StyleRow` derive: the derive is what makes
/// the type a self-drawing page (and therefore something the registry can hand
/// the Preferences panel), so the declaration cannot stand alone. Absence is
/// the normal case: most style structs are nested groups of a style (such as
/// the toolbar's menu), not shell-owned styles.
fn shared_decl(input: &DeriveInput) -> syn::Result<Option<SharedDecl>> {
    let mut found = None;
    for attr in &input.attrs {
        if !attr.path().is_ident("SharedStyle") {
            continue;
        }
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "`SharedStyle` is declared twice",
            ));
        }

        let mut constant = None;
        let mut title = None;
        attr.parse_nested_meta(|item| {
            if item.path.is_ident("title") {
                if title.is_some() {
                    return Err(item.error("`title` is declared twice"));
                }
                title = Some(item.value()?.parse::<LitStr>()?);
                return Ok(());
            }
            let Some(name) = item.path.get_ident().cloned() else {
                return Err(item.error("expected CONST = \"style.id\""));
            };
            if constant.is_some() {
                return Err(item.error("the shared style id is declared twice"));
            }
            constant = Some((name, item.value()?.parse::<LitStr>()?));
            Ok(())
        })?;

        let Some((constant, id)) = constant else {
            return Err(syn::Error::new_spanned(
                attr,
                "expected SharedStyle(CONST = \"style.id\", title = \"Page title\")",
            ));
        };
        let Some(title) = title else {
            return Err(syn::Error::new_spanned(
                attr,
                "expected title = \"Preferences page title\"",
            ));
        };
        let constant_text = constant.to_string();
        if constant_text != constant_text.to_ascii_uppercase() {
            return Err(syn::Error::new_spanned(
                &constant,
                "the shared style constant must be UPPER_SNAKE_CASE",
            ));
        }
        found = Some(SharedDecl {
            constant,
            id,
            title,
        });
    }
    Ok(found)
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

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn expand(input: DeriveInput) -> String {
        expand_style_row(input)
            .expect("the declaration expands")
            .to_string()
    }

    #[test]
    fn a_style_struct_walks_its_fields_as_rows() {
        let tokens = expand(parse_quote! {
            struct ConsoleStyle {
                height: f32,
                icons: Icons,
                #[style(skip)]
                cache: u32,
            }
        });

        assert!(tokens.contains("StyleRow :: row"), "{tokens}");
        assert!(tokens.contains("\"height\"") && tokens.contains("\"icons\""));
        assert!(
            !tokens.contains("\"cache\""),
            "a skipped field draws no row"
        );
        assert!(
            tokens.contains("sink . group"),
            "a struct is a group of its fields"
        );
    }

    /// A style is also a page value: the editor holds a typed copy and hands
    /// the whole value back, so the derive emits the erased page and the blob.
    #[test]
    fn a_style_struct_is_also_an_erased_page() {
        let tokens = expand(parse_quote! {
            struct ConsoleStyle {
                height: f32,
            }
        });

        assert!(tokens.contains("StyleBlob"));
        assert!(tokens.contains("StylePage"));
        assert!(
            tokens.contains("draw_page"),
            "the page draws the style into the row sink"
        );
    }

    #[test]
    fn a_shared_declaration_emits_the_id_the_seal_and_the_submission() {
        let tokens = expand(parse_quote! {
            #[SharedStyle(TOOLBAR = "toolbar", title = "Toolbar")]
            struct ToolBarStyle {
                height: f32,
            }
        });

        assert!(tokens.contains("SharedStyleSealed"));
        assert!(tokens.contains("TOOLBAR"));
        assert!(tokens.contains("SharedStyleDef"));
        assert!(tokens.contains("toolbar"));
        assert!(tokens.contains("Toolbar"));
    }

    #[test]
    fn a_plain_style_struct_submits_nothing() {
        let tokens = expand(parse_quote! {
            struct ToolBarMenuStyle {
                item_height: f32,
            }
        });

        assert!(!tokens.contains("SharedStyleSealed"));
        assert!(!tokens.contains("SharedStyleDef"));
        assert!(!tokens.contains("inventory"));
    }

    #[test]
    fn an_enum_draws_its_menu_row_then_the_active_payload() {
        let tokens = expand(parse_quote! {
            enum ShadowMode {
                Off,
                Soft { radius: f32, samples: u32 },
            }
        });

        assert!(
            tokens.contains("sink . variant"),
            "the enum is one menu row: {tokens}"
        );
        assert!(
            tokens.contains("sink . payload"),
            "the active variant's fields hang under the menu row"
        );
        assert!(tokens.contains("\"Off\"") && tokens.contains("\"Soft\""));
        assert!(
            tokens.contains("Default :: default"),
            "switching to a payload variant rebuilds it from Default"
        );
        assert!(
            tokens.contains("\"radius\"") && tokens.contains("\"samples\""),
            "payload fields become rows"
        );
    }

    /// A payload field can be a non-row: it stays part of the value (and is
    /// rebuilt from Default) but is not drawn.
    #[test]
    fn a_skipped_payload_field_is_still_part_of_the_value() {
        let tokens = expand(parse_quote! {
            enum Mode {
                Soft { radius: f32, #[style(skip)] cache: u32 },
            }
        });

        assert!(tokens.contains("\"radius\""));
        assert!(
            !tokens.contains("\"cache\""),
            "a skipped field is not a row"
        );
        assert!(
            tokens.matches("cache").count() >= 2,
            "the pattern and the constructor still mention the skipped field"
        );
        assert!(
            tokens.contains("Default :: default"),
            "the constructor still builds the skipped field"
        );
    }

    /// A single-field tuple payload is transparent: it enters the payload scope
    /// with an empty name, so its fields are the enum's own rows.
    #[test]
    fn a_single_tuple_payload_is_transparent() {
        let tokens = expand(parse_quote! {
            enum Mode {
                Soft(Payload),
            }
        });

        assert!(tokens.contains("sink . payload"));
        assert!(
            tokens.contains("StyleRow :: row (payload , \"\" , sink"),
            "the payload is drawn under an empty name: {tokens}"
        );
        assert!(
            !tokens.contains("\"0\""),
            "the macro does not invent an index for the unnamed field"
        );
    }

    /// Several tuple fields have no names, so there is no stable row for each
    /// one.
    #[test]
    fn a_multi_field_tuple_payload_is_rejected() {
        let error = expand_style_row(parse_quote! {
            enum Mode {
                Soft(f32, u32),
            }
        })
        .expect_err("two tuple fields have no names");

        assert!(error.to_string().contains("exactly one field"), "{error}");
    }

    #[test]
    fn a_shared_declaration_requires_a_title() {
        let error = expand_style_row(parse_quote! {
            #[SharedStyle(TOOLBAR = "toolbar")]
            struct ToolBarStyle {
                height: f32,
            }
        })
        .expect_err("a Preferences page heading is required");

        assert!(error.to_string().contains("title"));
    }
}
