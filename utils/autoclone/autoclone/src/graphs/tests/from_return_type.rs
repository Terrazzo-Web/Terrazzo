use std::rc::Rc;

use quote::quote;

use crate::graphs::return_type::RefKind;
use crate::graphs::return_type::ReturnType;

#[test]
fn from_return_type_value() {
    let ty = make_type(quote! { Vec<String> });
    let return_type = ReturnType::T(ty.clone());
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_unit() {
    let ty = make_type(quote! { () });
    assert_eq!(ty.as_ref(), &syn::Type::from(&ReturnType::Unit));
}

#[test]
fn from_return_type_void_fn() {
    let func: syn::ItemFn = syn::parse2(quote! { fn x() {} }).unwrap();
    let return_type = ReturnType::from(&func.sig.output);
    assert_eq!(
        make_type(quote! { () }).as_ref(),
        &syn::Type::from(&return_type)
    );
}

#[test]
fn from_return_type_value_fn() {
    let func: syn::ItemFn = syn::parse2(quote! { fn x() -> Box<String> {} }).unwrap();
    let return_type = ReturnType::from(&func.sig.output);
    assert_eq!(
        make_type(quote! { Box<String> }).as_ref(),
        &syn::Type::from(&return_type)
    );
}

#[test]
fn from_return_type_ref() {
    let ty = make_type(quote! { &String });
    let return_type = ReturnType::Ref {
        kind: RefKind::Ref,
        ty: Rc::new(ReturnType::T(make_type(quote! { String }))),
    };
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_box() {
    let ty = make_type(quote! { Box<String> });
    let return_type = ReturnType::Ref {
        kind: RefKind::Box,
        ty: Rc::new(ReturnType::T(make_type(quote! { String }))),
    };
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_arc() {
    let ty = make_type(quote! { Arc<String> });
    let return_type = ReturnType::Ref {
        kind: RefKind::Arc,
        ty: Rc::new(ReturnType::T(make_type(quote! { String }))),
    };
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_rc() {
    let ty = make_type(quote! { Rc<String> });
    let return_type = ReturnType::Ref {
        kind: RefKind::Rc,
        ty: Rc::new(ReturnType::T(make_type(quote! { String }))),
    };
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_future() {
    let ty = make_type(quote! { Future<Output = String> });
    let return_type = ReturnType::Future(Rc::new(ReturnType::T(make_type(quote! { String }))));
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_result() {
    let ty = make_type(quote! { Result<Vec<String>, std::io::Error> });
    let return_type = ReturnType::Result(
        Rc::new(ReturnType::T(make_type(quote! { Vec<String> }))),
        Rc::new(ReturnType::T(make_type(quote! { std::io::Error }))),
    );
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

#[test]
fn from_return_type_combo() {
    let ty = make_type(quote! {
        Future<Output =
            Result<
                Rc<
                    Vec<String>
                >,
                Box<std::io::Error>
            >
        >
    });
    let return_type = ReturnType::Future(Rc::new(ReturnType::Result(
        Rc::new(ReturnType::Ref {
            kind: RefKind::Rc,
            ty: Rc::new(ReturnType::T(make_type(quote! { Vec<String> }))),
        }),
        Rc::new(ReturnType::Ref {
            kind: RefKind::Box,
            ty: Rc::new(ReturnType::T(make_type(quote! { std::io::Error }))),
        }),
    )));
    assert_eq!(ty.as_ref(), &syn::Type::from(&return_type));
}

fn make_type(tokens: proc_macro2::TokenStream) -> Rc<syn::Type> {
    Rc::new(syn::parse2(tokens).unwrap())
}
