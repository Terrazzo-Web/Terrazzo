pub use nameth_macro::nameth;

pub trait NamedType {
    fn type_name() -> &'static str;
}

pub trait NamedEnumValues {
    fn name(&self) -> &'static str;
}
