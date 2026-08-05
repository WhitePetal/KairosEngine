use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{Expr, ExprPath, Path, Result, Token, parse::Parse, spanned::Spanned};

const ENTITIES: &str = "entities";

/// The type of `MapEntities` attribute.
#[derive(Debug)]
pub enum MapEntitiesAttributeKind {
    /// expressions like function or struct names
    ///
    /// structs will throw compile errors on the code generation so this is safe
    Path(ExprPath),
    /// When no value is specified
    Default,
}

impl MapEntitiesAttributeKind {
    fn from_expr(value: Expr) -> syn::Result<Self> {
        match value {
            Expr::Path(path) => Ok(Self::Path(path)),
            _ => Err(syn::Error::new(
                value.span(),
                [
                    "Not supported in this position, please use one of the following:",
                    "- path to function",
                    "- nothing to default to MapEntities implementation",
                ]
                .join("\n"),
            )),
        }
    }

    fn to_token_stream(&self, ecs_path: &Path) -> TokenStream {
        match self {
            MapEntitiesAttributeKind::Path(path) => path.to_token_stream(),
            MapEntitiesAttributeKind::Default => {
                quote! {
                    <Self as #ecs_path::entity::MapEntities>::map_entities
                }
            }
        }
    }
}

impl Parse for MapEntitiesAttributeKind {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            input.parse::<Expr>().and_then(Self::from_expr)
        } else {
            Ok(Self::Default)
        }
    }
}
