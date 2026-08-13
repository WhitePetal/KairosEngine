//! Macros for deriving ECS traits.

#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate proc_macro;

use kairos_macro_utils::{
    KairosManifest, ensure_no_collision, fq_std::FQResult, get_struct_fields,
};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{ToTokens, format_ident, quote};
use syn::{
    ConstParam, DeriveInput, GenericParam, TypeParam, parse_macro_input, parse_quote,
    punctuated::Punctuated, token::Comma,
};

/// Cheat sheet for derive syntax,
/// see full explanation and examples on the `Component` trait doc.
///
/// ## Immutability
/// ```ignore
/// #[derive(Component)]
/// #[component(immutable)]
/// struct MyComponent;
/// ```
///
/// ## Sparse instead of table-based storage
/// ```ignore
/// #[derive(Component)]
/// #[component(storage = "SparseSet")]
/// struct MyComponent;
/// ```
///
/// ## Required Components
///
/// ```ignore
/// #[derive(Component)]
/// #[require(
///     // `Default::default()`
///     A,
///     // tuple structs
///     B(1),
///     // named-field structs
///     C {
///         x: 1,
///         ..default()
///     },
///     // unit structs/variants
///     D::One,
///     // associated consts
///     E::ONE,
///     // constructors
///     F::new(1),
///     // arbitrary expressions
///     G = make(1, 2, 3)
/// )]
/// struct MyComponent;
/// ```
///
/// ## Relationships
/// ```ignore
/// #[derive(Component)]
/// #[relationship(relationship_target = Children)]
/// pub struct ChildOf {
///     // Marking the field is not necessary if there is only one.
///     #[relationship]
///     pub parent: Entity,
///     internal: u8,
/// };
///
/// #[derive(Component)]
/// #[relationship_target(relationship = ChildOf)]
/// pub struct Children(Vec<Entity>);
/// ```
///
/// On despawn, also despawn all related entities:
/// ```ignore
/// #[derive(Component)]
/// #[relationship_target(relationship = ChildOf, linked_spawn)]
/// pub struct Children(Vec<Entity>);
/// ```
///
/// Allow relationships to point to their own entity:
/// ```ignore
/// #[derive(Component)]
/// #[relationship(relationship_target = PeopleILike, allow_self_referential)]
/// pub struct LikedBy(pub Entity);
/// ```
/// ## Warning
///
/// When `allow_self_referential` is enabled, be careful when using recursive traversal methods
/// like `iter_ancestors` or `root_ancestor`, as they will loop infinitely if an entity points to itself.
///
/// ## Hooks
/// ```ignore
/// #[derive(Component)]
/// #[component(hook_name = function)]
/// struct MyComponent;
/// ```
/// where `hook_name` is `on_add`, `on_insert`, `on_discard` or `on_remove`;
/// `function` can be either a path, e.g. `some_function::<Self>`,
/// or a function call that returns a function that can be turned into
/// a `ComponentHook`, e.g. `get_closure("Hi!")`.
/// `function` can be elided if the path is `Self::on_add`, `Self::on_insert` etc.
///
/// ## Ignore this component when cloning an entity
/// ```ignore
/// #[derive(Component)]
/// #[component(clone_behavior = Ignore)]
/// struct MyComponent;
/// ```
#[proc_macro_derive(
    Component,
    attributes(component, require, relationship, relationship_target, entities)
)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    todo!()
}

