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
    Named(XString),
    Index(usize),
}

/// The name of the custom attribute used to store the [Xkey] of a generated DOM node.
pub const KEY_ATTRIBUTE: &str = "data-trz-key";

impl XKey {
    pub fn of(template: &XTemplate, index: usize, element: &Element) -> Self {
        if let Some(key) = element.get_attribute(template.key_attribute()) {
            parse_index_key(&key).unwrap_or_else(|| XKey::Named(key.into()))
        } else {
            XKey::Index(index)
        }
    }
}

fn parse_index_key(key: &str) -> Option<XKey> {
    if !key.starts_with('#') {
        return None;
    }
    let index: Result<usize, _> = key[1..].parse();
    return Some(XKey::Index(index.ok()?));
}

impl std::fmt::Debug for XKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Named(name) => write!(f, "'{name}'"),
            Self::Index(index) => write!(f, "#{index}"),
        }
    }
}

impl Default for XKey {
    fn default() -> Self {
        Self::Index(0)
    }
}

#[cfg(test)]
mod tests {
    use crate::key::XKey;

    #[test]
    fn debug() {
        assert_eq!("'key'", format!("{:?}", XKey::Named("key".into())));
        assert_eq!("#123", format!("{:?}", XKey::Index(123)));
    }
}
