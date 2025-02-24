#![cfg(test)]

use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;

#[test]
fn nameth_struct() {
    #[nameth]
    struct ZeroStruct;
    assert_eq!("ZeroStruct", ZeroStruct::type_name());

    #[nameth]
    struct EmptyStruct {}
    assert_eq!("EmptyStruct", EmptyStruct::type_name());

    #[nameth]
    struct TupleStruct(#[expect(unused)] i32, #[expect(unused)] String);
    assert_eq!("TupleStruct", TupleStruct::type_name());

    #[nameth]
    struct GenericStruct<T, U: std::fmt::Display>(T, U);
    assert_eq!("GenericStruct", GenericStruct::<i32, i32>::type_name());
}

#[test]
fn nameth_enum() {
    #[nameth]
    enum TestEnum {
        ZeroVariant,
        TupleVariant(#[expect(unused)] i32, #[expect(unused)] String),
        StructVariant {
            #[expect(unused)]
            a: i32,
            #[expect(unused)]
            b: String,
        },
    }
    assert_eq!("ZeroVariant", TestEnum::ZeroVariant.name());
    assert_eq!("TupleVariant", TestEnum::TupleVariant(0, "".into()).name());
    assert_eq!(
        "StructVariant",
        TestEnum::StructVariant { a: 0, b: "".into() }.name()
    );
    assert_eq!("TestEnum", TestEnum::type_name());

    #[nameth]
    #[expect(unused)]
    enum GenericEnum<T, U: std::fmt::Display> {
        A(T),
        B(U),
    }
    assert_eq!("GenericEnum", GenericEnum::<i32, i32>::type_name());
}

#[test]
#[nameth]
fn nameth_fn() {
    assert_eq!("some_nameth_function", SOME_NAMETH_FUNCTION);
    assert_eq!("nameth_fn", NAMETH_FN);
}

#[expect(unused)]
#[nameth]
pub(crate) fn some_nameth_function() {}
