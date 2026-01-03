#![cfg(test)]

use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;

#[test]
fn nameth_struct() {
    #[nameth]
    struct ZeroStruct;
    assert_eq!("ZeroStruct", ZeroStruct::type_name());
    assert_eq!("ZeroStruct", ZERO_STRUCT);

    #[nameth]
    struct EmptyStruct {}
    assert_eq!("EmptyStruct", EmptyStruct::type_name());
    assert_eq!("EmptyStruct", EMPTY_STRUCT);

    #[nameth]
    struct TupleStruct(#[expect(unused)] i32, #[expect(unused)] String);
    assert_eq!("TupleStruct", TupleStruct::type_name());
    assert_eq!("TupleStruct", TUPLE_STRUCT);

    #[nameth]
    struct GenericStruct<T, U: std::fmt::Display>(T, U);
    assert_eq!("GenericStruct", GenericStruct::<i32, i32>::type_name());
    assert_eq!("GenericStruct", GENERIC_STRUCT);

    #[nameth]
    struct GenericStructWithDefaults<T = String, U: std::fmt::Display = &'static str>(T, U);
    assert_eq!(
        "GenericStructWithDefaults",
        GenericStructWithDefaults::<i32, i32>::type_name()
    );
    assert_eq!("GenericStructWithDefaults", GENERIC_STRUCT_WITH_DEFAULTS);
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
    assert_eq!("TestEnum", TEST_ENUM);

    #[nameth]
    #[expect(unused)]
    enum GenericEnum<T, U: std::fmt::Display> {
        A(T),
        B(U),
    }
    assert_eq!("GenericEnum", GenericEnum::<i32, i32>::type_name());

    #[nameth]
    #[expect(unused)]
    enum GenericEnumWithDefaults<T = usize, U: std::fmt::Display = String> {
        A(T),
        B(U),
    }
    assert_eq!(
        "GenericEnumWithDefaults",
        GenericEnumWithDefaults::<i32, i32>::type_name()
    );
    assert_eq!("GenericEnumWithDefaults", GENERIC_ENUM_WITH_DEFAULTS);
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
