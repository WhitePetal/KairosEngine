//! Macros for deriving ECS traits.

#![cfg_attr(docsrs, feature(doc_cfg))]

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

extern crate proc_macro;

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
