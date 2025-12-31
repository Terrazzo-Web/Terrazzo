use super::template::XAttributeTemplate;
use super::template::XDynamicAttribute;
use crate::signal::reactive_closure::reactive_closure_builder::Consumers;
use crate::string::XString;

/// Represents the value of an [XAttribute].
///
/// Usually the `#[template]` macro takes care of generating the code for [XAttributeValue]s.
pub enum XAttributeValue {
    /// When the value is not set, like [Option::None].
    Null,

    /// When the attribute as some value.
    Static(XString),

    /// When the attribute must be computed by some reactive closure.
    Dynamic(XDynamicAttribute),

    /// When the dynamic attribute is computed and owned by the reactive closure.
    Generated {
        template: XAttributeTemplate,
        consumers: Consumers,
    },
}

impl<S> From<S> for XAttributeValue
where
    XString: From<S>,
{
    fn from(value: S) -> Self {
        Self::Static(value.into())
    }
}

impl<S> From<Option<S>> for XAttributeValue
where
    XString: From<S>,
{
    fn from(value: Option<S>) -> Self {
        match value {
            Some(value) => Self::Static(value.into()),
            None => Self::Null,
        }
    }
}