/// Implement `SystemParam` to use a struct as a parameter in a system
#[proc_macro_derive(SystemParam, attributes(system_param))]
pub fn derive_system_param(input: TokenStream) -> TokenStream {
    let token_stream = input.clone();
    let ast = parse_macro_input!(input as DeriveInput);

    match derive_system_param_impl(token_stream, ast) {
        Ok(t) => t,
        Err(e) => e.into_compile_error().into(),
    }
}
fn derive_system_param_impl(
    token_stream: TokenStream,
    ast: DeriveInput,
) -> syn::Result<TokenStream> {
    let fields = get_struct_fields(&ast.data, "derive(SystemParam)")?;
    let path = kairos_ecs_path();

    let field_locals = fields
        .members()
        .map(|m| format_ident!("field{}", m))
        .collect::<Vec<_>>();
    let field_members = fields.members().collect::<Vec<_>>();
    let field_types = fields.iter().map(|f| &f.ty).collect::<Vec<_>>();

    let field_validation_names = fields.members().map(|m| format!("::{}", quote! { #m }));
    let mut field_validation_messages = Vec::with_capacity(fields.len());
    for attr in fields
        .iter()
        .map(|f| f.attrs.iter().find(|a| a.path().is_ident("system_param")))
    {
        let mut field_validation_message = None;
        if let Some(attr) = attr {
            attr.parse_nested_meta(|nested| {
                if nested.path.is_ident("validation_message") {
                    field_validation_message = Some(nested.value()?.parse()?);
                    Ok(())
                } else {
                    Err(nested.error("Unsupported attribute"))
                }
            })?;
        }
        field_validation_messages
            .push(field_validation_message.unwrap_or_else(|| quote! { err.message }));
    }

    let generics = ast.generics;

    // Emit an error if there's any unrecognized lifetime names.
    let w = format_ident!("w");
    let s = format_ident!("s");
    for lt in generics.lifetimes() {
        let ident = &lt.lifetime.ident;
        if ident != &w && ident != &s {
            return Err(syn::Error::new_spanned(
                lt,
                r#"invalid lifetime name: expected `'w` or `'s`
'w -- refers to data stored in the World.
's -- refers to data stored in the SystemParam's state."#,
            ));
        }
    }

    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let lifetimeless_generics: Vec<_> = generics
        .params
        .iter()
        .filter(|g| !matches!(g, GenericParam::Lifetime(_)))
        .collect();

    let shadowed_lifetimes: Vec<_> = generics.lifetimes().map(|_| quote!('_)).collect();

    let mut punctuated_generics = Punctuated::<_, Comma>::new();
    punctuated_generics.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => GenericParam::Type(TypeParam {
            default: None,
            ..g.clone()
        }),
        GenericParam::Const(g) => GenericParam::Const(ConstParam {
            default: None,
            ..g.clone()
        }),
        _ => unreachable!(),
    }));

    let mut punctuated_generic_idents = Punctuated::<_, Comma>::new();
    punctuated_generic_idents.extend(lifetimeless_generics.iter().map(|g| match g {
        GenericParam::Type(g) => &g.ident,
        GenericParam::Const(g) => &g.ident,
        _ => unreachable!(),
    }));

    let punctuated_generics_no_bounds: Punctuated<_, Comma> = lifetimeless_generics
        .iter()
        .map(|&g| match g.clone() {
            GenericParam::Type(mut g) => {
                g.bounds.clear();
                GenericParam::Type(g)
            }
            g => g,
        })
        .collect();

    let mut tuple_types: Vec<_> = field_types.iter().map(ToTokens::to_token_stream).collect();
    let mut tuple_patterns: Vec<_> = field_locals.iter().map(ToTokens::to_token_stream).collect();

    // If the number of fields exceeds the 16-parameter limit,
    // fold the fields into tuples of tuples until we are below the limit.
    const LIMIT: usize = 16;
    while tuple_types.len() > LIMIT {
        let end = Vec::from_iter(tuple_types.drain(..LIMIT));
        tuple_types.push(parse_quote!( (#(#end,)*) ));

        let end = Vec::from_iter(tuple_patterns.drain(..LIMIT));
        tuple_patterns.push(parse_quote!( (#(#end,)*) ));
    }
    // Create a where clause for the `ReadOnlySystemParam` impl.
    // Ensure that each field implements `ReadOnlySystemParam`.
    let mut read_only_generics = generics.clone();
    let read_only_where_clause = read_only_generics.make_where_clause();
    for field_type in &field_types {
        read_only_where_clause
            .predicates
            .push(syn::parse_quote!(#field_type: #path::system::ReadOnlySystemParam));
    }

    let fields_alias =
        ensure_no_collision(format_ident!("__StructFieldsAlias"), token_stream.clone());

    let struct_name = &ast.ident;
    let state_struct_visibility = &ast.vis;
    let state_struct_name = ensure_no_collision(format_ident!("FetchState"), token_stream);

    let mut builder_name = None;
    for meta in ast
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("system_param"))
    {
        meta.parse_nested_meta(|nested| {
            if nested.path.is_ident("builder") {
                builder_name = Some(format_ident!("{struct_name}Builder"));
                Ok(())
            } else {
                Err(nested.error("Unsupported attribute"))
            }
        })?;
    }

    let builder = builder_name.map(|builder_name| {
        let builder_type_parameters: Vec<Ident> = field_members.iter().map(|m| format_ident!("B{}", m)).collect();
        let builder_doc_comment = format!("A [`SystemParamBuilder`] for a [`{struct_name}`].");
        let builder_struct = quote! {
            #[doc = #builder_doc_comment]
            struct #builder_name<#(#[allow(non_camel_case_types, reason = "generated from snake-case field name")] #builder_type_parameters,)*> {
                #(#field_members: #builder_type_parameters,)*
            }
        };
        let lifetimes: Vec<_> = generics.lifetimes().collect();
        let generic_struct = quote! { #struct_name <#(#lifetimes,)* #punctuated_generic_idents> };
        let builder_impl = quote! {
            // SAFETY: This delegates to the `SystemParamBuilder` for tuples.
            unsafe impl<
                #(#lifetimes,)*
                #(#[allow(non_camel_case_types, reson = "generated from snake-case field name")] #builder_type_parameters: #path::system::SystemParamBuilder<#field_types>,)*
                #punctuated_generics
            > #path::system::SystemParamBuilder<#generic_struct> for #builder_name<#(#builder_type_parameters,)*>
                #where_clause
            {
                fn build(self, world: &mut path::world::World) -> <#generic_struct as #path::system::SystemParam>::State {
                    let #builder_name { #(#field_members: #field_locals,)* } = self;
                    #state_struct_name {
                        state: #path::system::SystemParamBuilder::build((#(#tuple_patterns,)*), world)
                    }
                }
            }
        };
        (builder_struct, builder_impl)
    });
    let (builder_struct, builder_impl) = builder.unzip();

    Ok(TokenStream::from(quote! {
        // We define the FetchState struct in an anonymous scope to avoid polluting the user namespace.
        // The struct can still be accessed via SystemParam::State, e.g. MessageReaderState can be accessed via
        // <MessageReader<'static, 'static, T> as SystemParam>::State
        const _: () = {
            // Allows rebinding the lifetimes of each field type.
            type #fields_alias <'w, 's, #punctuated_generics_no_bounds> = (#(#tuple_types,)*);

            #[doc(hidden)]
            #state_struct_visibility struct #state_struct_name <#(#lifetimeless_generics,)*>
            #where_clause {
                state: <#fields_alias::<'static, 'static, #punctuated_generic_idents> as #path::system::SystemParam>::State,
            }

            unsafe impl<#punctuated_generics> #path::system::SystemParam for
                #struct_name <#(#shadowed_lifetimes,)* #punctuated_generic_idents> #where_clause
            {
                type State = #state_struct_name<#punctuated_generic_idents>;
                type Item<'w, 's> = #struct_name #ty_generics;

                fn init_state(world: &mut #path::world::World) -> Self::State {
                    #state_struct_name {
                        state: <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::system::SystemParam>::init_state(world),
                    }
                }

                fn init_access(state: &Self::State, system_meta: &mut #path::system::SystemMeta, component_access_set: &mut #path::query::FilteredAccessSet, world: &mut #path::world::World) {
                    <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::system::SystemParam>::init_access(&state.state, system_meta, component_access_set, world);
                }

                fn apply(state: &mut Self::State, system_meta: &#path::system::SystemMeta, world: &mut #path::world::World) {
                    <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::system::SystemParam>::apply(&mut state.state, system_meta, world);
                }

                fn queue(state: &mut Self::State, system_meta: &#path::system::SystemMeta, world: #path::world::DeferredWorld) {
                    <#fields_alias::<'_, '_, #punctuated_generic_idents> as #path::system::SystemParam>::queue(&mut state.state, system_meta, world);
                }

                unsafe fn get_param<'w, 's>(
                    state: &'s mut Self::State,
                    system_meta: &#path::system::SystemMeta,
                    world: #path::world::unsafe_world_cell::UnsafeWorldCell<'w>,
                    change_tick: #path::change_detection::Tick
                ) -> #FQResult<Self::Item<'w, 's>, #path::system::SystemParamValidationError> {
                    let (#(#tuple_patterns,)*) = &mut state.state;
                    #(
                        let #field_locals = unsafe {
                            <#field_types as #path::system::SystemParam>::get_param(#field_locals, system_meta, world, change_tick)
                        }.map_err(|err| #path::system::SystemParamValidationError::new::<Self>(err.skipped, #field_validation_messages, #field_validation_names))?;
                    )*
                    #FQResult::Ok(#struct_name {
                        #(#field_members: #field_locals,)*
                    })
                }
            }

            // Safety: Each field is `ReadOnlySystemParam`, so this can only read from the `World`
            unsafe impl<'w, 's, #punctuated_generics> #path::system::ReadOnlySystemParam for #struct_name #ty_generics #read_only_where_clause {}

            #builder_impl;
        };

        #builder_struct
    }))
}

/// Return the path to the Kairos ECS module, relative to the caller's crate.
///
/// The ECS is a module of the `kairos_engine` crate (`kairos_engine::ecs`),
/// so this resolves the engine crate from the caller's manifest and appends
/// the `ecs` module segment. Macro expansions can then append further module
/// paths, e.g. `#kairos_ecs_path::world::World`.
pub(crate) fn kairos_ecs_path() -> syn::Path {
    KairosManifest::shared(|manifest| {
        let mut path = manifest.get_path("kairos_engine");
        path.segments
            .push(KairosManifest::parse_str::<syn::PathSegment>("ecs"));
        path
    })
}

// TODO!
