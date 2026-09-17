#![cfg(test)]

use std::rc::Rc;

use quote::quote;

use super::return_type::RefKind;
use super::return_type::ReturnType;

#[test]
fn into_return_type_ref() {
    let ty = make_type(quote! { &String });
    let return_type = ReturnType::from(ty.as_ref());
    assert_eq!(
        ReturnType::Ref {
            kind: RefKind::Ref,
            ty: Rc::new(ReturnType::T(make_type(quote! { String })))
        },
        return_type
    );
}

#[test]
fn into_return_type_ref() {
    let ty = make_type(quote! { &String });
    let return_type = ReturnType::from(ty.as_ref());
    assert_eq!(
        ReturnType::Ref {
            kind: RefKind::Ref,
            ty: Rc::new(ReturnType::T(make_type(quote! { String })))
        },
        return_type
    );
}

#[test]
fn into_return_type_box() {
    let ty = make_type(quote! { Box<String> });
    let return_type = ReturnType::from(ty.as_ref());
    assert_eq!(
        ReturnType::Ref {
            kind: RefKind::Box,
            ty: Rc::new(ReturnType::T(make_type(quote! { String })))
        },
        return_type
    );
}

#[test]
fn into_return_type_arc() {
    let ty = make_type(quote! { Arc<String> });
    let return_type = ReturnType::from(ty.as_ref());
    assert_eq!(
        ReturnType::Ref {
            kind: RefKind::Arc,
            ty: Rc::new(ReturnType::T(make_type(quote! { String })))
        },
        return_type
    );
}

#[test]
fn into_return_type_rc() {
    let ty = make_type(quote! { Rc<String> });
    let return_type = ReturnType::from(ty.as_ref());
    assert_eq!(
        ReturnType::Ref {
            kind: RefKind::Rc,
            ty: Rc::new(ReturnType::T(make_type(quote! { String })))
        },
        return_type
    );
}

fn make_type(tokens: proc_macro2::TokenStream) -> Rc<syn::Type> {
    Rc::new(syn::parse2(tokens).unwrap())
}
