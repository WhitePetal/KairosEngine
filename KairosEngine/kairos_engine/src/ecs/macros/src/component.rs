//! `#[derive(Component)]` 的展开逻辑。

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Error, LitStr, Result};

/// 组件存储方式,对应 `kairos_engine::ecs::component::StorageType`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Storage {
    Table,
    SparseSet,
}

impl Storage {
    fn from_lit(lit: &LitStr) -> Result<Self> {
        match lit.value().as_str() {
            "Table" => Ok(Self::Table),
            "SparseSet" => Ok(Self::SparseSet),
            other => Err(Error::new(
                lit.span(),
                format!(
                    "unknown storage type `{other}`: expected `\"Table\"` or `\"SparseSet\"`"
                ),
            )),
        }
    }
}

/// 解析后的 `#[component(...)]` 配置。
#[derive(Clone, Copy)]
struct Config {
    storage: Storage,
    immutable: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            storage: Storage::Table,
            immutable: false,
        }
    }
}

impl Config {
    fn from_attrs(attrs: &[Attribute]) -> Result<Self> {
        let mut config = Self::default();
        for attr in attrs {
            if !attr.path().is_ident("component") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("storage") {
                    let lit: LitStr = meta.value()?.parse()?;
                    config.storage = Storage::from_lit(&lit)?;
                } else if meta.path.is_ident("immutable") {
                    config.immutable = true;
                } else {
                    return Err(meta.error("unsupported `#[component(...)]` option"));
                }
                Ok(())
            })?;
        }
        Ok(config)
    }
}

/// 生成 `#[derive(Component)]` 展开后的代码。
///
/// 生成的 impl 引用 `::kairos_engine::ecs::component::Component`,
/// 因此该宏只能用于依赖 `kairos_engine` 的 crate。
pub fn derive_component_impl(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let config = Config::from_attrs(&input.attrs)?;

    let storage = match config.storage {
        Storage::Table => quote! { ::kairos_engine::ecs::component::StorageType::Table },
        Storage::SparseSet => quote! { ::kairos_engine::ecs::component::StorageType::SparseSet },
    };
    let mutability = if config.immutable {
        quote! { ::kairos_engine::ecs::component::Immutable }
    } else {
        quote! { ::kairos_engine::ecs::component::Mutable }
    };

    Ok(quote! {
        impl #impl_generics ::kairos_engine::ecs::component::Component for #ident #ty_generics #where_clause {
            const STORAGE_TYPE: ::kairos_engine::ecs::component::StorageType = #storage;
            type Mutability = #mutability;
        }
    })
}
