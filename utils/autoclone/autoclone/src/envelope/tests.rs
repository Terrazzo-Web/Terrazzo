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
    impl<IntoMyStruct: Into<MyStruct>> From<IntoMyStruct> for MyStructPtr {
        fn from(inner: IntoMyStruct) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    struct MyStruct {
        a: String,
        b: i32,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_generics() {
    let sample = quote! {
        struct MyStructGenerics<T: Clone, U: Default = usize> {
            t: T,
            u: U,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStructGenericsPtr<T: Clone, U: Default = usize> {
        inner: ::std::sync::Arc<MyStructGenerics<T, U>>,
    }
    impl<T: Clone, U: Default> ::std::ops::Deref for MyStructGenericsPtr<T, U> {
        type Target = MyStructGenerics<T, U>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<T: Clone, U: Default> ::core::convert::AsRef<MyStructGenerics<T, U>>
    for MyStructGenericsPtr<T, U> {
        fn as_ref(&self) -> &MyStructGenerics<T, U> {
            &self.inner
        }
    }
    impl<T: Clone, U: Default> ::core::clone::Clone for MyStructGenericsPtr<T, U> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<
        T: Clone,
        U: Default,
        IntoMyStructGenerics: Into<MyStructGenerics<T, U>>,
    > From<IntoMyStructGenerics> for MyStructGenericsPtr<T, U> {
        fn from(inner: IntoMyStructGenerics) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    struct MyStructGenerics<T: Clone, U: Default = usize> {
        t: T,
        u: U,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_const() {
    let sample = quote! {
        struct MyStruct<T: Clone, const N: usize, const D: usize = 0> {
            t: T,
        }
    };
    let expected = r#"
mod envelope {
    struct MyStructPtr<T: Clone, const N: usize, const D: usize = 0> {
        inner: ::std::sync::Arc<MyStruct<T, N, D>>,
    }
    impl<T: Clone, const N: usize, const D: usize> ::std::ops::Deref
    for MyStructPtr<T, N, D> {
        type Target = MyStruct<T, N, D>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<
        T: Clone,
        const N: usize,
        const D: usize,
    > ::core::convert::AsRef<MyStruct<T, N, D>> for MyStructPtr<T, N, D> {
        fn as_ref(&self) -> &MyStruct<T, N, D> {
            &self.inner
        }
    }
    impl<T: Clone, const N: usize, const D: usize> ::core::clone::Clone
    for MyStructPtr<T, N, D> {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<
        T: Clone,
        const N: usize,
        const D: usize,
        IntoMyStruct: Into<MyStruct<T, N, D>>,
    > From<IntoMyStruct> for MyStructPtr<T, N, D> {
        fn from(inner: IntoMyStruct) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    struct MyStruct<T: Clone, const N: usize, const D: usize = 0> {
        t: T,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_struct_where() {
    let sample = quote! {
        struct MyStruct<'t, 'tt: 't, T: 't>
        where
            T: Clone,
        {
            t: &'t T,
            tt: &'tt T,
        }
    };
    let expected = r#"

mod envelope {
    struct MyStructPtr<'t, 'tt: 't, T: 't>
    where
        T: Clone,
    {
        inner: ::std::sync::Arc<MyStruct<'t, 'tt, T>>,
    }
    impl<'t, 'tt: 't, T: 't> ::std::ops::Deref for MyStructPtr<'t, 'tt, T>
    where
        T: Clone,
    {
        type Target = MyStruct<'t, 'tt, T>;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl<'t, 'tt: 't, T: 't> ::core::convert::AsRef<MyStruct<'t, 'tt, T>>
    for MyStructPtr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn as_ref(&self) -> &MyStruct<'t, 'tt, T> {
            &self.inner
        }
    }
    impl<'t, 'tt: 't, T: 't> ::core::clone::Clone for MyStructPtr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<'t, 'tt: 't, T: 't, IntoMyStruct: Into<MyStruct<'t, 'tt, T>>> From<IntoMyStruct>
    for MyStructPtr<'t, 'tt, T>
    where
        T: Clone,
    {
        fn from(inner: IntoMyStruct) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    struct MyStruct<'t, 'tt: 't, T: 't>
    where
        T: Clone,
    {
        t: &'t T,
        tt: &'tt T,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_enum() {
    let sample = quote! {
        enum MyEnum {
            A(String),
            B
        }
    };
    let expected = r#"

mod envelope {
    struct MyEnumPtr {
        inner: ::std::sync::Arc<MyEnum>,
    }
    impl ::std::ops::Deref for MyEnumPtr {
        type Target = MyEnum;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }
    impl ::core::convert::AsRef<MyEnum> for MyEnumPtr {
        fn as_ref(&self) -> &MyEnum {
            &self.inner
        }
    }
    impl ::core::clone::Clone for MyEnumPtr {
        fn clone(&self) -> Self {
            Self { inner: self.inner.clone() }
        }
    }
    impl<IntoMyEnum: Into<MyEnum>> From<IntoMyEnum> for MyEnumPtr {
        fn from(inner: IntoMyEnum) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    enum MyEnum {
        A(String),
        B,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_visibility() {
    let sample = quote! {
        pub(super) struct MyStruct {
            pub a: String,
            pub(super) b: i32,
        }
    };
    let expected = r#"

mod envelope {
    pub(super) struct MyStructPtr {
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
    impl<IntoMyStruct: Into<MyStruct>> From<IntoMyStruct> for MyStructPtr {
        fn from(inner: IntoMyStruct) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    struct MyStruct {
        pub a: String,
        pub(super) b: i32,
    }
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_derives() {
    let sample = quote! {
        #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
        struct MyStruct(String);
    };
    let expected = r#"
mod envelope {
    #[derive(Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
    impl<IntoMyStruct: Into<MyStruct>> From<IntoMyStruct> for MyStructPtr {
        fn from(inner: IntoMyStruct) -> Self {
            Self { inner: inner.into().into() }
        }
    }
    #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
    struct MyStruct(String);
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
