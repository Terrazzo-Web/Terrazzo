use std::sync::Mutex;

use web_sys::Element;

use self::inner::TemplateInner;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::element::XElement;
use crate::element::XElementValue;
use crate::key::KEY_ATTRIBUTE;
use crate::key::XKey;
use crate::node::XNode;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::trace;
use crate::signal::depth::Depth;
use crate::template::IsTemplate;
use crate::template::IsTemplated;
use crate::utils::Ptr;

/// A template represents an [Element] managed by the Terrazzo framework.
///
/// It holds a reference to the old [XElement] which ensures Javacript event callbacks
/// aren't dropped as long as the template is live.
#[derive(Clone)]
pub struct XTemplate(Ptr<TemplateInner>);

mod inner {
    use std::ops::Deref;
    use std::sync::Mutex;

    use web_sys::Element;

    use super::XTemplate;
    use crate::debug_correlation_id::DebugCorrelationId;
    use crate::element::XElement;
    use crate::signal::depth::Depth;
    use crate::utils::Ptr;

    pub struct TemplateInner {
        pub(super) key_attribute: String,
        pub(super) debug_id: DebugCorrelationId<&'static str>,
        pub(super) depth: Depth,
        pub(super) element_mut: Ptr<Mutex<Element>>,
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
    pub fn new(element_mut: Ptr<Mutex<Element>>) -> Self {
        Self::with_depth(Depth::zero(), element_mut)
    }

    pub(crate) fn with_depth(depth: Depth, element_mut: Ptr<Mutex<Element>>) -> Self {
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
        self.element_mut.lock().or_throw("element").clone()
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
