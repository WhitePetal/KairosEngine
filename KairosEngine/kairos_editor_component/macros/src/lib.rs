#![allow(linker_messages)]

//! Macros for `kairos_editor_component`.
//!
//! Two macros, both about "declare it once, next to the thing it describes":
//!
//! * [`macro@panel`] on `impl Panel for YourPanel` declares the panel's id,
//!   registers it (`PanelRegister` + `inventory` submission), generates the
//!   factory and — when the panel has a skin — the three `Panel::skin` methods.
//! * `#[derive(StyleFields)]` makes a style struct describe itself field by
//!   field, which is what the Preferences panel edits. The optional
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
/// * with `skin = StyleType`, the three `Panel` methods that let Preferences
///   edit this panel's `Preferences/Styles/*.ron` file: `skin` (the field
///   table, titled by the panel's own `title`), `set_skin_field` (live
///   write) and `save_skin` (persist). The style is expected in a field named
///   `style`; the type must implement `Style + StyleFields`.
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
        // The page title is the panel's own title: a skin page is named after
        // the panel, so passing a second title here would be a duplicate
        // source of truth.
        item.items.push(parse_quote! {
            fn skin(
                &self,
                frame: &::kairos_editor_component::frame::Frame,
            ) -> ::std::option::Option<::kairos_editor_component::skin::PanelSkin> {
                ::std::option::Option::Some(::kairos_editor_component::skin::PanelSkin {
                    panel: #id_name,
                    title: ::kairos_editor_component::panel::Panel::title(self, frame)
                        .text()
                        .to_owned(),
                    file: <#style as ::kairos_editor_widgets::style::Style>::FILE,
                    fields: ::kairos_editor_component::skin::StyleFields::style_fields(
                        &self.style,
                    ),
                })
            }
        });
        item.items.push(parse_quote! {
            fn set_skin_field(
                &mut self,
                path: &str,
                value: &::kairos_editor_core::StyleValue,
            ) -> bool {
                ::kairos_editor_component::skin::StyleFields::set_style_field(
                    &mut self.style,
                    path,
                    value,
                )
            }
        });
        item.items.push(parse_quote! {
            fn edit_skin_list(
                &mut self,
                path: &str,
                edit: ::kairos_editor_core::ListEdit,
            ) -> bool {
                ::kairos_editor_component::skin::StyleFields::edit_style_list(
                    &mut self.style,
                    path,
                    edit,
                )
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
/// * `skin::StyleFields` — the entry point `Panel::skin` forwards to.
///
/// Field values come from the field's type (`f32`/`f64`, `i8`..`i64`/`isize`,
/// `u8`/`u16`/`u32`/`u64`/`usize`, `bool`, `String`, `Color`, `ColorProperty`);
/// the macro itself only knows names and accessors. `#[style(skip)]` leaves a
/// field out.
///
/// On an enum it generates the enum *token* protocol instead: the active id,
/// how to rebuild a variant, and a `StyleNode` that emits the enum's own row
/// followed by the active variant's named fields as dotted children. The
/// variant table is strum's `VariantNames::VARIANTS`, so the enum must also
/// derive `strum::VariantNames`; the macros do not invent a second name list.
/// Unit variants add nothing; a single-field tuple variant is transparent: its
/// payload's fields are the enum's own payload, with no header row for the
/// unnamed field. The tuple payload is meant to be a struct/group; a leaf
/// payload would collect at the enum's own path. A tuple with more than one
/// field is rejected because its values have no names to address.
///
/// # Shared styles
///
/// `#[SharedStyle(CONST = "style.id", title = "Page title")]` additionally
/// declares the type as a shared style:
///
/// * emits `pub const CONST: SharedStyleId`, so the id lives next to the style;
/// * implements the crate-private `SharedStyleSealed` erasure trait, so
///   `SharedStyle` cannot be implemented without this attribute;
/// * submits a `SharedStyleDef` to the inventory table. The submission
///   requires `SharedStyle`, so the attribute without the trait impl does not
///   compile either.
///
/// `SharedStyle` is a helper attribute of this derive, not a separate macro:
/// the derive supplies the field table, and the helper supplies the id and the
/// Preferences page title. `#[style(skip)]` stays the field-level helper.
///
/// The type then writes only the `preload` hook:
///
/// ```ignore
/// #[derive(StyleFields)]
/// #[SharedStyle(TOOLBAR = "toolbar", title = "Toolbar")]
/// struct ToolBarStyle { /* ... */ }
///
/// impl SharedStyle for ToolBarStyle {
///     fn preload(&self, ctx: &egui::Context) { /* ... */ }
/// }
/// ```
#[proc_macro_derive(StyleFields, attributes(style, SharedStyle))]
pub fn style_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_style_fields(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_style_fields(input: DeriveInput) -> syn::Result<TokenStream2> {
    match &input.data {
        Data::Struct(data) => expand_struct_style_fields(&input, data),
        Data::Enum(data) => expand_enum_style_fields(&input, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input,
            "StyleFields can only be derived for a struct or an enum",
        )),
    }
}

/// A style struct: one row per token, nested structs are dotted groups.
fn expand_struct_style_fields(
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

    let mut collects = Vec::new();
    let mut writes = Vec::new();
    let mut edits = Vec::new();
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
        edits.push(quote! {
            #key => ::kairos_editor_component::skin::StyleNode::edit_list(&mut self.#ident, tail, edit),
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

            fn edit_list(
                &mut self,
                path: &str,
                edit: ::kairos_editor_core::ListEdit,
            ) -> bool {
                let (head, tail) = match path.split_once('.') {
                    ::std::option::Option::Some((head, tail)) => (head, tail),
                    ::std::option::Option::None => (path, ""),
                };
                match head {
                    #(#edits)*
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

            fn edit_style_list(
                &mut self,
                path: &str,
                edit: ::kairos_editor_core::ListEdit,
            ) -> bool {
                ::kairos_editor_component::skin::StyleNode::edit_list(self, path, edit)
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
fn expand_enum_style_fields(
    input: &DeriveInput,
    data: &syn::DataEnum,
) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    if shared_decl(input)?.is_some() {
        return Err(syn::Error::new_spanned(
            input,
            "a shared style must be a struct: an enum token is a field of a style, not a style itself",
        ));
    }

    let mut variant_ids = Vec::new();
    let mut with_variant_arms = Vec::new();
    let mut collect_arms = Vec::new();
    let mut write_arms = Vec::new();
    let mut edit_arms = Vec::new();

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
                collect_arms.push(quote! { Self::#ident => {}, });
                write_arms.push(quote! { Self::#ident => false, });
                edit_arms.push(quote! { Self::#ident => false, });
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

                let collects = token_fields.iter().map(|field| {
                    let field = field.ident.as_ref().expect("a named payload field");
                    let key = field.to_string();
                    quote! {
                        {
                            let child = if path.is_empty() {
                                ::std::string::String::from(#key)
                            } else {
                                ::std::format!("{path}.{}", #key)
                            };
                            ::kairos_editor_component::skin::StyleNode::collect(
                                #field,
                                &child,
                                out,
                            );
                        }
                    }
                });
                collect_arms.push(quote! {
                    Self::#ident { #(#bindings),* } => {
                        #(#collects)*
                    }
                });

                let writes = token_fields.iter().map(|field| {
                    let field = field.ident.as_ref().expect("a named payload field");
                    let key = field.to_string();
                    quote! {
                        #key => ::kairos_editor_component::skin::StyleNode::write(
                            #field,
                            tail,
                            value,
                        ),
                    }
                });
                write_arms.push(quote! {
                    Self::#ident { #(#bindings),* } => {
                        match head {
                            #(#writes)*
                            _ => false,
                        }
                    }
                });

                let edits = token_fields.iter().map(|field| {
                    let field = field.ident.as_ref().expect("a named payload field");
                    let key = field.to_string();
                    quote! {
                        #key => ::kairos_editor_component::skin::StyleNode::edit_list(
                            #field,
                            tail,
                            edit,
                        ),
                    }
                });
                edit_arms.push(quote! {
                    Self::#ident { #(#bindings),* } => {
                        match head {
                            #(#edits)*
                            _ => false,
                        }
                    }
                });
            }
            Fields::Unnamed(fields) => {
                if fields.unnamed.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        fields,
                        "a tuple payload must have exactly one field: a single struct payload is transparent, while several values need names to be addressed",
                    ));
                }
                let field = fields.unnamed.first().expect("one tuple field");
                let payload = syn::Ident::new("payload", Span::call_site());
                variant_ids.push(quote! { Self::#ident(_) => #id, });
                with_variant_arms.push(quote! {
                    #id => ::std::option::Option::Some(
                        Self::#ident(::core::default::Default::default()),
                    ),
                });
                if is_skipped(field) {
                    collect_arms.push(quote! { Self::#ident(_) => {}, });
                    write_arms.push(quote! { Self::#ident(_) => false, });
                    edit_arms.push(quote! { Self::#ident(_) => false, });
                } else {
                    // A tuple payload has no field name to put on a header
                    // row: its tokens are the enum's own payload, at the same
                    // path. A struct payload therefore contributes its fields
                    // directly under the enum row.
                    collect_arms.push(quote! {
                        Self::#ident(#payload) => {
                            ::kairos_editor_component::skin::StyleNode::collect(
                                #payload,
                                path,
                                out,
                            );
                        }
                    });
                    write_arms.push(quote! {
                        Self::#ident(#payload) => {
                            let _ = (head, tail);
                            ::kairos_editor_component::skin::StyleNode::write(
                                #payload,
                                path,
                                value,
                            )
                        }
                    });
                    edit_arms.push(quote! {
                        Self::#ident(#payload) => {
                            let _ = (head, tail);
                            ::kairos_editor_component::skin::StyleNode::edit_list(
                                #payload,
                                path,
                                edit,
                            )
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

        impl ::kairos_editor_component::skin::StyleNode for #name {
            fn collect(
                &self,
                path: &str,
                out: &mut ::std::vec::Vec<::kairos_editor_component::skin::StyleField>,
            ) {
                out.push(
                    ::kairos_editor_component::skin::StyleField::new(
                        path,
                        ::kairos_editor_component::skin::humanize(path),
                        ::kairos_editor_core::StyleValue::Variant(
                            ::std::string::String::from(
                                <Self as ::kairos_editor_component::skin::StyleEnum>::variant_id(
                                    self,
                                ),
                            ),
                        ),
                    )
                    .with_variants(
                        <Self as ::kairos_editor_component::skin::StyleEnum>::variants(),
                    ),
                );
                match self {
                    #(#collect_arms)*
                }
            }

            fn write(
                &mut self,
                path: &str,
                value: &::kairos_editor_core::StyleValue,
            ) -> bool {
                if path.is_empty() {
                    let ::kairos_editor_core::StyleValue::Variant(id) = value else {
                        return false;
                    };
                    if <Self as ::kairos_editor_component::skin::StyleEnum>::variant_id(self)
                        == id.as_str()
                    {
                        return true;
                    }
                    return match <Self as ::kairos_editor_component::skin::StyleEnum>::with_variant(
                        id.as_str(),
                    ) {
                        ::std::option::Option::Some(next) => {
                            *self = next;
                            true
                        }
                        ::std::option::Option::None => false,
                    };
                }
                let (head, tail) = match path.split_once('.') {
                    ::std::option::Option::Some((head, tail)) => (head, tail),
                    ::std::option::Option::None => (path, ""),
                };
                match self {
                    #(#write_arms)*
                }
            }

            fn edit_list(
                &mut self,
                path: &str,
                edit: ::kairos_editor_core::ListEdit,
            ) -> bool {
                let (head, tail) = match path.split_once('.') {
                    ::std::option::Option::Some((head, tail)) => (head, tail),
                    ::std::option::Option::None => (path, ""),
                };
                match self {
                    #(#edit_arms)*
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

            fn edit_style_list(
                &mut self,
                path: &str,
                edit: ::kairos_editor_core::ListEdit,
            ) -> bool {
                ::kairos_editor_component::skin::StyleNode::edit_list(self, path, edit)
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
/// It is a helper attribute of the `StyleFields` derive: the derive is what
/// generates the field table the type needs to be a shared style, so the
/// declaration cannot stand alone. Absence is the normal case: most
/// `StyleFields` structs are field tables (a nested group such as the toolbar
/// menu), not shell-owned styles.
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
        expand_style_fields(input)
            .expect("the declaration expands")
            .to_string()
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
    fn a_plain_style_fields_struct_submits_nothing() {
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
    fn an_enum_lists_its_variants_and_routes_the_active_payload() {
        let tokens = expand(parse_quote! {
            enum ShadowMode {
                Off,
                Soft { radius: f32, samples: u32 },
            }
        });

        assert!(
            !tokens.contains("StyleVariant"),
            "the names come from strum::VariantNames, not a bespoke table"
        );
        assert!(tokens.contains("\"Off\""));
        assert!(tokens.contains("\"Soft\""));
        assert!(
            tokens.contains("with_variants"),
            "the enum row carries the variant names as field metadata"
        );
        assert!(
            tokens.contains("StyleValue :: Variant"),
            "the row's value is the active variant id"
        );
        assert!(
            tokens.contains("Default :: default"),
            "switching to a payload variant rebuilds it from Default"
        );
        assert!(
            tokens.contains("\"radius\"") && tokens.contains("\"samples\""),
            "payload fields become dotted tokens"
        );
    }

    /// A payload field can be a non-token: it stays part of the value (and is
    /// rebuilt from Default) but is not collected or written.
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

    /// A single-field tuple payload is transparent: the payload's own StyleNode
    /// emits its fields at the enum row's path, with no field-name header.
    #[test]
    fn a_single_tuple_payload_is_transparent() {
        let tokens = expand(parse_quote! {
            enum Mode {
                Soft(Payload),
            }
        });

        assert!(tokens.contains("StyleNode :: collect"));
        assert!(
            tokens.contains("StyleNode :: write"),
            "payload writes are routed transparently"
        );
        assert!(
            !tokens.contains("\"0\""),
            "the macro does not invent an index for the unnamed field"
        );
    }

    /// Several tuple fields have no names, so there is no stable token path for
    /// each one.
    #[test]
    fn a_multi_field_tuple_payload_is_rejected() {
        let error = expand_style_fields(parse_quote! {
            enum Mode {
                Soft(f32, u32),
            }
        })
        .expect_err("two tuple fields have no names");

        assert!(error.to_string().contains("exactly one field"), "{error}");
    }

    #[test]
    fn a_shared_declaration_requires_a_title() {
        let error = expand_style_fields(parse_quote! {
            #[SharedStyle(TOOLBAR = "toolbar")]
            struct ToolBarStyle {
                height: f32,
            }
        })
        .expect_err("a Preferences page heading is required");

        assert!(error.to_string().contains("title"));
    }
}
