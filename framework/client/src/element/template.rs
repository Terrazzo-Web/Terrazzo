use std::cell::RefCell;
use std::sync::Mutex;

use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use web_sys::CssStyleDeclaration;
use web_sys::Element;
use web_sys::HtmlElement;

use self::inner::TemplateInner;
use crate::attribute::XAttribute;
use crate::attribute::XAttributeName;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::element::XElement;
use crate::element::XElementValue;
use crate::key::KEY_ATTRIBUTE;
use crate::key::XKey;
use crate::node::XNode;
use crate::prelude::diagnostics::trace;
use crate::signal::depth::Depth;
use crate::string::XString;
use crate::template::IsTemplate;
use crate::template::IsTemplated;
use crate::utils::Ptr;
use crate::utils::or_else_log::OrElseLog as _;

/// A template represents an [Element] managed by the Terrazzo framework.
///
/// It holds a reference to the old [XElement] which ensures Javacript event callbacks
/// aren't dropped as long as the template is live.
#[derive(Clone)]
pub struct XTemplate(Ptr<TemplateInner>);

mod inner {
    use std::ops::Deref;
    use std::sync::Mutex;

    use super::LiveElement;
    use super::XTemplate;
    use crate::debug_correlation_id::DebugCorrelationId;
    use crate::element::XElement;
    use crate::signal::depth::Depth;
    use crate::utils::Ptr;

    pub struct TemplateInner {
        pub(super) key_attribute: String,
        pub(super) debug_id: DebugCorrelationId<&'static str>,
        pub(super) depth: Depth,
        pub(super) element_mut: Ptr<Mutex<LiveElement>>,
        pub(super) old: Mutex<Option<XElement>>,
    }

    impl Deref for XTemplate {
        type Target = TemplateInner;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}

impl XTemplate {
    pub fn new(element_mut: Ptr<Mutex<LiveElement>>) -> Self {
        Self::with_depth(Depth::zero(), element_mut)
    }

    pub(crate) fn with_depth(depth: Depth, element_mut: Ptr<Mutex<LiveElement>>) -> Self {
        use std::sync::atomic::AtomicI32;
        use std::sync::atomic::Ordering::SeqCst;
        static NEXT: AtomicI32 = AtomicI32::new(0);
        Self(Ptr::new(TemplateInner {
            key_attribute: format!("{KEY_ATTRIBUTE}-{:#x}", NEXT.fetch_add(1, SeqCst)),
            debug_id: DebugCorrelationId::new(|| "template"),
            depth,
            element_mut,
            old: Mutex::default(),
        }))
    }

    pub fn element(&self) -> Element {
        self.element_mut.lock().or_throw("element").html.clone()
    }

    #[cfg(not(feature = "concise-traces"))]
    pub(crate) fn with_old(&self, f: impl FnOnce(&Option<XElement>)) {
        f(&self.old.lock().or_throw("old"))
    }

    pub(crate) fn key_attribute(&self) -> &str {
        &self.key_attribute
    }
}

impl IsTemplate for XTemplate {
    type Value = XElement;

    fn apply<R: Into<Self::Value>>(self, new: impl FnOnce() -> R) {
        let mut new = new().into();
        reindex_nodes(&mut new);
        {
            let mut old = self.old.lock().unwrap();
            if let Some(old) = &mut *old {
                new.merge(&self, old, self.element_mut.clone())
            } else {
                let mut old = new.zero();
                new.merge(&self, &mut old, self.element_mut.clone())
            };
            *old = Some(new);
        }
        trace! { "The template is updated to {:?}", self.old.lock().unwrap() };
    }

    fn depth(&self) -> Depth {
        self.depth
    }

    fn debug_id(&self) -> &DebugCorrelationId<impl std::fmt::Display> {
        &self.debug_id
    }
}

impl IsTemplated for XElement {
    type Template = XTemplate;
}

fn reindex_nodes(new: &mut XElement) {
    let XElementValue::Static { children, .. } = &mut new.value else {
        return;
    };
    let mut next = 0;
    for child in children {
        let XNode::Element(child) = child else {
            continue;
        };
        reindex_nodes(child);
        if let XKey::Index(index) = &mut child.key {
            *index = next;
        }
        next += 1;
    }
}

#[derive(Clone)]
pub struct LiveElement {
    pub html: Element,
    pub attributes: Ptr<RefCell<AttributeValuesBuilder>>,
}

impl LiveElement {
    pub fn new(html: Element) -> Self {
        Self {
            html,
            attributes: Default::default(),
        }
    }

    pub fn css_style(&self) -> CssStyleDeclaration {
        let html_element: &HtmlElement = self.html.dyn_ref().or_throw("HtmlElement");
        return html_element.style();
    }

    pub fn set_key_attribute(&self, template: &XTemplate, value: &str) -> Result<(), JsValue> {
        self.html.set_attribute(template.key_attribute(), value)
    }

    pub fn get_key_attribute(&self, template: &XTemplate) -> Option<String> {
        self.html.get_attribute(template.key_attribute())
    }
}

#[derive(Default)]
pub struct AttributeValuesBuilder {
    values: Vec<Vec<AttributeValueDiff>>,
}

#[derive(Default)]
pub enum AttributeValueDiff {
    #[default]
    Same,
    Null,
    Value(XString),
}

impl AttributeValuesBuilder {
    pub fn get_mut(&mut self, name: &XAttributeName) -> &mut AttributeValueDiff {
        if self.values.len() == name.index {
            self.values.push(Default::default());
        }
        let values = &mut self.values[name.index];
        if values.len() == name.sub_index {
            values.push(Default::default());
        }
        &mut values[name.sub_index]
    }

    pub fn group<'t>(
        &self,
        attributes: &'t [XAttribute],
    ) -> impl Iterator<Item = &'t [XAttribute]> {
        let mut iterator = attributes.iter();
        let mut start = 0;
        return (1..attributes.len() + 1).filter_map(move |i| {
            if i == attributes.len() {
                return Some(&attributes[start..i]);
            }

            let last = &attributes[start].name;
            let cur = &attributes[i].name;
            if last.kind != cur.kind || last.name != cur.name {
                let result = &attributes[start..i];
                start = i;
                return Some(result);
            }
            return None;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::XAttributeValue;

    #[test]
    fn attributes_group() {
        let attributes = [
            mkattr("1"),
            mkattr("1"),
            mkattr("2"),
            mkattr("2"),
            mkattr("2"),
            mkattr("3"),
        ];
        let groups = AttributeValuesBuilder::default()
            .group(&attributes)
            .map(|group| {
                group
                    .iter()
                    .map(|attr| attr.name.name.as_str())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(vec![vec!["1", "1"], vec!["2", "2", "2"], vec!["3"]], groups);
    }

    fn mkattr(name: &'static str) -> XAttribute {
        XAttribute {
            name: XAttributeName::attribute(name),
            value: XAttributeValue::Null,
        }
    }
}
