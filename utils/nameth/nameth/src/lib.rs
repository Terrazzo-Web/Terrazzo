#![doc = include_str!("../README.md")]

/// Attribute that can be added to structs, enums or function,
/// to generate a static &str representing the name of the item.
///
/// Example:
///
/// **With structs**
/// ```
/// # use nameth::nameth;
/// # use nameth::NamedType as _;
/// #[nameth]
/// struct Point { x: i32, y: i32 }
/// assert_eq!("Point", Point::type_name());
/// ```
///
/// **With enums**
/// ```
/// # use nameth::nameth;
/// # use nameth::NamedType as _;
/// # use nameth::NamedEnumValues as _;
/// #[nameth]
/// enum Shape {
///     Square(f64),
///     Triangle { a: f64, b: f64, c: f64 },
///     Rectangle { x: f64, y: f64 },
///     Circle { radius: f64 },
/// }
/// assert_eq!("Shape", Shape::type_name());
/// assert_eq!("Square", Shape::Square(2.0).name());
/// assert_eq!("Circle", Shape::Circle { radius: 2.0 }.name());
/// ```
///
/// **With functions**
/// ```
/// # use nameth::nameth;
/// # use nameth::NamedType as _;
/// # struct Shape;
///
/// #[nameth]
/// fn draw(shape: Shape) {}
/// assert_eq!("draw", DRAW);
///
/// #[nameth]
/// fn draw_shape(shape: Shape) {}
/// assert_eq!("draw_shape", DRAW_SHAPE);
/// ```
pub use nameth_macro::nameth;

/// Trait implemented by [nameth] macro to get the name of a struct, enum, or function.
pub trait NamedType {
    fn type_name() -> &'static str;
}

/// Trait implemented by [nameth] macro to get the name of an enum value.
pub trait NamedEnumValues {
    fn name(&self) -> &'static str;
}
