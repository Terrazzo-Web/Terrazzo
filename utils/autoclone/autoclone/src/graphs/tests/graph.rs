use quote::quote;

use crate::item_to_string;

#[test]
fn simple_graph() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            fn comp1() -> Comp1 {
                Comp1::new()
            }

            fn comp2() -> Comp2 {
                Comp2::new()
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    fn comp1_impl() -> Comp1 {
        Comp1::new()
    }
    #[doc(hidden)]
    fn comp2_impl() -> Comp2 {
        Comp2::new()
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub fn run(name: String) -> App {
        let comp1 = comp1_impl();
        let comp2 = comp2_impl();
        return run_impl(name, comp1, comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_async() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
            }

            fn comp2(comp1: Result<Comp1, Error>) -> Comp2 {
                Comp2::new()
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Comp1 {
        Comp1::new()
    }
    #[doc(hidden)]
    fn comp2_impl(comp1: Result<Comp1, Error>) -> Comp2 {
        Comp2::new()
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> App {
        let comp1 = comp1_impl().await;
        let comp2 = comp2_impl(Ok(comp1));
        return run_impl(name, comp1, comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_async2() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp2: Comp2, comp1: Comp1) -> App {
                App { name, comp1, comp2 }
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
            }

            fn comp2(comp1: Result<Comp1, Error>) -> Comp2 {
                Comp2::new()
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Comp1 {
        Comp1::new()
    }
    #[doc(hidden)]
    fn comp2_impl(comp1: Result<Comp1, Error>) -> Comp2 {
        Comp2::new()
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp2: Comp2, comp1: Comp1) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> App {
        let comp1 = Ok(comp1_impl().await);
        let comp2 = comp2_impl(comp1);
        return run_impl(name, comp2, comp1?);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[track_caller]
fn run_test(args: proc_macro2::TokenStream, sample: proc_macro2::TokenStream, expected: &str) {
    let actual = super::super::graph2(args, sample).unwrap();
    let actual = syn::parse2(actual.clone())
        .map(|item| item_to_string(&item))
        .unwrap_or_else(|error| format!("Error {error}\nParsing {actual}"));
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
}
