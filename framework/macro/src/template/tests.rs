#![cfg(test)]

use quote::quote;

use super::template;
use crate::item_to_string;

#[test]
fn simple_node() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(key = "root", "Root text")
        }
    };
    let expected = r#"
fn sample(__generated_template: <XElement as IsTemplated>::Template) -> Consumers {
    let __generated_template1 = __generated_template.clone();
    make_reactive_closure()
        .named("sample")
        .closure(move || {
            let __generated_template = __generated_template.clone();
            let generated_body = move || { div(key = "root", "Root text") };
            __generated_template.apply(generated_body)
        })
        .register(__generated_template1)
}"#;
    let actual = template(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}

#[test]
fn with_signal() -> syn::Result<()> {
    let sample = quote! {
        fn sample(#[signal] signal: String) -> XElement {
            div(key = "root", "Root text")
        }
    };
    let expected = r#"
fn sample(
    __generated_template: <XElement as IsTemplated>::Template,
    signal: XSignal<String>,
) -> Consumers {
    let __generated_template1 = __generated_template.clone();
    make_reactive_closure()
        .named("sample")
        .closure(move || {
            let __generated_template = __generated_template.clone();
            move |signal: String| {
                let generated_body = move || { div(key = "root", "Root text") };
                __generated_template.apply(generated_body)
            }
        })
        .bind(signal)
        .register(__generated_template1)
}"#;
    let actual = template(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}

#[test]
fn with_2_signals() -> syn::Result<()> {
    let sample = quote! {
        fn sample(#[signal] signal1: String, #[signal] signal2: &'static str) -> XElement {
            div(key = "root", "{signal1}", "{signal2}")
        }
    };
    let expected = r#"
fn sample(
    __generated_template: <XElement as IsTemplated>::Template,
    signal1: XSignal<String>,
    signal2: XSignal<&'static str>,
) -> Consumers {
    let __generated_template1 = __generated_template.clone();
    make_reactive_closure()
        .named("sample")
        .closure(move || {
            let __generated_template = __generated_template.clone();
            move |signal1: String| {
                move |signal2: &'static str| {
                    let generated_body = move || {
                        div(key = "root", "{signal1}", "{signal2}")
                    };
                    __generated_template.apply(generated_body)
                }
            }
        })
        .bind(signal1)
        .bind(signal2)
        .register(__generated_template1)
}"#;
    let actual = template(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}

#[test]
fn with_2_mutable_signals() -> syn::Result<()> {
    let sample = quote! {
        fn sample(
            #[signal] mut signal1: String,
            #[signal] mut signal2: &'static str,
            constant: i32,
        ) -> XElement {
            div(key = "root", "{signal1}", "{signal2}")
        }
    };
    let expected = r#"
fn sample(
    __generated_template: <XElement as IsTemplated>::Template,
    signal1: XSignal<String>,
    signal2: XSignal<&'static str>,
    constant: i32,
) -> Consumers {
    let __generated_template1 = __generated_template.clone();
    let signal1_mut = signal1.clone();
    let signal2_mut = signal2.clone();
    make_reactive_closure()
        .named("sample")
        .closure(move || {
            let __generated_template = __generated_template.clone();
            let signal1_mut = MutableSignal::from(&signal1_mut);
            let signal2_mut = MutableSignal::from(&signal2_mut);
            let constant = constant.clone();
            move |signal1: String| {
                move |signal2: &'static str| {
                    let generated_body = move || {
                        div(key = "root", "{signal1}", "{signal2}")
                    };
                    __generated_template.apply(generated_body)
                }
            }
        })
        .bind(signal1)
        .bind(signal2)
        .register(__generated_template1)
}"#;
    let actual = template(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}

#[test]
fn tagged() -> syn::Result<()> {
    let sample = quote! {
        /// Docs
        pub fn sample(arg: &str) -> XElement {
            div(key = "root", "Root text")
        }
    };
    let expected = r#"
/// Docs
pub fn sample(arg: &str) -> XElement {
    #[doc(hidden)]
    fn __sample_aux(
        __generated_template: <XElement as IsTemplated>::Template,
        arg: &str,
    ) -> Consumers {
        let __generated_template1 = __generated_template.clone();
        make_reactive_closure()
            .named("sample")
            .closure(move || {
                let __generated_template = __generated_template.clone();
                let arg = arg.clone();
                let generated_body = move || { div(key = "root", "Root text") };
                __generated_template.apply(generated_body)
            })
            .register(__generated_template1)
    }
    XElement {
        tag_name: Some("div".into()),
        key: XKey::default(),
        value: XElementValue::Dynamic(
            (move |element| __sample_aux(element, arg.clone())).into(),
        ),
        before_render: None,
        after_render: None,
    }
}"#;
    let actual = template(quote! { tag = div }, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}

#[test]
fn attribute() -> syn::Result<()> {
    let sample = quote! {
        /// Docs
        pub fn sample(#[signal] arg: String) -> XAttributeValue {
            format!("KEY={arg}")
        }
    };
    let expected = r#"
/// Docs
pub fn sample(arg: XSignal<String>) -> impl Fn(XAttributeTemplate) -> Consumers {
    #[doc(hidden)]
    fn __sample_aux(
        __generated_template: <XAttributeValue as IsTemplated>::Template,
        arg: XSignal<String>,
    ) -> Consumers {
        let __generated_template1 = __generated_template.clone();
        make_reactive_closure()
            .named("sample")
            .closure(move || {
                let __generated_template = __generated_template.clone();
                move |arg: String| {
                    let generated_body = move || { format!("KEY={arg}") };
                    __generated_template.apply(generated_body)
                }
            })
            .bind(arg)
            .register(__generated_template1)
    }
    move |element| __sample_aux(element, arg.clone())
}"#;
    let actual = template(quote! { wrap = true }, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
    Ok(())
}
