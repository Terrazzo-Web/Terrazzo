//! Generated HTML elements

use std::sync::Mutex;

use nameth::NamedType as _;
use nameth::nameth;
use scopeguard::defer;
use tracing::debug;
use tracing::debug_span;
use tracing::info;
use tracing::trace;
use tracing::trace_span;
use tracing::warn;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::Element;
use web_sys::js_sys::Function;

use self::template::XTemplate;
use crate::attribute::XAttribute;
use crate::key::XKey;
use crate::node::XNode;
use crate::prelude::OrElseLog as _;
use crate::signal::reactive_closure::reactive_closure_builder::Consumers;
use crate::string::XString;
use crate::template::IsTemplate;
use crate::utils::Prc;

mod debug;
mod merge_attributes;
mod merge_children;
mod merge_events;
pub mod template;

/// Represents a generated HTML element.
///
/// Example: the HTML tag `<input type="text" name="username" value="LamparoS@Pavy.one" />`
/// would be written as
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XElement {
///     tag_name: Some("form".into()),
///     key: XKey::default(),
///     value: XElementValue::Static {
///         attributes: vec![],
///         children: vec![XNode::from(XElement {
///             tag_name: Some("input".into()),
///             key: XKey::default(),
///             value: XElementValue::Static {
///                 attributes: vec![
///                     XAttribute { name: "type".into(), value: "text".into() },
///                     XAttribute { name: "name".into(), value: "username".into() },
///                     XAttribute { name: "value".into(), value: "LamparoS@Pavy.one".into() },
///                 ],
///                 children: vec![],
///                 events: vec![],
///             },
///             before_render: None,
///             after_render: None,
///         })],
///         events: vec![],
///     },
///     before_render: None,
///     after_render: None,
/// }
/// # ;
/// ```
#[nameth]
pub struct XElement {
    /// The key of an element is used to reconcile a generated [XElement] with an existing [Element]
    /// node.
    ///
    /// This allows reusing existing DOM nodes instead of generating entirely new ones when an
    /// [XElement] is recomputed after a [signal] is updated and triggers.
    ///
    /// If the key is not set, a key is generated using the ordinal position of the node,
    /// and nodes are merged on a best-effort basis.
    ///
    /// [signal]: super::signal::XSignal
    pub key: XKey,

    /// The name of the HTML tag. Can be [None] when provided by a `#[template(tag = ...)]` attribute.
    pub tag_name: Option<XString>,

    /// The content of the HTML node.
    pub value: XElementValue,

    /// A callback executed after the [Element] is created but before it is rendered.
    ///
    /// On first render this will always be an empty node.
    pub before_render: Option<OnRenderCallback>,

    /// A callback executed after the [Element] is rendered.
    pub after_render: Option<OnRenderCallback>,
}

/// The content of an HTML node.
pub enum XElementValue {
    /// When the node is some static content.
    Static {
        attributes: Vec<XAttribute>,
        events: Vec<XEvent>,
        children: Vec<XNode>,
    },

    /// When the node must be computed by some reactive closure.
    Dynamic(XDynamicElement),

    /// When the dynamic node is computed and owned by the reactive closure.
    Generated {
        template: XTemplate,
        consumers: Consumers,
    },
}

/// Represents the callback that generates a dynamic [XElement].
pub struct XDynamicElement(Box<dyn Fn(XTemplate) -> Consumers>);

impl<F: Fn(XTemplate) -> Consumers + 'static> From<F> for XDynamicElement {
    fn from(value: F) -> Self {
        Self(Box::new(value))
    }
}

/// Represents an event that will be attached to the generated DOM node.
pub struct XEvent {
    /// The name of the event, e.g. "click" or "mouseover".
    pub event_type: XString,

    /// The callback that takes the [event] as parameter and is executed when the event fires.
    ///
    /// [event]: web_sys::Event
    pub callback: Prc<dyn ClosureAsFunction>,
}

pub trait ClosureAsFunction: std::fmt::Debug {
    fn as_function(&self) -> &Function;
}

impl<T: ?Sized> ClosureAsFunction for Closure<T> {
    fn as_function(&self) -> &Function {
        self.as_ref().unchecked_ref()
    }
}

/// A callback that is executed before/after a node is rendered.
///
/// Exampple:
/// ```ignore
/// div(
///     h1("Some HTML template"),
///     before_render = |_: Element| info!("Before render"),
///     after_render = |_: Element| info!("After render"),
/// )
/// ```
pub struct OnRenderCallback(pub Box<dyn Fn(Element)>);

impl XElement {
    pub fn merge(&mut self, template: &XTemplate, old: &mut Self, element_rc: Prc<Mutex<Element>>) {
        match &self.key {
            XKey::Named(key) => {
                let _span = debug_span!("Merge", %key).entered();
                debug!("Start");
                defer!(debug!("End"));
                self.merge_impl(template, old, element_rc);
            }
            XKey::Index(_) => {
                let _span = trace_span!("Merge", key = ?self.key).entered();
                trace!("Start");
                defer!(trace!("End"));
                self.merge_impl(template, old, element_rc);
            }
        };
    }

