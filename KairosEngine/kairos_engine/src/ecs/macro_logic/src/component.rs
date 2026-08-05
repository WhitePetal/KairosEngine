use std::collections::HashSet;

use kairos_macro_utils::fq_std::FQDefault;
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Data, DataStruct, DeriveInput, Expr, ExprCall, ExprPath, Field, Fields, LitStr, Member, Path,
    Result, Token, Type, braced, parenthesized,
    parse::Parse,
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Brace, Comma, Paren},
};

use crate::{component::kw::relationship, map_entities::MapEntitiesAttributeKind};

const COMPONENT: &str = "component";
const MAP_ENTITIES: &str = "map_entities";
const STORAGE: &str = "storage";
const REQUIRE: &str = "require";
const RELATIONSHIP: &str = "relationship";
const RELATIONSHIP_TARGET: &str = "relationship_target";

const ON_ADD: &str = "on_add";
const ON_INSERT: &str = "on_insert";
const ON_DISCARD: &str = "on_discard";
const ON_REMOVE: &str = "on_remove";
const ON_DESPAWN: &str = "on_despawn";

const IMMUTABLE: &str = "immutable";
const CLONE_BEHAVIOR: &str = "clone_behavior";

mod kw {
    syn::custom_keyword!(relationship_target);
    syn::custom_keyword!(relationship);
    syn::custom_keyword!(linked_spawn);
    syn::custom_keyword!(allow_self_referential);
}

/// Whether the derive macro may contain a `component(storage = "…")` attribute.
pub enum StorageAttribute {
    /// User can overwrite the storage type
    Allowed,
    /// User cannot overwrite the storage type
    Disallowed,
}

/// The derived component storage type
#[derive(Clone, Copy)]
pub enum StorageTy {
    /// Table storage
    Table,
    /// Sparse set storage
    SparseSet,
}

// values for `storage` attribute
const TABLE: &str = "Table";
const SPARSE_SET: &str = "SparseSet";

/// Derived required component from the `#[require]` attribute.
pub struct Require {
    path: Path,
    func: Option<TokenStream>,
}

impl Parse for Require {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let mut path = input.parse::<Path>()?;
        let mut last_segment_is_lower = false;
        let mut is_constructor_call = false;

        // Use the case of the type name to check if it's an enum
        // This doesn't match everything that can be an enum according to the rust spec
        // but it matches what clippy is OK with
        let is_enum = {
            let mut first_chars = path
                .segments
                .iter()
                .rev()
                .filter_map(|s| s.ident.to_string().chars().next());
            if let Some(last) = first_chars.next() {
                if last.is_uppercase() {
                    if let Some(last) = first_chars.next() {
                        last.is_uppercase()
                    } else {
                        false
                    }
                } else {
                    last_segment_is_lower = true;
                    false
                }
            } else {
                false
            }
        };

