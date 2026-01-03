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
    mod my_struct {
        use super::*;
        pub struct MyStruct {
            pub(super) a: String,
            pub(super) b: i32,
        }
    }
    use my_struct::MyStruct;
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
    mod my_struct_generics {
        use super::*;
        pub struct MyStructGenerics<T: Clone, U: Default = usize> {
            pub(super) t: T,
            pub(super) u: U,
        }
    }
    use my_struct_generics::MyStructGenerics;
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
    mod my_struct {
        use super::*;
        pub struct MyStruct<T: Clone, const N: usize, const D: usize = 0> {
            pub(super) t: T,
        }
    }
    use my_struct::MyStruct;
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
    mod my_struct {
        use super::*;
        pub struct MyStruct<'t, 'tt: 't, T: 't>
        where
            T: Clone,
        {
            pub(super) t: &'t T,
            pub(super) tt: &'tt T,
        }
    }
    use my_struct::MyStruct;
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
    mod my_enum {
        use super::*;
        pub enum MyEnum {
            A(String),
            B,
        }
    }
    use my_enum::MyEnum;
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
    mod my_struct {
        use super::*;
        pub struct MyStruct {
            pub a: String,
            pub(in super::super) b: i32,
        }
    }
    use my_struct::MyStruct;
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
    mod my_struct {
        use super::*;
        #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
        pub struct MyStruct(pub(super) String);
    }
    use my_struct::MyStruct;
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
}
"#;
    run_test(sample, expected);
}


#[test]
fn envelope_custom_attributes() {
    let sample = quote! {
        #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
        #[other_custom_attributes_1]
        #[other_custom_attributes_2]
        struct MyStruct(String);
    };
    let expected = r#"
mod envelope {
    mod my_struct {
        use super::*;
        #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
        pub struct MyStruct(pub(super) String);
    }
    use my_struct::MyStruct;
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
}
"#;
    run_test(sample, expected);
}

#[test]
fn envelope_rc() {
    let args = quote! { ptr = std::rc::Rc };
    let sample = quote! {
        struct MyStruct {
            a: String,
            b: i32,
        }
    };
    let expected = r#"
mod envelope {
    mod my_struct {
        use super::*;
        pub struct MyStruct {
            pub(super) a: String,
            pub(super) b: i32,
        }
    }
    use my_struct::MyStruct;
    struct MyStructPtr {
        inner: std::rc::Rc<MyStruct>,
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
}
"#;
    run_test_args(args, sample, expected);
}

#[track_caller]
fn run_test(sample: proc_macro2::TokenStream, expected: &str) {
    run_test_args(quote!(), sample, expected)
}

#[track_caller]
fn run_test_args(args: proc_macro2::TokenStream, sample: proc_macro2::TokenStream, expected: &str) {
    let actual = super::envelope2(args, sample);
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
