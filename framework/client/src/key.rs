//! Keys for generated HTML elements

use web_sys::Element;

use crate::element::template::XTemplate;
use crate::string::XString;

/// The key of an Element node.
///
/// See [XElement::key]
///
/// [XElement::key]: crate::prelude::XElement::key
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum XKey {
    Named(i32, XString),
    Index(i32, usize),
}

/// The name of the custom attribute used to store the [Xkey] of a generated DOM node.
pub const KEY_ATTRIBUTE: &str = "data-trz-key";

impl XKey {
    pub fn of(template: &XTemplate, index: usize, element: &Element) -> Self {
        if let Some(key) = element.get_attribute(template.key_attribute()) {
            parse_index_key(&key).unwrap_or_else(|| XKey::Named(template.tid, key.into()))
        } else {
            XKey::Index(template.tid, index)
        }
    }
}

fn parse_index_key(key: &str) -> Option<XKey> {
    if !key.starts_with('#') {
        return None;
    }
    let mut split = key[1..].split('-');
    let index = split.next()?.parse().ok()?;
    let tid = split.next()?.parse().ok()?;
    return Some(XKey::Index(tid, index));
}

impl std::fmt::Debug for XKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Named(tid, name) => write!(f, "'{name}'-{tid}"),
            Self::Index(tid, index) => write!(f, "#{index}-{tid}"),
        }
    }
}

impl Default for XKey {
    fn default() -> Self {
        Self::Index(0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::key::XKey;

    #[test]
    fn debug() {
        assert_eq!("'key'-1", format!("{:?}", XKey::Named(1, "key".into())));
        assert_eq!("#123-2", format!("{:?}", XKey::Index(2, 123)));
    }
}
