use crate::string::XString;

/// Represents the name of an [XAttribute].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct XAttributeName {
    pub name: XString,
    pub kind: XAttributeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum XAttributeKind {
    /// Represents the name of an HTML attribute.
    Attribute,

    /// Represents the name of a CSS property.
    ///
    /// Example:
    /// `<div style="width:100%'> ... </div>`
    /// would have the following [XAttribute]:
    /// ```
    /// # use terrazzo_client::prelude::*;
    /// # let _ =
    /// XAttribute {
    ///     id: XAttributeId {
    ///         name: XAttributeKind::Style.make("width"),
    ///         index: 0,
    ///         sub_index: 0,
    ///     },
    ///     value: "100%".into(),
    /// }
    /// # ;
    /// ```
    Style,
}

impl XAttributeName {
    pub const fn zero() -> Self {
        Self {
            name: XString::Str(""),
            kind: XAttributeKind::Attribute,
        }
    }
}

impl std::fmt::Display for XAttributeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            XAttributeKind::Attribute => std::fmt::Display::fmt(&self.name, f),
            XAttributeKind::Style => write!(f, "style::{}", self.name),
        }
    }
}

impl XAttributeKind {
    pub fn make<T>(self, name: T) -> XAttributeName
    where
        T: Into<XString>,
    {
        XAttributeName {
            name: name.into(),
            kind: self,
        }
    }
}