        let func = if input.peek(Token![=]) {
            // If there is an '=', then this is a "function style" require
            input.parse::<Token![=]>()?;
            let expr: Expr = input.parse()?;
            Some(quote! {|| #expr})
        } else if input.peek(Brace) {
            // This is a "value style" named-struct-like require
            let content;
            braced!(content in input);
            let content = content.parse::<TokenStream>()?;
            Some(quote! {|| #path { #content }})
        } else if input.peek(Paren) {
            // This is a "value style" tuple-struct-like require
            let content;
            parenthesized!(content in input);
            let content = content.parse::<TokenStream>()?;
            is_constructor_call = last_segment_is_lower;
            Some(quote! {|| #path (#content)})
        } else if is_enum {
            // if this is an enum, then it is an inline enum component declaration
            Some(quote!(|| #path))
        } else {
            None
        };
        if is_enum || is_constructor_call {
            path.segments.pop();
            path.segments.pop_punct();
        }
        Ok(Require { path, func })
    }
}

/// All allowed attribute value expression kinds for component hooks.
/// This doesn't simply use general expressions because of conflicting needs:
/// - we want to be able to use `Self` & generic parameters in paths
/// - call expressions producing a closure need to be wrapped in a function
///   to turn them into function pointers, which prevents access to the outer generic params
#[derive(Debug)]
pub enum HookAttributeKind {
    /// expressions like function or struct names
    ///
    /// structs will throw compile errors on the code generation so this is safe
    Path(ExprPath),
    /// function call like expressions
    Call(ExprCall),
}

impl HookAttributeKind {
    fn parse(
        input: syn::parse::ParseStream,
        default_hook_path: impl FnOnce() -> ExprPath,
    ) -> Result<Self> {
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            input.parse::<Expr>().and_then(Self::from_expr)
        } else {
            Ok(Self::Path(default_hook_path()))
        }
    }

    fn from_expr(value: Expr) -> Result<Self> {
        match value {
            Expr::Path(path) => Ok(HookAttributeKind::Path(path)),
            Expr::Call(call) => Ok(HookAttributeKind::Call(call)),
            _ => Err(syn::Error::new(
                value.span(),
                [
                    "Not supported in this position, please use one of the following:",
                    "- path to function",
                    "- call to function yielding closure",
                ]
                .join("\n"),
            )),
        }
    }

    fn to_token_stream(&self, ecs_path: &Path) -> TokenStream {
        match self {
            HookAttributeKind::Path(path) => path.to_token_stream(),
            HookAttributeKind::Call(call) => {
                quote! {
                    fn _internal_hook(world: #ecs_path::world::DeferredWorld, ctx: #ecs_path::lifecycle::HookContext) {
                        (#call)(world, ctx)
                    }
                    _internall_hook
                }
            }
        }
    }
}

/// Derived `#[relationship]` attribute information.
pub struct Relationship {
    relationship_target: Type,
    allow_self_referential: bool,
}

impl Parse for Relationship {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let mut relationship_target: Option<Type> = None;
        let mut allow_self_referential: bool = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::allow_self_referential) {
                input.parse::<kw::allow_self_referential>()?;
                allow_self_referential = true;
            } else if lookahead.peek(kw::relationship_target) {
                input.parse::<kw::relationship_target>()?;
                input.parse::<Token![=]>()?;
                relationship_target = Some(input.parse()?);
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Relationship {
            relationship_target: relationship_target.ok_or_else(|| {
                syn::Error::new(input.span(), "Missing `relationship_target = X` attribute")
            })?,
            allow_self_referential,
        })
    }
}

/// Derived `#[relationship_target]` attribute information.
pub struct RelationshipTarget {
    relationship: Type,
    linked_spawn: bool,
}

impl Parse for RelationshipTarget {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let mut relationship: Option<Type> = None;
        let mut linked_spawn: bool = false;

        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::linked_spawn) {
                input.parse::<kw::linked_spawn>()?;
                linked_spawn = true;
            } else if lookahead.peek(kw::relationship) {
                input.parse::<kw::relationship>()?;
                input.parse::<Token![=]>()?;
                relationship = Some(input.parse()?);
            } else {
                return Err(lookahead.error());
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(RelationshipTarget {
            relationship: relationship.ok_or_else(|| {
                syn::Error::new(input.span(), "Missing `relationship = X` attribute")
            })?,
            linked_spawn,
        })
    }
}

/// Derived `Component` trait specification, which can be used to generate a component implementation.
pub struct DeriveComponent {
    /// The storage type of the component.
    pub storage: Option<StorageTy>,
    /// The parsed punctuated list of required components.
    pub requires: Option<Punctuated<Require, Comma>>,
    /// The `on_add` hook.
    pub on_add: Option<HookAttributeKind>,
    /// The `on_insert` hook.
    pub on_insert: Option<HookAttributeKind>,
    /// The `on_discard` hook.
    pub on_discard: Option<HookAttributeKind>,
    /// The `on_remove` hook.
    pub on_remove: Option<HookAttributeKind>,
    /// The `on_despawn` hook.
    pub on_despawn: Option<HookAttributeKind>,
    /// The relationship attribute information.
    pub relationship: Option<Relationship>,
    /// The relationship target attribute information.
    pub relationship_target: Option<RelationshipTarget>,
    /// Whether or not this component is immutable.
    pub immutable: bool,
    /// The clone behavior for this component.
    pub clone_behavior: Option<Expr>,
    /// The `map_entities` attribute information.
    pub map_entities: Option<MapEntitiesAttributeKind>,
    /// Additional required component registrations that are added in `Component::register_required_components`
    pub additional_requires: Vec<TokenStream>,
}

impl DeriveComponent {
    /// Parse [`DeriveComponent`] from the given `ast`.
    pub fn parse(ast: &DeriveInput, storage_attr: StorageAttribute) -> Result<DeriveComponent> {
        let mut attrs = DeriveComponent {
            storage: None,
            on_add: None,
            on_insert: None,
            on_discard: None,
            on_remove: None,
            on_despawn: None,
            requires: None,
            relationship: None,
            relationship_target: None,
            immutable: false,
            clone_behavior: None,
            map_entities: None,
            additional_requires: Vec::new(),
        };

        let mut requir_paths = HashSet::new();
        for attr in ast.attrs.iter() {
            if attr.path().is_ident(COMPONENT) {
                attr.parse_nested_meta(|nested| {
                    if let StorageAttribute::Allowed = storage_attr
                        && nested.path.is_ident(STORAGE)
                    {
                        attrs.storage = Some(match nested.value()?.parse::<LitStr>()?.value() {
                            s if s == TABLE => StorageTy::Table,
                            s if s == SPARSE_SET => StorageTy::SparseSet,
                            s => {
                                return Err(nested.error(format!(
                                    "Invalid storage type '{s}', expected '{TABLE}' or '{SPARSE_SET}'.",
                                )));
                            }
                        });
                        Ok(())
                    } else if nested.path.is_ident(ON_ADD) {
                        attrs.on_add = Some(HookAttributeKind::parse(nested.input, || {
                            parse_quote! { Self::on_add }
                        })?);
                        Ok(())
                    } else if nested.path.is_ident(ON_INSERT) {
                        attrs.on_insert = Some(HookAttributeKind::parse(nested.input, || {
                            parse_quote! { Self::on_insert }
                        })?);
                        Ok(())
                    } else if nested.path.is_ident(ON_DISCARD) {
                        attrs.on_discard = Some(HookAttributeKind::parse(nested.input, || {
                            parse_quote! { Self::on_discard }
                        })?);
                        Ok(())
                    } else if nested.path.is_ident(ON_REMOVE) {
                        attrs.on_remove = Some(HookAttributeKind::parse(nested.input, || {
                            parse_quote! { Self::on_remove }
                        })?);
                        Ok(())
                    } else if nested.path.is_ident(ON_DESPAWN) {
                        attrs.on_despawn = Some(HookAttributeKind::parse(nested.input, || {
                            parse_quote! { Self::on_despawn }
                        })?);
                        Ok(())
                    } else if nested.path.is_ident(IMMUTABLE) {
                        attrs.immutable = true;
                        Ok(())
                    } else if nested.path.is_ident(CLONE_BEHAVIOR) {
                        attrs.clone_behavior = Some(nested.value()?.parse()?);
                        Ok(())
                    } else if nested.path.is_ident(MAP_ENTITIES) {
                        attrs.map_entities =
                            Some(nested.input.parse::<MapEntitiesAttributeKind>()?);
                        Ok(())
                    } else {
                        Err(nested.error("Unsupported attribute"))
                    }
                })?;
            } else if attr.path().is_ident(REQUIRE) {
                let punctuated =
                    attr.parse_args_with(Punctuated::<Require, Comma>::parse_terminated)?;
                for require in punctuated.iter() {
                    if !requir_paths.insert(require.path.to_token_stream().to_string()) {
                        return Err(syn::Error::new(
                            require.path.span(),
                            "Duplicate required components are not allowed.",
                        ));
                    }
                }
                if let Some(current) = &mut attrs.requires {
                    current.extend(punctuated);
                } else {
                    attrs.requires = Some(punctuated);
                }
            } else if attr.path().is_ident(RELATIONSHIP) {
                let relationship = attr.parse_args::<Relationship>()?;
                attrs.relationship = Some(relationship);
            } else if attr.path().is_ident(RELATIONSHIP_TARGET) {
                let relationship_target = attr.parse_args::<RelationshipTarget>()?;
                attrs.relationship_target = Some(relationship_target);
            }
        }

        if attrs.relationship_target.is_some() && attrs.clone_behavior.is_some() {
            return Err(syn::Error::new(
                attrs.clone_behavior.span(),
                "A Relationship Target already has its own clone behavior, please remove 'clone_behavior = ...'",
            ));
        }

        Ok(attrs)
    }

    /// Generates a new `Component` trait implementation from this specification.
    ///
    /// Note that this will add Send + Sync + 'static to the where clause
    pub fn impl_component(
        self,
        ast: &mut DeriveInput,
        ecs: &Path,
        default_storage: StorageTy,
    ) -> Result<TokenStream> {
        // We want to raise a compile time error when the generic lifetimes
        // are not bound to 'static lifetime
        let non_static_lifetime_error = ast
            .generics
            .lifetimes()
            .filter(|lifetime| !lifetime.bounds.iter().any(|bound| bound.ident == "static"))
            .map(|param| syn::Error::new(param.span(), "Lifetimes must be 'static"))
            .reduce(|mut err_acc, err| {
                err_acc.combine(err);
                err_acc
            });
        if let Some(err) = non_static_lifetime_error {
            return Err(err);
        }

        let relationship = match self.derive_relationship(ast, ecs) {
            Ok(value) => value,
            Err(err) => Some(err.into_compile_error()),
        };
    }

    fn derive_relationship(&self, ast: &DeriveInput, ecs: &Path) -> Result<Option<TokenStream>> {
        let Some(relationship) = &self.relationship else {
            return Ok(None);
        };
        let Data::Struct(DataStruct {
            fields,
            struct_token,
            ..
        }) = &ast.data
        else {
            return Err(syn::Error::new(
                ast.span(),
                "Relationship can only be derived for structs",
            ));
        };
        let field = relationship_field(fields, "Relationship", struct_token.span())?;

        let relationship_member = field.ident.clone().map_or(Member::from(0), Member::Named);
        let members = fields
            .members()
            .filter(|member| member != &relationship_member);

        let struct_name = &ast.ident;
        let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

        let relationship_target = &relationship.relationship_target;
        let allow_self_referential = relationship.allow_self_referential;

        let fqdefault = FQDefault.into_token_stream();

        Ok(Some(quote! {}))
    }
}

/// Returns the field with the `#[relationship]` attribute, the only field if unnamed,
/// or the only field in a [`Fields::Named`] with one field, otherwise `Err`.
pub(crate) fn relationship_field<'a>(
    fields: &'a Fields,
    derive: &'static str,
    span: Span,
) -> Result<&'a Field> {
    match fields {
        Fields::Named(fields) if fields.named.len() == 1 => Ok(fields.named.first().unwrap()),
        Fields::Named(fields) => fields.named.iter().find(|field| {
            field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident(RELATIONSHIP))
        }).ok_or(syn::Error::new(
            span,
            format!("{derive} derive expected named structs with a single field or with a field annotated with #[relationship].")
        )),
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(fields.unnamed.first().unwrap()),
        Fields::Unnamed(fields) => fields.unnamed.iter().find(|field| {
            field
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident(RELATIONSHIP))
        })
        .ok_or(syn::Error::new(
            span,
            format!("{derive} derive expected unnamed structs with one field or with a field annotated with #[relationship].")
        )),
        Fields::Unit => Err(syn::Error::new(
            span,
            format!("{derive} derive expected named or unnamed struct, found unit struct.")
        )),
    }
}
