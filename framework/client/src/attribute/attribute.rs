use std::cell::LazyCell;

use nameth::NamedType as _;
use nameth::nameth;
use web_sys::CssStyleDeclaration;
use web_sys::Element;

use self::diagnostics::trace;
use self::diagnostics::warn;
use super::builder::AttributeValueDiff;
use super::id::XAttributeId;
use super::name::XAttributeKind;
use super::name::XAttributeName;
use super::template::XAttributeTemplate;
use super::template::XDynamicAttribute;
use super::value::XAttributeValue;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::element::template::LiveElement;
use crate::prelude::diagnostics;
use crate::signal::depth::Depth;
use crate::signal::reactive_closure::reactive_closure_builder::Consumers;
use crate::string::XString;

/// Represents an attribute of an HTML node.
///
/// Example: the HTML tag `<input type="text" name="username" value="LamparoS@Pavy.one" />`
/// would have an attribute
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XAttribute {
///     id: XAttributeId {
///         name: XAttributeKind::Attribute::make("name"),
///         index: 0,
///         sub_index: 0,
///     },
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
///     id: XAttributeId {
///         name: XAttributeKind::Attribute::make("value"),
///         index: 1,
///         sub_index: 0,
///     },
///     value: "LamparoS@Pavy.one".into(),
/// }
/// # ;
/// ```
#[nameth]
pub struct XAttribute {
    /// ID of the attribute
    pub id: XAttributeId,

    /// Value of the attribute
    pub value: XAttributeValue,
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
                merge_static_atttribute(element, &new_attribute.id, None, old_attribute_value);
            }
            XAttributeValue::Static(new_attribute_value) => {
                merge_static_atttribute(
                    element,
                    &new_attribute.id,
                    Some(new_attribute_value),
                    old_attribute_value,
                );
            }
            XAttributeValue::Dynamic(XDynamicAttribute(new_attribute_value)) => {
                new_attribute.value = merge_dynamic_atttribute(
                    depth,
                    element,
                    &new_attribute.id,
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
    attribute_id: &XAttributeId,
    new_value: Option<&XString>,
    old_value: Option<XAttributeValue>,
) {
    if let Some((XAttributeValue::Static(old_attribute_value), new_value)) =
        old_value.as_ref().zip(new_value)
        && new_value == old_attribute_value
    {
        trace!("Attribute '{attribute_id}' is still '{new_value}'");
        *element.attributes.borrow_mut().get_mut(attribute_id) =
            AttributeValueDiff::Same(new_value.to_owned());
        return;
    }
    drop(old_value);

    *element.attributes.borrow_mut().get_mut(attribute_id) = if let Some(new_value) = new_value {
        AttributeValueDiff::Value(new_value.to_owned())
    } else {
        AttributeValueDiff::Null
    };
}

fn merge_dynamic_atttribute(
    depth: Depth,
    element: &LiveElement,
    attribute_id: &XAttributeId,
    new_attribute_value: &dyn Fn(XAttributeTemplate) -> Consumers,
    old_attribute_value: Option<XAttributeValue>,
) -> XAttributeValue {
    *element.attributes.borrow_mut().get_mut(attribute_id) = AttributeValueDiff::Undefined;
    let new_template = if let Some(XAttributeValue::Generated {
        template: old_template,
        consumers: old_consumers,
    }) = old_attribute_value
    {
        trace!("Reuse exising attribute template {attribute_id}");
        drop(old_consumers);
        old_template
    } else {
        trace!("Create a new attribute template {attribute_id}");
        XAttributeTemplate::new(
            element.clone(),
            attribute_id.clone(),
            DebugCorrelationId::new(|| format!("attribute_template:{attribute_id}")),
            depth.next(),
        )
    };
    XAttributeValue::Generated {
        template: new_template.clone(),
        consumers: new_attribute_value(new_template),
    }
}

pub fn set_attribute(
    element: &Element,
    css_style: &LazyCell<CssStyleDeclaration, impl FnOnce() -> CssStyleDeclaration>,
    attribute_name: &XAttributeName,
    value: Option<&str>,
) {
    let XAttributeName { name, kind } = attribute_name;
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
