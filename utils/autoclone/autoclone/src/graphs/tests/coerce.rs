use std::rc::Rc;

use quote::quote;

use crate::graphs::return_type::ReturnType;

#[test]
fn coerce_trivial() {
    let from = make_type(quote! { () });
    let into = make_type(quote! { () });
    let coerce = from.coerce(&into, quote! { a });
    assert_eq(
        r#"
fn actual() {
    a
}"#,
        coerce,
    );
}

#[test]
fn coerce_future_result_to_rc() {
    let from = make_type(quote! { Future<Output = Result<String, i32>> });
    let into = make_type(quote! { Box<String> });
    let coerce = from.coerce(&into, quote! { a });
    assert_eq(
        r#"
fn actual() {
    Box::new((a.await)?)
}"#,
        coerce,
    );
}

#[test]
fn coerce_future_to_result_rc() {
    let from = make_type(quote! { Future<Output = String> });
    let into = make_type(quote! { Result<Rc<str>, Error> });
    let coerce = from.coerce(&into, quote! { a });
    assert_eq(
        r#"
fn actual() {
    Ok(Rc::new(a.await))
}"#,
        coerce,
    );
}

#[test]
fn coerce_future_to_rc_result() {
    let from = make_type(quote! { Future<Output = String> });
    let into = make_type(quote! { Rc<Result<str, Error>> });
    let coerce = from.coerce(&into, quote! { a });
    assert_eq(
        r#"
fn actual() {
    Rc::new(Ok(a.await))
}"#,
        coerce,
    );
}

fn make_type(tokens: proc_macro2::TokenStream) -> Rc<ReturnType> {
    ReturnType::from(&syn::parse2::<syn::Type>(tokens).unwrap()).into()
}

#[track_caller]
fn assert_eq(expected: &str, actual: proc_macro2::TokenStream) {
    let actual = quote! { fn actual() { #actual } };
    let actual = syn::parse2(actual.clone())
        .map(|item| crate::item_to_string(&item))
        .unwrap_or_else(|error| format!("Error {error}\nParsing {actual}"));
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
}
