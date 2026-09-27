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

// TODO: The generated run should return an Ok() of Result<App, Error> since evaluating the components may throw
#[test]
fn coerce_result() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            async fn comp1() -> Result<Comp1, Error> {
                Ok(Comp1::new())
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error> {
                Ok(Comp2::new())
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Result<Comp1, Error> {
        Ok(Comp1::new())
    }
    #[doc(hidden)]
    fn comp2_impl(comp1: Comp1) -> Result<Comp1, Error> {
        Ok(Comp2::new())
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> App { // TODO: this should be returning Result<App, Error>, there is no ambiguity on the returned error type
        let comp1 = (comp1_impl().await)?;
        let comp2 = comp2_impl(comp1.clone())?;
        return run_impl(name, comp1, comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

// TODO: The generation should produce compilation errors because the error types are inconsistent
#[test]
fn coerce_result_failure() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            async fn comp1() -> Result<Comp1, Error1> {
                Ok(Comp1::new())
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error2> {
                Ok(Comp2::new())
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Result<Comp1, Error1> {
        Ok(Comp1::new())
    }
    #[doc(hidden)]
    fn comp2_impl(comp1: Comp1) -> Result<Comp2, Error2> {
        Ok(Comp2::new())
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> App { // TODO: there should be a compilation error added to this generated run function since it's undefined which error type should be returned.
        let comp1 = (comp1_impl().await)?;
        let comp2 = comp2_impl(comp1.clone())?;
        return run_impl(name, comp1, comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

// TODO: The generation should assume Error1 and Error2 are From/Into convertible into Error and the ? operator will just work. There is no ambiguity that the error type should be Error and not Error1 or Error2 since this is what is declared on the original run function. The current assertion is thus correct, this is how the generated code should look like.
#[test]
fn coerce_result2() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> Result<App, Error> {
                Ok(App { name, comp1, comp2 })
            }

            async fn comp1() -> Result<Comp1, Error1> {
                Ok(Comp1::new())
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error2> {
                Ok(Comp2::new())
            }
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Result<Comp1, Error1> {
        Ok(Comp1::new())
    }
    #[doc(hidden)]
    fn comp2_impl(comp1: Comp1) -> Result<Comp2, Error2> {
        Ok(Comp2::new())
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> Result<App, Error> {
        Ok(App { name, comp1, comp2 })
    }
    pub async fn run(name: String) -> Result<App, Error> {
        let comp1 = (comp1_impl().await)?;
        let comp2 = comp2_impl(comp1.clone())?;
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
        let comp2 = comp2_impl(Ok(comp1.clone()));
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
        let comp1 = comp1_impl().await;
        let comp2 = comp2_impl(Ok(comp1.clone()));
        return run_impl(name, comp2, comp1);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_async3() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: impl Future<Output = Comp1>, comp2: Comp2) -> App {
                App { name, comp2 }
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
            }

            async fn comp2() -> Comp2 {
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
    async fn comp2_impl() -> Comp2 {
        Comp2::new()
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: impl Future<Output = Comp1>, comp2: Comp2) -> App {
        App { name, comp2 }
    }
    pub async fn run(name: String) -> App {
        let comp1 = comp1_impl();
        let comp2 = comp2_impl().await;
        return run_impl(name, comp1, comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_ref() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: &Comp1, comp2: &Comp2) -> App {
                App { name, comp1, comp2 }
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
            }

            fn comp2(comp1: Comp1) -> Comp2 {
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
    fn comp2_impl(comp1: Comp1) -> Comp2 {
        Comp2::new()
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: &Comp1, comp2: &Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> App {
        let comp1 = comp1_impl().await;
        let comp2 = comp2_impl(comp1.clone());
        return run_impl(name, &comp1, &comp2);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

// TODO: comp2 and comp3 can be awaited in parallel using tokio::join!
#[test]
fn parallel_eval() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2, comp3: Comp3) -> App {}
            async fn comp1() -> Comp1 {}
            async fn comp2(comp1: Comp1) -> Comp2 {}
            async fn comp3() -> Comp3 {}
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Comp1 {}
    #[doc(hidden)]
    async fn comp2_impl(comp1: Comp1) -> Comp2 {}
    #[doc(hidden)]
    async fn comp3_impl() -> Comp3 {}
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2, comp3: Comp3) -> App {}
    pub async fn run(name: String) -> App {
        let comp1 = comp1_impl().await;
        let comp2 = comp2_impl(comp1.clone()).await;
        let comp3 = comp3_impl().await;
        return run_impl(name, comp1, comp2, comp3);
    }
}"#;
    run_test(quote! {}, sample, expected);
}

// TODO: comp2 and comp3 could also be awaited in parallel using tokio::join!, but this is more complicated. I will instruct you to address this TODO later
#[test]
fn parallel_eval2() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: &Comp2, comp3: Comp3) -> App {}
            async fn comp1() -> Comp1 {}
            async fn comp2(comp1: Comp1) -> Comp2 {}
            async fn comp3() -> Result<Comp3, String> {}
        }
    };
    let expected = r#"
mod make_app {
    #[doc(hidden)]
    async fn comp1_impl() -> Comp1 {}
    #[doc(hidden)]
    async fn comp2_impl(comp1: Comp1) -> Comp2 {}
    #[doc(hidden)]
    async fn comp3_impl() -> Result<Comp3, String> {}
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: &Comp2, comp3: Comp3) -> App {}
    pub async fn run(name: String) -> App {
        let comp1 = comp1_impl().await;
        let comp2 = comp2_impl(comp1.clone()).await;
        let comp3 = (comp3_impl().await)?;
        return run_impl(name, comp1, &comp2, comp3);
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
