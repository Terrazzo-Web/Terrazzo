use quote::quote;
use syn::visit_mut::VisitMut;

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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub fn run(self) -> App {
            let Self { name } = self;
            run(name)
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_result() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error> {
                Ok(Comp2::new())
            }

            async fn comp1() -> Result<Comp1, Error> {
                Ok(Comp1::new())
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
    fn comp2_impl(comp1: Comp1) -> Result<Comp2, Error> {
        Ok(Comp2::new())
    }
    #[doc(hidden)]
    fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
        App { name, comp1, comp2 }
    }
    pub async fn run(name: String) -> Result<App, Error> {
        let comp1 = (comp1_impl().await)?;
        let comp2 = comp2_impl(comp1.clone())?;
        return Ok(run_impl(name, comp1, comp2));
    }
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> Result<App, Error> {
            let Self { name } = self;
            run(name).await
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_result_failure() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
                App { name, comp1, comp2 }
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error2> {
                Ok(Comp2::new())
            }

            async fn comp1() -> Result<Comp1, Error1> {
                Ok(Comp1::new())
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
    pub async fn run(name: String) -> Result<App, Error1> {
        compile_error!(
            "Cannot infer graph error type: conflicting error types `Error1` and `Error2`; declare an explicit Result return type"
        );
        let comp1 = (comp1_impl().await)?;
        let comp2 = comp2_impl(comp1.clone())?;
        return Ok(run_impl(name, comp1, comp2));
    }
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> Result<App, Error1> {
            let Self { name } = self;
            run(name).await
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn coerce_result2() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> Result<App, Error> {
                Ok(App { name, comp1, comp2 })
            }

            fn comp2(comp1: Comp1) -> Result<Comp2, Error2> {
                Ok(Comp2::new())
            }

            async fn comp1() -> Result<Comp1, Error1> {
                Ok(Comp1::new())
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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> Result<App, Error> {
            let Self { name } = self;
            run(name).await
        }
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

            fn comp2(comp1: Result<Comp1, Error>) -> Comp2 {
                Comp2::new()
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> App {
            let Self { name } = self;
            run(name).await
        }
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

            fn comp2(comp1: Result<Comp1, Error>) -> Comp2 {
                Comp2::new()
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> App {
            let Self { name } = self;
            run(name).await
        }
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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> App {
            let Self { name } = self;
            run(name).await
        }
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

            fn comp2(comp1: Comp1) -> Comp2 {
                Comp2::new()
            }

            async fn comp1() -> Comp1 {
                Comp1::new()
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
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> App {
            let Self { name } = self;
            run(name).await
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn parallel_eval() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: Comp2, comp3: Comp3) -> App {}
            async fn comp2(comp1: Comp1) -> Comp2 {}
            async fn comp1() -> Comp1 {}
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
        let (comp2, comp3) = tokio::join!(comp2_impl(comp1.clone()), comp3_impl());
        return run_impl(name, comp1, comp2, comp3);
    }
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> App {
            let Self { name } = self;
            run(name).await
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn parallel_eval2() {
    let sample = quote! {
        mod make_app {
            pub fn run(name: String, comp1: Comp1, comp2: &Comp2, comp3: Comp3) -> App {}
            async fn comp2(comp1: Comp1) -> Comp2 {}
            async fn comp1() -> Comp1 {}
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
    pub async fn run(name: String) -> Result<App, String> {
        let comp1 = comp1_impl().await;
        let (comp2, comp3) = tokio::join!(comp2_impl(comp1.clone()), comp3_impl());
        let comp3 = comp3?;
        return Ok(run_impl(name, comp1, &comp2, comp3));
    }
    pub struct Run {
        pub name: String,
    }
    impl Run {
        pub async fn run(self) -> Result<App, String> {
            let Self { name } = self;
            run(name).await
        }
    }
}"#;
    run_test(quote! {}, sample, expected);
}

#[test]
fn topological_order_accepts_shared_dependencies_and_external_inputs() {
    let sample = quote! {
        mod graph {
            pub fn run(left: usize, right: usize) -> usize { left + right }
            fn unrelated() {}
            pub fn left(shared: usize) -> usize { shared }
            const OFFSET: usize = 1;
            fn right(shared: usize) -> usize { shared + OFFSET }
            fn shared(mut input: usize) -> usize { input }
        }
    };
    super::super::graph2(quote! {}, sample).unwrap();
}

#[test]
fn topological_order_rejects_backward_dependencies() {
    for sample in [
        quote! {
            mod graph {
                fn helper() -> usize { 1 }
                pub fn run(helper: usize) -> usize { helper }
            }
        },
        quote! {
            mod graph {
                pub fn run(helper: usize) -> usize { helper }
                fn leaf() -> usize { 1 }
                fn helper(leaf: usize) -> usize { leaf }
            }
        },
        quote! {
            mod graph {
                fn leaf() -> usize { 1 }
                fn helper(leaf: usize) -> usize { leaf }
            }
        },
        quote! {
            mod graph {
                pub fn run(helper: usize) -> usize { helper }
                pub fn helper(run: usize) -> usize { run }
            }
        },
        quote! {
            mod graph {
                pub fn run(run: usize) -> usize { run }
            }
        },
    ] {
        let error = super::super::graph2(quote! {}, sample).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must be declared before its dependency"),
            "{error}"
        );
    }
}

#[test]
fn topological_order_diagnostic_names_caller_and_dependency() {
    let error = super::super::graph2(
        quote! {},
        quote! {
            mod graph {
                fn helper() -> usize { 1 }
                pub fn run(helper: usize) -> usize { helper }
            }
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Graph functions must be declared in topological order: `run` must be declared before its dependency `helper`"
    );
}

#[track_caller]
fn run_test(args: proc_macro2::TokenStream, sample: proc_macro2::TokenStream, expected: &str) {
    let actual = super::super::graph2(args, sample).unwrap();
    let actual = syn::parse2(actual.clone())
        .map(|mut item| {
            RemoveCfgAttr.visit_item_mut(&mut item);
            item_to_string(&item)
        })
        .unwrap_or_else(|error| format!("Error {error}\nParsing {actual}"));
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
}

struct RemoveCfgAttr;

impl syn::visit_mut::VisitMut for RemoveCfgAttr {
    fn visit_attributes_mut(&mut self, i: &mut Vec<syn::Attribute>) {
        i.retain(|syn::Attribute { meta, .. }| {
            if let syn::Meta::List(syn::MetaList { path, .. }) = meta
                && path.is_ident("cfg_attr")
            {
                false
            } else {
                true
            }
        });
    }
}
