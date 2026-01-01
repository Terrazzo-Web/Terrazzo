use quote::ToTokens as _;
use quote::format_ident;
use quote::quote;
use syn::visit_mut;
use syn::visit_mut::VisitMut;

mod tests;

pub fn envelope2(
    _attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut item: syn::Item = match syn::parse2(item) {
        Ok(item) => item,
        Err(err) => return err.into_compile_error(),
    };

    let mut visitor = EnvelopeVisitor::default();
    visitor.visit_item_mut(&mut item);
    let items = visitor.items;
    return quote! { #(#items)* };
}

/// From
/// #[derive(PartialEq, Eq, Hash)]
/// struct MyStruct {
///   field: String,
///   other: i32,
/// }
///
/// to
/// #[derive(PartialEq, Eq, Hash)]
/// struct MyStructPtr(Arc<MyStruct>);
///
/// struct MyStructInner {
///   field: String,
///   other: i32,
/// }
///
/// impl Deref + AsRef
#[derive(Default)]
struct EnvelopeVisitor {
    items: Vec<proc_macro2::TokenStream>,
}

impl VisitMut for EnvelopeVisitor {
    fn visit_item_mut(&mut self, i: &mut syn::Item) {
        self.items.push(i.to_token_stream());
        visit_mut::visit_item_mut(self, i);
    }

    fn visit_item_struct_mut(&mut self, i: &mut syn::ItemStruct) {
        let i = i.clone();
        let name = i.ident;
        let name_ptr = format_ident!("{name}Ptr");

        let generics = &i.generics;
        let where_clause = &generics.where_clause;
        let without_defaults = without_defaults(generics);
        let param_names_only = param_names_only(generics);
        let derives = get_derives(i.attrs);

        self.items.push(quote! {
            #derives
            struct #name_ptr #generics #where_clause {
                inner: ::std::sync::Arc<#name #param_names_only>
            }

            impl #without_defaults ::std::ops::Deref
            for #name_ptr #param_names_only
            #where_clause {
                type Target = #name #param_names_only;

                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl #without_defaults ::core::convert::AsRef<#name #param_names_only>
            for #name_ptr #param_names_only
            #where_clause {
                fn as_ref(&self) -> &#name #param_names_only {
                    &self.inner
                }
            }

            impl #without_defaults ::core::clone::Clone
            for #name_ptr #param_names_only
            #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        inner: self.inner.clone()
                    }
                }
            }

            impl #without_defaults From<#name #param_names_only>
            for #name_ptr #param_names_only
            #where_clause {
                fn from(inner: #name #param_names_only) -> Self {
                    Self { inner: inner.into() }
                }
            }
        });
    }
}

fn get_derives(attrs: Vec<syn::Attribute>) -> proc_macro2::TokenStream {
    for attr in attrs {
        if let syn::Meta::List(list) = attr.meta
            && let Some(maybe_derive) = list.path.get_ident()
            && *maybe_derive == "derive"
        {
            let tokens = list.tokens;
            let Ok(valid): syn::Result<syn::ExprArray> = syn::parse2(quote! { [ #tokens ] }) else {
                continue;
            };

            let valid = valid
                .elems
                .into_iter()
                .filter_map(|elem| {
                    let syn::Expr::Path(elem) = elem else {
                        return None;
                    };
                    let elem = elem.path.get_ident()?;
                    match elem.to_string().as_str() {
                        "Default" | "Debug" | "PartialEq" | "Eq" | "PartialOrd" | "Ord"
                        | "Hash" => Some(elem.clone()),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>();

            return quote! {
                #[derive( #(#valid,)* )]
            };
        }
    }
    quote! {}
}

fn param_names_only(generics: &syn::Generics) -> proc_macro2::TokenStream {
    let syn::Generics {
        lt_token: Some(lt_token),
        params,
        gt_token: Some(gt_token),
        where_clause: _,
    } = generics
    else {
        return quote!();
    };
    syn::AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: lt_token.to_owned(),
        args: params
            .into_iter()
            .map(|param| match param {
                syn::GenericParam::Lifetime(x) => {
                    syn::GenericArgument::Lifetime(x.lifetime.clone())
                }
                syn::GenericParam::Type(syn::TypeParam { ident, .. }) => {
                    syn::GenericArgument::Type(syn::parse2(quote! { #ident }).unwrap())
                }
                syn::GenericParam::Const(syn::ConstParam { ident, .. }) => {
                    syn::GenericArgument::Const(syn::parse2(quote! { #ident }).unwrap())
                }
            })
            .collect(),
        gt_token: gt_token.to_owned(),
    }
    .into_token_stream()
}

fn without_defaults(generics: &syn::Generics) -> syn::Generics {
    let mut generics = generics.clone();
    for param in &mut generics.params {
        match param {
            syn::GenericParam::Lifetime(syn::LifetimeParam { attrs, .. }) => {
                *attrs = vec![];
            }
            syn::GenericParam::Type(syn::TypeParam {
                attrs,
                eq_token,
                default,
                ..
            }) => {
                *attrs = vec![];
                *eq_token = None;
                *default = None;
            }
            syn::GenericParam::Const(syn::ConstParam {
                attrs,
                eq_token,
                default,
                ..
            }) => {
                *attrs = vec![];
                *eq_token = None;
                *default = None;
            }
        }
    }
    generics
}
