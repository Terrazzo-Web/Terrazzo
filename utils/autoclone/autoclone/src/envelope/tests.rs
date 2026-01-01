#![cfg(test)]

use quote::quote;

use crate::item_to_string;

#[test]
fn envelope_struct() {
    let sample = quote! {
        struct MyStruct {
            a: String,
            b: i32,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStruct {
        a: String,
        b: i32,
    }
    struct MyStructPtr {
        inner: ::std::sync::Arc<MyStruct>,
    }
    impl ::std::ops::Deref for MyStructPtr {
        type Target = MyStruct;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl ::core::convert::AsRef<MyStruct> for MyStructPtr {
        fn as_ref(&self) -> &MyStruct {
            &self.inner
        }
    }
    impl ::core::clone::Clone for MyStructPtr {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl From<MyStruct> for MyStructPtr {
        fn from(inner: MyStruct) -> Self {
            Self { inner: inner.into() }
        }
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_generics() {
    let sample = quote! {
        struct MyStruct<T: Clone, U: Default = usize> {
            t: T,
            u: U,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStruct<T: Clone, U: Default = usize> {
        t: T,
        u: U,
    }
    struct MyStructPtr<T: Clone, U: Default = usize> {
        inner: ::std::sync::Arc<MyStruct<T, U>>,
    }
    impl<T: Clone, U: Default> ::std::ops::Deref for MyStructPtr<T, U> {
        type Target = MyStruct<T, U>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<T: Clone, U: Default> ::core::convert::AsRef<MyStruct<T, U>>
    for MyStructPtr<T, U> {
        fn as_ref(&self) -> &MyStruct<T, U> {
            &self.inner
        }
    }
    impl<T: Clone, U: Default> ::core::clone::Clone for MyStructPtr<T, U> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<T: Clone, U: Default> From<MyStruct<T, U>> for MyStructPtr<T, U> {
        fn from(inner: MyStruct<T, U>) -> Self {
            Self { inner: inner.into() }
        }
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_const() {
    let sample = quote! {
        struct MyStruct2<T: Clone, const N: usize, const D: usize = 0> {
            t: T,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStruct2<T: Clone, const N: usize, const D: usize = 0> {
        t: T,
    }
    struct MyStruct2Ptr<T: Clone, const N: usize, const D: usize = 0> {
        inner: ::std::sync::Arc<MyStruct2<T, N, D>>,
    }
    impl<T: Clone, const N: usize, const D: usize> ::std::ops::Deref
    for MyStruct2Ptr<T, N, D> {
        type Target = MyStruct2<T, N, D>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<
        T: Clone,
        const N: usize,
        const D: usize,
    > ::core::convert::AsRef<MyStruct2<T, N, D>> for MyStruct2Ptr<T, N, D> {
        fn as_ref(&self) -> &MyStruct2<T, N, D> {
            &self.inner
        }
    }
    impl<T: Clone, const N: usize, const D: usize> ::core::clone::Clone
    for MyStruct2Ptr<T, N, D> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<T: Clone, const N: usize, const D: usize> From<MyStruct2<T, N, D>>
    for MyStruct2Ptr<T, N, D> {
        fn from(inner: MyStruct2<T, N, D>) -> Self {
            Self { inner: inner.into() }
        }
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_where() {
    let sample = quote! {
        struct MyStruct2<'t, 'tt: 't, T: 't>
        where
            T: Clone,
        {
            t: &'t T,
            tt: &'tt T,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStruct2<'t, 'tt: 't, T: 't>
    where
        T: Clone,
    {
        t: &'t T,
        tt: &'tt T,
    }
    struct MyStruct2Ptr<'t, 'tt: 't, T: 't>
    where
        T: Clone,
    {
        inner: ::std::sync::Arc<MyStruct2<'t, 'tt, T>>,
    }
    impl<'t, 'tt: 't, T: 't> ::std::ops::Deref for MyStruct2Ptr<'t, 'tt, T>
    where
        T: Clone,
    {
        type Target = MyStruct2<'t, 'tt, T>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<'t, 'tt: 't, T: 't> ::core::convert::AsRef<MyStruct2<'t, 'tt, T>>
    for MyStruct2Ptr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn as_ref(&self) -> &MyStruct2<'t, 'tt, T> {
            &self.inner
        }
    }
    impl<'t, 'tt: 't, T: 't> ::core::clone::Clone for MyStruct2Ptr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<'t, 'tt: 't, T: 't> From<MyStruct2<'t, 'tt, T>> for MyStruct2Ptr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn from(inner: MyStruct2<'t, 'tt, T>) -> Self {
            Self { inner: inner.into() }
        }
    }
}
"#;
    run_test(sample, expected);
}

#[track_caller]
fn run_test(sample: proc_macro2::TokenStream, expected: &str) {
    let actual = super::envelope2(quote! {}, sample);
    let actual = syn::parse2(quote! {
        mod envelope { #actual }
    })
    .map(|item| item_to_string(&item))
    .unwrap_or_else(|error| format!("Error {error}\nParsing {actual}"));
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        assert!(false);
    }
}
