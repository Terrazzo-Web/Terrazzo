use crate::element::XElement;
use crate::string::XString;

/// Represents a generated node, which is either an HTML element or a text node.
#[derive(Debug)]
pub enum XNode {
    Element(XElement),
    Text(XText),
}

/// Represents a generated text node
#[derive(Debug)]
pub struct XText(pub XString);

impl From<XElement> for XNode {
    fn from(value: XElement) -> Self {
        Self::Element(value)
    }
}

impl From<XText> for XNode {
    fn from(value: XText) -> Self {
        Self::Text(value)
    }
}
