use std::cell::LazyCell;

use web_sys::CssStyleDeclaration;
use web_sys::Element;

use self::diagnostics::debug;
use self::diagnostics::trace;
use self::diagnostics::trace_span;
use self::diagnostics::warn;
use super::XATTRIBUTE;
use super::XAttribute;
use super::builder::AttributeValueDiff;
use super::diff_store::AttributeValueDiffStore;
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

impl XAttribute {
    pub fn merge(
        &mut self,
        depth: Depth,
        element: &LiveElement,
        backend: &mut impl AttributeValueDiffStore,
        old_attribute_value: Option<XAttributeValue>,
    ) {
        let _span = trace_span!("MergeAttr", attribute = %self.id.name.name).entered();
        let new_attribute = self;
        match &new_attribute.value {
            XAttributeValue::Null => {
                merge_static_atttribute(backend, &new_attribute.id, None, old_attribute_value);
            }
            XAttributeValue::Static(new_attribute_value) => {
                merge_static_atttribute(
                    backend,
                    &new_attribute.id,
                    Some(new_attribute_value),
                    old_attribute_value,
                );
            }
            XAttributeValue::Dynamic(XDynamicAttribute(new_attribute_value)) => {
                new_attribute.value = merge_dynamic_atttribute(
                    depth,
                    element,
                    backend,
                    &new_attribute.id,
                    new_attribute_value,
                    old_attribute_value,
                );
            }
            XAttributeValue::Generated { .. } => {
                warn!("Illegal {XATTRIBUTE} state");
                unreachable!()
            }
        }
    }
}

fn merge_static_atttribute(
    backend: &mut impl AttributeValueDiffStore,
    attribute_id: &XAttributeId,
    new_value: Option<&XString>,
    old_value: Option<XAttributeValue>,
) {
    if let Some((XAttributeValue::Static(old_attribute_value), new_value)) =
        old_value.as_ref().zip(new_value)
        && new_value == old_attribute_value
    {
        trace!("Attribute '{attribute_id}' is still '{new_value}'");
        backend.set(attribute_id, AttributeValueDiff::Same(new_value.to_owned()));
    } else {
        drop(old_value);
        trace!("Attribute '{attribute_id}' is set to '{new_value:?}'");
        backend.set(
            attribute_id,
            if let Some(new_value) = new_value {
                AttributeValueDiff::Value(new_value.to_owned())
            } else {
                AttributeValueDiff::Null
            },
        );
    }
}

fn merge_dynamic_atttribute(
    depth: Depth,
    element: &LiveElement,
    backend: &mut impl AttributeValueDiffStore,
    attribute_id: &XAttributeId,
    new_attribute_value: &dyn Fn(XAttributeTemplate) -> Consumers,
    old_attribute_value: Option<XAttributeValue>,
) -> XAttributeValue {
    trace!("Dynamic Attribute '{attribute_id}' is initialized");
    backend.set(attribute_id, AttributeValueDiff::Undefined);
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
                Ok(()) => debug!("Set attribute '{name}' to '{value}'"),
                Err(error) => warn!("Set attribute '{name}' to '{value}' failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.set_property(name, value) {
                Ok(()) => debug!("Set style {name}: {value}"),
                Err(error) => warn!("Set style {name}: {value} failed: {error:?}"),
            },
        }
    } else {
        match kind {
            XAttributeKind::Attribute => match element.remove_attribute(name) {
                Ok(()) => debug!("Removed attribute {name}"),
                Err(error) => warn!("Removed attribute {name} failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.remove_property(name) {
                Ok(old_value) => debug!("Removed style {name}: {old_value}"),
                Err(error) => warn!("Removed style {name} failed: {error:?}"),
            },
        }
        return;
    }
}
