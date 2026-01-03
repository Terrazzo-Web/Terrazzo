use super::name::XAttributeName;

/// Represents the unique ID of an [XAttribute](super::XAttribute).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct XAttributeId {
    pub name: XAttributeName,
    pub index: usize,
    pub sub_index: usize,
}

impl std::fmt::Display for XAttributeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let XAttributeId {
            name,
            index,
            sub_index,
        } = self;
        write!(f, "{name} {index}:{sub_index}")
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::XAttributeId;
    use crate::prelude::XAttributeKind;

    #[test]
    fn to_string() {
        assert_eq!(
            "class 0:0",
            XAttributeId {
                name: XAttributeKind::Attribute.make("class"),
                index: 0,
                sub_index: 0
            }
            .to_string()
        );
        assert_eq!(
            "style::width 0:1",
            XAttributeId {
                name: XAttributeKind::Style.make("width"),
                index: 0,
                sub_index: 1
            }
            .to_string()
        );
    }
}
