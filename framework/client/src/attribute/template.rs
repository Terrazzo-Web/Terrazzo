use std::cell::LazyCell;

use self::diagnostics::debug;
use self::inner::AttributeTemplateInner;
use super::attribute::XAttribute;
use super::attribute::set_attribute;
use super::builder::aggregate_attribute;
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
#[derive(Clone)]
pub struct XAttributeTemplate(Ptr<AttributeTemplateInner>);

mod inner {
    use std::ops::Deref;

    use super::XAttributeTemplate;
    use crate::debug_correlation_id::DebugCorrelationId;
    use crate::element::template::LiveElement;
    use crate::prelude::XAttributeId;
    use crate::signal::depth::Depth;

    pub struct AttributeTemplateInner {
        pub element: LiveElement,
        pub attribute_id: XAttributeId,
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

impl XAttributeTemplate {
    pub fn new(
        element: LiveElement,
        attribute_id: XAttributeId,
        debug_id: DebugCorrelationId<String>,
        depth: Depth,
    ) -> Self {
        Self(Ptr::new(AttributeTemplateInner {
            element,
            attribute_id,
            debug_id,
            depth,
        }))
    }
}

impl IsTemplate for XAttributeTemplate {
    type Value = XAttributeValue;

    fn apply<R: Into<Self::Value>>(self, new: impl FnOnce() -> R) {
        let mut new = XAttribute {
            id: self.attribute_id.clone(),
            value: new().into(),
        };
        new.merge(self.depth, &self.element, None);

        let value = aggregate_attribute(self.element.attributes.borrow().get_chunk(new.id.index));
        debug!("Update attribute template {} to {value:?}", new.id);
        let Some(value) = value else {
            // There was no diff!
            return;
        };

        let css_style = LazyCell::new(|| self.element.css_style());
        set_attribute(
            &self.element.html,
            &css_style,
            &new.id.name,
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
