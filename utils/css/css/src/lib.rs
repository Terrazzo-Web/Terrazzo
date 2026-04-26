#[macro_export]
macro_rules! import_style {
    ($(#[$meta:meta])* $vis:vis $ident:ident, $str:expr) => {
        $(#[$meta])* $vis mod $ident {
            ::terrazzo_css::internal::import_style!($str);
        }
    };
}

#[doc(hidden)]
pub mod internal {
    pub use ::terrazzo_css_macro::import_style;
}
