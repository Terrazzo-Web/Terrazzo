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
        let without_defaults = without_defaults(generics);
        let param_names_only = param_names_only(generics);
        let derives = get_derives(i.attrs);

        self.items.push(quote! {
            #derives
            struct #name_ptr #generics {
                inner: ::std::sync::Arc<#name #param_names_only>
            }

            impl #without_defaults ::std::ops::Deref for #name_ptr #param_names_only {
                type Target = #name #param_names_only;

                fn deref(&self) -> &Self::Target {
                    &self.inner
                }
            }

            impl #without_defaults ::core::convert::AsRef<#name #param_names_only> for #name_ptr #param_names_only {
                fn as_ref(&self) -> &#name #param_names_only {
                    &self.inner
                }
            }

            impl #without_defaults ::core::clone::Clone for #name_ptr #param_names_only {
                fn clone(&self) -> Self {
                    Self {
                        inner: self.inner.clone()
                    }
                }
            }

            impl #without_defaults From<#name #param_names_only> for #name_ptr #param_names_only {
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

fn param_names_only(generics: &syn::Generics) -> syn::Generics {
    let mut generics = without_defaults(generics);
    for param in &mut generics.params {
        match param {
            syn::GenericParam::Lifetime(syn::LifetimeParam {
                colon_token,
                bounds,
                ..
            }) => {
                *colon_token = None;
                *bounds = syn::punctuated::Punctuated::default()
            }
            syn::GenericParam::Type(syn::TypeParam {
                colon_token,
                bounds,
                ..
            }) => {
                *colon_token = None;
                *bounds = syn::punctuated::Punctuated::default();
            }
            syn::GenericParam::Const(syn::ConstParam {
                eq_token, default, ..
            }) => {
                *eq_token = None;
                *default = None;
            }
        }
    }
    generics
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
            syn::GenericParam::Const(syn::ConstParam { attrs, .. }) => {
                *attrs = vec![];
            }
        }
    }
    generics
}
