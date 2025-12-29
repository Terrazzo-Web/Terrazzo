//! Attributes of generated HTML elements

use std::cell::LazyCell;

use nameth::NamedType as _;
use nameth::nameth;
use web_sys::CssStyleDeclaration;
use web_sys::Element;

use self::inner::AttributeTemplateInner;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::element::template::AttributeValueDiff;
use crate::element::template::LiveElement;
use crate::prelude::diagnostics::trace;
use crate::prelude::diagnostics::warn;
use crate::signal::depth::Depth;
use crate::signal::reactive_closure::reactive_closure_builder::Consumers;
use crate::string::XString;
use crate::template::IsTemplate;
use crate::template::IsTemplated;
use crate::utils::Ptr;

/// Represents an attribute of an HTML node.
///
/// Example: the HTML tag `<input type="text" name="username" value="LamparoS@Pavy.one" />`
/// would have an attribute
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XAttribute {
///     name: XAttributeName::attribute("name"),
///     value: "username".into(),
/// }
/// # ;
/// ```
///
/// and an attribute
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XAttribute {
///     name: XAttributeName::attribute("value"),
///     value: "LamparoS@Pavy.one".into(),
/// }
/// # ;
/// ```
#[nameth]
pub struct XAttribute {
    /// Name of the attribute
    pub name: XAttributeName,

    /// Value of the attribute
    pub value: XAttributeValue,
}

/// Represents the name of an [XAttribute].
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct XAttributeName {
    pub name: XString,
    pub kind: XAttributeKind,
    pub index: usize,
    pub sub_index: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum XAttributeKind {
    /// Represents the name of an HTML attribute.
    Attribute,

    /// Represents the name of a CSS property.
    ///
    /// Example:
    /// `<div style="width:100%'> ... </div>`
    /// would have the following [XAttribute]:
    /// ```
    /// # use terrazzo_client::prelude::*;
    /// # let _ =
    /// XAttribute {
    ///     name: XAttributeName::style("width"),
    ///     value: "100%".into(),
    /// }
    /// # ;
    /// ```
    Style,
}

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

/// Represents the callback that generates a dynamic [XAttribute].
pub struct XDynamicAttribute(pub Box<dyn Fn(XAttributeTemplate) -> Consumers>);

impl<F: Fn(XAttributeTemplate) -> Consumers + 'static> From<F> for XDynamicAttribute {
    fn from(value: F) -> Self {
        Self(Box::new(value))
    }
}

/// Represents the template that generates a dynamic [XAttribute].
#[derive(Clone)]
pub struct XAttributeTemplate(Ptr<AttributeTemplateInner>);

mod inner {
    use std::ops::Deref;

    use super::XAttributeName;
    use super::XAttributeTemplate;
    use crate::debug_correlation_id::DebugCorrelationId;
    use crate::element::template::LiveElement;
    use crate::signal::depth::Depth;

    pub struct AttributeTemplateInner {
        pub element: LiveElement,
        pub attribute_name: XAttributeName,
        pub(super) debug_id: DebugCorrelationId<String>,
        pub(super) depth: Depth,
    }

    impl Deref for XAttributeTemplate {
        type Target = AttributeTemplateInner;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}

impl IsTemplate for XAttributeTemplate {
    type Value = XAttributeValue;

    fn apply<R: Into<Self::Value>>(self, new: impl FnOnce() -> R) {
        let mut new = XAttribute {
            name: self.attribute_name.clone(),
            value: new().into(),
        };
        new.merge(self.depth, &self.element, None);

        let value = aggregate_attribute(
            self.element
                .attributes
                .borrow()
                .get_chunk(new.name.index)
                .iter()
                .map(attribute_diff_to_option),
        );

        let css_style = LazyCell::new(|| self.element.css_style());
        set_attribute(
            &self.element.html,
            &css_style,
            &new.name.name,
            new.name.kind,
            value.as_deref(),
        );
    }

    fn depth(&self) -> Depth {
        self.depth
    }

    fn debug_id(&self) -> &DebugCorrelationId<impl std::fmt::Display> {
        &self.debug_id
    }
}

impl IsTemplated for XAttributeValue {
    type Template = XAttributeTemplate;
}

impl<T> From<T> for XAttributeValue
where
    XString: From<T>,
{
    fn from(value: T) -> Self {
        Self::Static(value.into())
    }
}

impl<T> From<Option<T>> for XAttributeValue
where
    XString: From<T>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => Self::Static(value.into()),
            None => Self::Null,
        }
    }
}

impl XAttribute {
    pub fn merge(
        &mut self,
        depth: Depth,
        element: &LiveElement,
        old_attribute_value: Option<XAttributeValue>,
    ) {
        let new_attribute = self;
        match &new_attribute.value {
            XAttributeValue::Null => {
                merge_static_atttribute(element, &new_attribute.name, None, old_attribute_value);
            }
            XAttributeValue::Static(new_attribute_value) => {
                merge_static_atttribute(
                    element,
                    &new_attribute.name,
                    Some(new_attribute_value),
                    old_attribute_value,
                );
            }
            XAttributeValue::Dynamic(XDynamicAttribute(new_attribute_value)) => {
                new_attribute.value = merge_dynamic_atttribute(
                    depth,
                    element,
                    &new_attribute.name,
                    new_attribute_value,
                    old_attribute_value,
                );
            }
            XAttributeValue::Generated { .. } => {
                warn!("Illegal {} state", XAttribute::type_name());
                unreachable!()
            }
        }
    }
}

