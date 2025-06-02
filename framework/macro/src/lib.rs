#![doc = include_str!("../README.md")]

use quote::format_ident;

mod arguments;
mod html;
use server_fn_macro::server_macro_impl;
mod template;

#[proc_macro_attribute]
pub fn html(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    self::html::html(attr.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn template(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    self::template::template(attr.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn server(
    args: proc_macro::TokenStream,
    s: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    match server_macro_impl(
        args.into(),
        s.into(),
        Some(syn::parse_quote!(::server_fn)),
        "/api/fn",
        None,
        None,
    ) {
        Err(e) => e.to_compile_error().into(),
        Ok(s) => s.into(),
    }
}

fn item_to_string(item: &syn::Item) -> String {
    prettyplease::unparse(&syn::File {
        shebang: None,
        attrs: vec![],
        items: vec![item.clone()],
    })
}

fn is_path(path: &syn::Path, expected: &'static str) -> bool {
    let syn::Path {
        leading_colon: None,
        segments,
    } = path
    else {
        return false;
    };
    let Some(segment) = segments.last() else {
        return false;
    };
    if segments.len() != 1 {
        return false;
    }
    let syn::PathArguments::None = segment.arguments else {
        return false;
    };
    return segment.ident == format_ident!("{}", expected);
}
