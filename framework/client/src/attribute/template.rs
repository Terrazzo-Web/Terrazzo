use std::cell::LazyCell;

use autoclone::envelope;

use self::diagnostics::trace;
use super::attribute::XAttribute;
use super::attribute::set_attribute;
use super::diff_store::AttributeValueDiffStore;
use super::diff_store::DynamicBackend;
use super::id::XAttributeId;
use super::value::XAttributeValue;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::LiveElement;
use crate::prelude::diagnostics;
use crate::signal::depth::Depth;
use crate::signal::reactive_closure::reactive_closure_builder::Consumers;
use crate::template::IsTemplate;
use crate::template::IsTemplated;
use crate::utils::Ptr;

/// Represents the callback that generates a dynamic [XAttribute].
pub struct XDynamicAttribute(pub Box<dyn Fn(XAttributeTemplate) -> Consumers>);

impl<F: Fn(XAttributeTemplate) -> Consumers + 'static> From<F> for XDynamicAttribute {
    fn from(value: F) -> Self {
        Self(Box::new(value))
    }
}

/// Represents the template that generates a dynamic [XAttribute].
#[envelope(ptr = Ptr)]
pub struct AttributeTemplateInner {
    pub element: LiveElement,
    pub attribute_id: XAttributeId,
    pub(super) debug_id: DebugCorrelationId<String>,
    pub(super) depth: Depth,
}

pub use AttributeTemplateInnerPtr as XAttributeTemplate;

impl XAttributeTemplate {
    pub fn new(
        element: LiveElement,
        attribute_id: XAttributeId,
        debug_id: DebugCorrelationId<String>,
        depth: Depth,
    ) -> Self {
        AttributeTemplateInner {
            element,
            attribute_id,
            debug_id,
            depth,
        }
        .into()
    }
}

impl IsTemplate for XAttributeTemplate {
    type Value = XAttributeValue;

    fn apply<R: Into<Self::Value>>(self, new: impl FnOnce() -> R) {
        let mut new = XAttribute {
            id: self.attribute_id.clone(),
            value: new().into(),
        };
        let mut backend = DynamicBackend::new(&self.element);
        new.merge(self.depth, &self.element, &mut backend, None);

        let value_acc = backend.aggregate_attribute(self.attribute_id.index);
        let value_acc = value_acc.as_ref().map(|v| v.as_ref().map(|v| v.as_ref()));
        trace!("Update attribute template {} to {value_acc:?}", new.id);
        let Some(value_acc) = value_acc else {
            // There was no diff!
            return;
        };

        let css_style = LazyCell::new(|| self.element.css_style());
        set_attribute(&self.element.html, &css_style, &new.id.name, value_acc);
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
