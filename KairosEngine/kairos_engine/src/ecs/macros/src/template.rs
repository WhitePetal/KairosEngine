use kairos_macro_utils::fq_std::FQDefault;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Fields, Ident, Index, Path, Result, parse::ParseStream, parse_macro_input,
    spanned::Spanned,
};

use crate::kairos_ecs_path;

const TEMPLATE_DEFAULT_ATTRIBUTE: &str = "default";
const TEMPLATE_ATTRIBUTE: &str = "template";
const BUILT_IN_ATTRIBUTE: &str = "built_in";

pub(crate) fn derive_from_template(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ecs = kairos_ecs_path();

    let type_ident = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let template_ident = format_ident!("{type_ident}Template");

    let is_pub = matches!(ast.vis, syn::Visibility::Public(_));
    let maybe_pub = if is_pub { quote!(pub) } else { quote!() };

    let template = match &ast.data {
        Data::Struct(data_struct) => {
            let result = match struct_impl(&data_struct.fields, &ecs, false) {
                Ok(result) => result,
                Err(err) => return err.into_compile_error().into(),
            };
            let StructImpl {
                template_fields,
                template_field_builds,
                template_field_defaults,
                template_field_clones,
                ..
            } = result;
            match &data_struct.fields {
                Fields::Named(_) => {
                    quote! {
                        #[allow(missing_docs)]
                        #maybe_pub struct #template_ident #impl_generics #where_clause {
                            #(#template_fields,)*
                        }

                        impl #impl_generics #ecs::template::Template for #template_ident #type_generics #where_clause {
                            type Output = #type_ident #type_generics;
                            fn build_template(&self, context: &mut #ecs::template::TemplateContext) -> #ecs::error::Result<Self::Output> {
                                #ecs::error::Result::Ok(#type_ident {
                                    #(#template_field_builds,)*
                                })
                            }

                            fn clone_template(&self) -> Self {
                                Self {
                                    #(#template_field_clones,)*
                                }
                            }
                        }

                        impl #impl_generics #FQDefault for #template_ident #type_generics #where_clause {
                            fn default() -> Self {
                                Self {
                                    #(#template_field_defaults,)*
                                }
                            }
                        }
                    }
                }
                Fields::Unnamed(fields_unnamed) => todo!(),
                Fields::Unit => todo!(),
            }
        }
        Data::Enum(data_enum) => todo!(),
        Data::Union(data_union) => todo!(),
    };

    todo!()
}

struct StructImpl {
    template_fields: Vec<proc_macro2::TokenStream>,
    template_field_builds: Vec<proc_macro2::TokenStream>,
    template_field_defaults: Vec<proc_macro2::TokenStream>,
    template_field_clones: Vec<proc_macro2::TokenStream>,
}

enum TemplateType {
    FromTemplate,
    BuildIn,
    Manual(Path),
}

fn struct_impl(fields: &Fields, ecs: &Path, is_enum: bool) -> Result<StructImpl> {
    let mut template_fields = Vec::with_capacity(fields.len());
    let mut template_field_builds = Vec::with_capacity(fields.len());
    let mut template_field_defaults = Vec::with_capacity(fields.len());
    let mut template_field_clones = Vec::with_capacity(fields.len());
    let is_named = matches!(fields, Fields::Named(_));
    for (index, field) in fields.iter().enumerate() {
        let is_pub = matches!(field.vis, syn::Visibility::Public(_));
        let field_maybe_pub = if is_pub { quote!(pub) } else { quote!() };
        let ident = &field.ident;
        let ty = &field.ty;
        let index = Index::from(index);
        let mut template_type = TemplateType::FromTemplate;
        for attr in &field.attrs {
            if attr.path().is_ident(TEMPLATE_ATTRIBUTE) {
                attr.parse_args_with(|stream: ParseStream| {
                    let forked = stream.fork();
                    let ident = forked.parse::<Ident>()?;
                    if ident == BUILT_IN_ATTRIBUTE {
                        stream.parse::<Ident>()?;
                        template_type = TemplateType::BuildIn;
                    } else {
                        if let Ok(path) = stream.parse::<Path>() {
                            template_type = TemplateType::Manual(path);
                        } else {
                            return Err(syn::Error::new(
                                attr.span(),
                                "Expected a Template type path",
                            ));
                        }
                    }
                    Ok(())
                })?;
            }
        }

        let template_type = match template_type {
            TemplateType::FromTemplate => {
                quote!(<#ty as #ecs::template::FromTemplate>::Template)
            }
            TemplateType::BuildIn => {
                quote!(<#ty as #ecs::template::BuiltInTemplate>::Template)
            }
            TemplateType::Manual(path) => quote! {#path},
        };

        if is_named {
            template_fields.push(quote! {
                #field_maybe_pub #ident: #template_type
            });
            if is_enum {
                template_field_builds.push(quote! {
                    #ident: #ident.build_template(context)?
                });
                template_field_clones.push(quote! {
                    #ident: #ecs::template::Template::clone_template(#ident)
                });
            } else {
                template_field_builds.push(quote! {
                    #ident: self.#ident.build_template(context)?
                });
                template_field_clones.push(quote! {
                    #ident: #ecs::template::Template::clone_template(&self.#ident)
                });
            }

            template_field_defaults.push(quote! {
                #ident: #FQDefault::default()
            });
        } else {
            template_fields.push(quote! {
                #field_maybe_pub #template_type
            });
            if is_enum {
                let enum_tuple_ident = format_ident!("t{}", index);
                template_field_builds.push(quote! {
                    #enum_tuple_ident.build_template(context)?
                });
                template_field_clones.push(quote! {
                    #ecs::template::Template::clone_template(#enum_tuple_ident)
                });
            } else {
                template_field_builds.push(quote! {
                    self.#index.build_template(context)?
                });
                template_field_clones.push(quote! {
                    #ecs::template::Template::clone_template(&self.#index)
                });
            }
            template_field_defaults.push(quote! {
                #FQDefault::default()
            });
        }
    }
    Ok(StructImpl {
        template_fields,
        template_field_builds,
        template_field_defaults,
        template_field_clones,
    })
}
