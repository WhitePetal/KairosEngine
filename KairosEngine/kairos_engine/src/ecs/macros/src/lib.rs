//! Derive macros for the KairosEngine ECS.
//!
//! 目前提供两个宏:
//!
//! - `#[derive(Component)]`:为类型实现
//!   [`Component`](`kairos_engine::ecs::component::Component`) trait,并支持
//!   `#[component(storage = "SparseSet")]` / `#[component(immutable)]` 等属性。
//! - `#[derive(Resource)]`:为类型实现
//!   [`Resource`](`kairos_engine::ecs::resource::Resource`) trait(同时实现
//!   `Component`,可独立使用)。
//!
//! 生成代码中引用的路径基于 `::kairos_engine::...`,因此这些宏面向
//! `kairos_engine` crate 及其下游使用者。宏由 `kairos_engine` 的
//! `ecs::component` / `ecs::resource` 模块 re-export,使用者
//! `use kairos_engine::ecs::component::Component;` 即可配合 `#[derive(...)]` 使用。

mod component;
mod resource;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

/// 为类型实现 [`Component`](`kairos_engine::ecs::component::Component`)。
///
/// 支持的 `#[component(...)]` 选项:
///
/// - `storage = "Table" | "SparseSet"` — 组件存储方式,默认 `"Table"`。
/// - `immutable` — 将组件标记为不可变(对应 `Component::Mutability = Immutable`)。
///
/// 其它选项(如 `map_entities`、hooks、`#[require(...)]`)尚未实现,遇到时会
/// 报错而不是静默忽略,便于逐步补齐。
#[proc_macro_derive(Component, attributes(component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    component::derive_component_impl(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// 为类型实现 [`Resource`](`kairos_engine::ecs::resource::Resource`)。
///
/// `Resource` 是 `Component` 的子 trait,因此这里同时生成 `Component` 与
/// `Resource` 两个 impl,`#[derive(Resource)]` 可独立使用(不要与
/// `#[derive(Component)]` 叠加使用,否则会生成重复的 `Component` impl)。
#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    resource::derive_resource_impl(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::parse2;

    fn parse_derive(input: TokenStream) -> DeriveInput {
        parse2(input).expect("failed to parse derive input")
    }

    #[test]
    fn component_generates_default_impl() {
        let input = parse_derive(quote! { struct Health(u32); });
        let out = component::derive_component_impl(&input).unwrap().to_string();
        assert!(out.contains(":: kairos_engine :: ecs :: component :: Component"), "{out}");
        assert!(out.contains("StorageType :: Table"), "{out}");
        assert!(out.contains(":: kairos_engine :: ecs :: component :: Mutable"), "{out}");
    }

    #[test]
    fn component_respects_storage_and_immutable() {
        let input = parse_derive(quote! {
            #[component(storage = "SparseSet", immutable)]
            struct Chunk;
        });
        let out = component::derive_component_impl(&input).unwrap().to_string();
        assert!(out.contains("StorageType :: SparseSet"), "{out}");
        assert!(out.contains(":: kairos_engine :: ecs :: component :: Immutable"), "{out}");
    }

    #[test]
    fn component_supports_generics() {
        let input = parse_derive(quote! { struct Handle<T: Clone>(T); });
        let out = component::derive_component_impl(&input).unwrap().to_string();
        assert!(out.contains("impl < T : Clone >"), "{out}");
        assert!(out.contains("Component for Handle < T >"), "{out}");
    }

    #[test]
    fn unknown_storage_type_is_rejected() {
        let input = parse_derive(quote! {
            #[component(storage = "Whatever")]
            struct A;
        });
        let err = component::derive_component_impl(&input).unwrap_err();
        assert!(err.to_string().contains("unknown storage type"), "{err}");
    }

    #[test]
    fn unknown_component_option_is_rejected() {
        let input = parse_derive(quote! {
            #[component(map_entities)]
            struct A;
        });
        let err = component::derive_component_impl(&input).unwrap_err();
        assert!(err.to_string().contains("unsupported"), "{err}");
    }

    #[test]
    fn resource_generates_component_and_resource_impls() {
        let input = parse_derive(quote! { struct Settings { value: u32 } });
        let out = resource::derive_resource_impl(&input).unwrap().to_string();
        assert!(out.contains(":: kairos_engine :: ecs :: resource :: Resource"), "{out}");
        assert!(out.contains(":: kairos_engine :: ecs :: component :: Component"), "{out}");
    }
}
