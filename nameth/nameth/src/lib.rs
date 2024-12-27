/// Attribute that can be added to structs, enums or function,
/// to generate a static &str representing the name of the item.
///
/// Example:
///
/// **With structs**
/// ```
/// # use nameth::nameth;
/// # use nameth::NamedType;
/// #[nameth]
/// struct Point { x: i32, y: i32 }
/// assert_eq!("Point", Point::type_name());
/// ```
///
/// **With enums**
/// ```
/// # use nameth::nameth;
/// # use nameth::NamedType;
/// # use nameth::NamedEnumValues;
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
/// # use nameth::NamedType;
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

pub trait NamedType {
    fn type_name() -> &'static str;
}

pub trait NamedEnumValues {
    fn name(&self) -> &'static str;
}