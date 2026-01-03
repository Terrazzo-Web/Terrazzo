#![cfg(test)]

use autoclone::envelope;

#[test]
fn envelope_struct() {
    #[envelope]
    #[derive(Debug)]
    struct MyStruct {
        a: String,
        b: i32,
    }

    let env = MyStructPtr::from(MyStruct {
        a: "hello".into(),
        b: 42,
    });
    assert_eq!("hello", env.a);
    assert_eq!(42, env.b);
    assert_eq!(
        "MyStructPtr { inner: MyStruct { a: \"hello\", b: 42 } }",
        format!("{env:?}")
    )
}

#[test]
fn envelope_struct_generics() {
    #[envelope]
    #[derive(Debug)]
    struct MyStructGenerics<T: Clone, U: Default = usize> {
        t: T,
        u: U,
    }
    let env = MyStructGenericsPtr::from(MyStructGenerics { t: "hello", u: 42 });
    assert_eq!("hello", env.t);
    assert_eq!(42, env.u);
    assert_eq!(
        "MyStructGenericsPtr { inner: MyStructGenerics { t: \"hello\", u: 42 } }",
        format!("{env:?}")
    )
}

#[test]
fn envelope_visibility() {
    #[expect(unused)]
    mod visibilities {
        pub mod child {
            use autoclone::envelope;
            #[envelope]
            pub struct MyStruct4 {
                pub a: String,
                pub(super) b: i32,
            }

            fn make4() -> MyStruct4Ptr {
                MyStruct4Ptr::from(MyStruct4 {
                    a: "hello".into(),
                    b: 42,
                })
            }
        }
        fn make4(v: child::MyStruct4Ptr) {
            assert_eq!("hello", v.a);
            assert_eq!(42, v.b);
        }
    }

    #[expect(unused)]
    fn make4(v: visibilities::child::MyStruct4Ptr) {
        assert_eq!("hello", v.a);
        // b is private: assert_eq!(42, v.b);
    }
}

#[test]
fn envelope_other() {
    #[expect(unused)]
    mod it_compiles {
        use autoclone::envelope;

        #[envelope]
        struct MyStruct1<T: Clone, const N: usize, const D: usize = 0> {
            t: T,
        }

        #[envelope]
        struct MyStruct2<'t, 'tt: 't, T: 't>
        where
            T: Clone,
        {
            t: &'t T,
            tt: &'tt T,
        }

        #[envelope]
        enum MyEnum3 {
            A(String),
            B,
        }

        #[envelope]
        pub(super) struct MyStruct4 {
            pub a: String,
            pub(super) b: i32,
        }

        #[envelope]
        #[derive(Copy, Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
        struct MyStruct5<'t>(&'t str);
    }
}