    fn merge_impl(
        &mut self,
        template: &XTemplate,
        old: &mut Self,
        element_rc: Prc<Mutex<Element>>,
    ) {
        let element = {
            let mut element = element_rc.lock().or_throw("element");
            if let XKey::Named(new_key) = &self.key {
                if let XKey::Named(cur_key) = XKey::of(template, 0, &element) {
                    if new_key != &cur_key {
                        warn!("Templates conflict on key cur_key:{cur_key} vs new_key:{new_key}");
                        let () = element
                            .set_attribute(template.key_attribute(), new_key)
                            .or_else_throw(|error| format!("Set element key failed: {error:?}'"));
                    }
                }
            }

            self.fix_element_tag(template, &mut element);
            element.clone()
        };

        if let Some(OnRenderCallback(before_render)) = &self.before_render {
            before_render(element.clone());
        }

        match &mut self.value {
            XElementValue::Static {
                attributes: new_attributes,
                events: new_events,
                children: new_children,
            } => match &mut old.value {
                XElementValue::Static {
                    attributes: old_attributes,
                    events: old_events,
                    children: old_children,
                } => {
                    merge_attributes::merge(
                        template.depth(),
                        new_attributes,
                        old_attributes,
                        &element,
                    );
                    merge_events::merge(new_events, old_events, &element);
                    merge_children::merge(template, new_children, old_children, &element);
                }
                XElementValue::Dynamic { .. } | XElementValue::Generated { .. } => {
                    // The reactive callback may still active!
                    old.value = XElementValue::Static {
                        attributes: vec![],
                        events: vec![],
                        children: vec![],
                    };
                    debug!("A node changed from Dynamic/Generated to Static");
                    merge_attributes::merge(template.depth(), new_attributes, &mut [], &element);
                    merge_events::merge(new_events, &[], &element);
                    merge_children::merge(template, new_children, &mut [], &element);
                }
            },
            XElementValue::Dynamic(XDynamicElement(new_reactive_callback)) => {
                let new_template = if let XElementValue::Generated {
                    template: old_template,
                    consumers: old_consumers,
                } = &mut old.value
                {
                    trace!("Reuse exising template");
                    let old_consumers = std::mem::take(old_consumers);
                    drop(old_consumers);
                    old_template.clone()
                } else {
                    trace!("Create a new template");
                    XTemplate::with_depth(template.depth().next(), element_rc.clone())
                };
                let consumers = new_reactive_callback(new_template.clone());
                self.value = XElementValue::Generated {
                    template: new_template,
                    consumers,
                };
            }
            XElementValue::Generated { .. } => {
                warn!("Illegal {} state", XElement::type_name());
                debug_assert!(false);
            }
        }

        if let Some(OnRenderCallback(after_render)) = &self.after_render {
            after_render(element);
        }
    }

    pub fn zero(&self) -> XElement {
        XElement {
            key: self.key.to_owned(),
            tag_name: None,
            value: XElementValue::Static {
                attributes: vec![],
                events: vec![],
                children: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }

    fn fix_element_tag(&self, template: &XTemplate, element: &mut Element) -> Option<()> {
        let Some(new_tag) = self.tag_name.as_deref() else {
            return Some(());
        };
        let old_tag = element.tag_name().to_lowercase();
        if old_tag == new_tag {
            return Some(());
        }

        info!(old_tag, new_tag, "Changing element tag");
        debug!(old_tag, new_tag, "Tag was {}", element.outer_html());
        let document = element.owner_document()?;
        let new_element: Element = document
            .create_element(new_tag)
            .inspect_err(|error| warn!("Create new element '{new_tag}' failed: {error:?}'"))
            .ok()?;

        #[cfg(debug_assertions)]
        let () = new_element
            .set_attribute("trz-old-tag", &old_tag)
            .inspect_err(|error| warn!("Set old-tag attribute failed: {error:?}'"))
            .ok()?;

        if let Some(key) = element.get_attribute(template.key_attribute()) {
            let () = new_element
                .set_attribute(template.key_attribute(), &key)
                .inspect_err(|error| warn!("Set element key failed: {error:?}'"))
                .ok()?;
        }

        // Note: replaceWith() doesn't always work in Chrome when replacing nodes with different tag names.
        let Some(parent) = element.parent_node() else {
            warn!("Node has no parent!");
            return None;
        };
        let insertion = parent.insert_before(&new_element, element.next_sibling().as_ref());
        element.remove();
        insertion
            .inspect_err(|error| warn!("Failed to insert before: {error:?}"))
            .ok()?;
        *element = new_element.clone();
        Some(())
    }
}