fn merge_static_atttribute(
    element: &LiveElement,
    attribute_name: &XAttributeName,
    new_value: Option<&XString>,
    old_value: Option<XAttributeValue>,
) {
    if let Some((XAttributeValue::Static(old_attribute_value), new_value)) =
        old_value.as_ref().zip(new_value)
        && new_value == old_attribute_value
    {
        trace!("Attribute '{attribute_name}' is still '{new_value}'");
        *element.attributes.borrow_mut().get_mut(attribute_name) = AttributeValueDiff::Same;
        return;
    }
    drop(old_value);

    *element.attributes.borrow_mut().get_mut(attribute_name) = if let Some(new_value) = new_value {
        AttributeValueDiff::Value(new_value.to_owned())
    } else {
        AttributeValueDiff::Null
    };
}

fn merge_dynamic_atttribute(
    depth: Depth,
    element: &LiveElement,
    attribute_name: &XAttributeName,
    new_attribute_value: &dyn Fn(XAttributeTemplate) -> Consumers,
    old_attribute_value: Option<XAttributeValue>,
) -> XAttributeValue {
    *element.attributes.borrow_mut().get_mut(attribute_name) = AttributeValueDiff::Same;
    let new_template = if let Some(XAttributeValue::Generated {
        template: old_template,
        consumers: old_consumers,
    }) = old_attribute_value
    {
        trace!("Reuse exising attribute template {attribute_name}");
        drop(old_consumers);
        old_template
    } else {
        trace!("Create a new attribute template {attribute_name}");
        XAttributeTemplate(Ptr::new(AttributeTemplateInner {
            element: element.clone(),
            attribute_name: attribute_name.clone(),
            debug_id: DebugCorrelationId::new(|| format!("attribute_template:{attribute_name}")),
            depth: depth.next(),
        }))
    };
    XAttributeValue::Generated {
        template: new_template.clone(),
        consumers: new_attribute_value(new_template),
    }
}

pub fn aggregate_attribute<'t>(
    attributes: impl Iterator<Item = Option<&'t XString>>,
) -> Option<String> {
    let mut value_acc: Option<String> = None;
    for value in attributes.flatten() {
        match &mut value_acc {
            Some(value_acc) => {
                *value_acc += " ";
                *value_acc += value.as_str();
            }
            None => value_acc = Some(value.to_string()),
        }
    }
    value_acc
}

pub fn attribute_diff_to_option(new_attribute: &AttributeValueDiff) -> Option<&XString> {
    match new_attribute {
        AttributeValueDiff::Same => None,
        AttributeValueDiff::Null => None,
        AttributeValueDiff::Value(value) => Some(value),
    }
}

pub fn attribute_value_to_option(new_attribute: &XAttribute) -> Option<&XString> {
    match &new_attribute.value {
        XAttributeValue::Null => None,
        XAttributeValue::Static(value) => Some(value),
        XAttributeValue::Dynamic { .. } | XAttributeValue::Generated { .. } => {
            unreachable!()
        }
    }
}

pub fn set_attribute(
    element: &Element,
    css_style: &LazyCell<CssStyleDeclaration, impl FnOnce() -> CssStyleDeclaration>,
    name: &str,
    kind: XAttributeKind,
    value: Option<&str>,
) {
    if let Some(value) = value {
        match kind {
            XAttributeKind::Attribute => match element.set_attribute(name, value) {
                Ok(()) => trace!("Set attribute '{name}' to '{value}'"),
                Err(error) => warn!("Set attribute '{name}' to '{value}' failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.set_property(name, value) {
                Ok(()) => trace!("Set style {name}: {value}"),
                Err(error) => warn!("Set style {name}: {value} failed: {error:?}"),
            },
        }
    } else {
        match kind {
            XAttributeKind::Attribute => match element.remove_attribute(name) {
                Ok(()) => trace!("Removed attribute {name}"),
                Err(error) => warn!("Removed attribute {name} failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.remove_property(name) {
                Ok(old_value) => trace!("Removed style {name}: {old_value}"),
                Err(error) => warn!("Removed style {name} failed: {error:?}"),
            },
        }
        return;
    }
}

impl XAttributeName {
    pub fn zero() -> Self {
        Self {
            name: Default::default(),
            kind: XAttributeKind::Attribute,
            index: usize::MAX,
            sub_index: usize::MAX,
        }
    }

    pub fn attribute<T>(name: T) -> Self
    where
        T: Into<XString>,
    {
        Self {
            name: name.into(),
            kind: XAttributeKind::Attribute,
            index: 0,
            sub_index: 0,
        }
    }

    pub fn style<T>(name: T) -> Self
    where
        T: Into<XString>,
    {
        Self {
            name: name.into(),
            kind: XAttributeKind::Style,
            index: 0,
            sub_index: 0,
        }
    }
}

impl std::fmt::Display for XAttributeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            XAttributeKind::Attribute => std::fmt::Display::fmt(&self.name, f),
            XAttributeKind::Style => write!(f, "style::{}", self.name),
        }
    }
}
