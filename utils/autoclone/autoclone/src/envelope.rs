use quote::ToTokens as _;
use quote::format_ident;
use quote::quote;
use syn::visit_mut::VisitMut as _;

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

impl syn::visit_mut::VisitMut for EnvelopeVisitor {
    fn visit_item_mut(&mut self, i: &mut syn::Item) {
        syn::visit_mut::visit_item_mut(self, i);
    }

    fn visit_item_enum_mut(&mut self, i: &mut syn::ItemEnum) {
        let mut public_item = i.to_owned();
        public_item.vis = syn::Visibility::Public(Default::default());
        self.do_visit(
            syn::Item::Enum(public_item),
            &i.ident,
            &i.vis,
            &i.attrs,
            &i.generics,
        );
    }

    fn visit_item_struct_mut(&mut self, i: &mut syn::ItemStruct) {
        let mut public_item = i.to_owned();
        public_item.vis = syn::Visibility::Public(Default::default());
        for field in &mut public_item.fields {
            match &field.vis {
                syn::Visibility::Inherited => {
                    field.vis = syn::parse2(quote! { pub(super) }).expect("pub(super)");
                }
                syn::Visibility::Restricted(syn::VisRestricted {
                    in_token: None,
                    path,
                    ..
                }) => {
                    if let syn::Path {
                        leading_colon: None,
                        segments,
                    } = &**path
                        && segments.len() > 0
                        && segments[0].ident == "super"
                    {
                        field.vis = syn::parse2(quote! { pub(in super :: #path ) }).unwrap_or_else(
                            |error| {
                                panic!(
                                    "Error {error} parsing: pub(super :: {})",
                                    path.to_token_stream().to_string()
                                )
                            },
                        );
                    }
                }
                _ => (),
            }
        }
        self.do_visit(
            syn::Item::Struct(public_item),
            &i.ident,
            &i.vis,
            &i.attrs,
            &i.generics,
        );
    }
}

impl EnvelopeVisitor {
    fn do_visit(
        &mut self,
        public_item: syn::Item,
        name: &syn::Ident,
        vis: &syn::Visibility,
        attrs: &[syn::Attribute],
        generics: &syn::Generics,
    ) {
        let name_ptr = format_ident!("{name}Ptr");
        let name_into = format_ident!("Into{name}");

        let where_clause = &generics.where_clause;
        let without_defaults = without_defaults(generics);
        let param_names_only = param_names_only(generics);
        let derives = get_derives(attrs);

        let with_into = {
            let mut with_into = without_defaults.clone();
            with_into.lt_token.get_or_insert_default();
            with_into
                .params
                .push(syn::GenericParam::Type(syn::TypeParam {
                    attrs: Default::default(),
                    ident: name_into.clone(),
                    colon_token: Some(Default::default()),
                    bounds: [syn::parse2::<syn::TypeParamBound>(
                        quote! { Into<#name #param_names_only> },
                    )
                    .unwrap()]
                    .into_iter()
                    .collect(),
                    eq_token: Default::default(),
                    default: Default::default(),
                }));
            with_into.gt_token.get_or_insert_default();
            with_into
        };
        let inner = syn::Ident::new(&ident_to_snake_case(&name), name.span());
        self.items.push(quote! {
            mod #inner {
                use super::*;
                #public_item
            }
            use #inner::#name;

            #derives
            #vis struct #name_ptr #generics #where_clause {
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

            impl #with_into From<#name_into>
            for #name_ptr #param_names_only
            #where_clause {
                fn from(inner: #name_into) -> Self {
                    Self { inner: inner.into().into() }
                }
            }
        });
    }
}

fn get_derives(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    for attr in attrs {
        if let syn::Meta::List(list) = &attr.meta
            && let Some(maybe_derive) = list.path.get_ident()
            && *maybe_derive == "derive"
        {
            let tokens = &list.tokens;
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

fn ident_to_snake_case(name: &impl std::fmt::Display) -> String {
    let name = name.to_string();
    let mut result = String::default();
    for c in name.chars() {
        if c.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push_str(&c.to_lowercase().to_string());
        } else {
            result.push(c);
        }
    }
    return result;
}
