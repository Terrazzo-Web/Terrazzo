use super::id::XAttributeId;
use crate::prelude::OrElseLog;
use crate::prelude::diagnostics;
use crate::string::XString;

#[derive(Default)]
pub struct AttributeValuesBuilder {
    values: Vec<Vec<AttributeValueDiff>>,
}

#[derive(Debug, Default)]
pub enum AttributeValueDiff {
    #[default]
    Undefined,
    Same(XString),
    Null,
    Value(XString),
}

impl AttributeValuesBuilder {
    pub fn get_mut(&mut self, id: &XAttributeId) -> &mut AttributeValueDiff {
        diagnostics::error!(
            "AttributeValuesBuilder::get_mut self.values.len()={}",
            self.values.len()
        );
        if self.values.len() == id.index {
            self.values.push(Default::default());
        }
        let values_len = self.values.len();
        let values = self.values.get_mut(id.index).or_else_throw(|()| {
            format!(
                "AttributeValuesBuilder::get_mut #1: index={} vs values_len={values_len}",
                id.index
            )
        });
        if values.len() == id.sub_index {
            values.push(Default::default());
        }
        let values_len = values.len();
        values.get_mut(id.sub_index).or_else_throw(|()| {
            format!(
                "AttributeValuesBuilder::get_mut #2: sub_index={} vs values_len={values_len}",
                id.sub_index
            )
        })
    }

    pub fn get_chunk(&self, index: usize) -> &[AttributeValueDiff] {
        self.values
            .get(index)
            .or_throw("AttributeValuesBuilder::get")
    }
}

pub(super) fn aggregate_attribute(attributes: &[AttributeValueDiff]) -> Option<Option<String>> {
    match ChangeType::resolve(attributes) {
        ChangeType::Same => return None,
        ChangeType::Null => return Some(None),
        ChangeType::Value => (),
    }

    let changed_attributes = attributes
        .into_iter()
        .filter_map(|attribute| match attribute {
            AttributeValueDiff::Undefined | AttributeValueDiff::Null => None,
            AttributeValueDiff::Same(value) | AttributeValueDiff::Value(value) => {
                Some(value.as_str())
            }
        });
    let mut value_acc = String::default();
    for value in changed_attributes {
        if !value_acc.is_empty() {
            value_acc.push(' ');
        }
        value_acc.push_str(value);
    }
    Some(Some(value_acc))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChangeType {
    Same,
    Null,
    Value,
}

impl ChangeType {
    fn resolve(attributes: &[AttributeValueDiff]) -> Self {
        let mut has_data = false;
        let mut has_null = false;
        for attribute in attributes {
            match attribute {
                AttributeValueDiff::Undefined => (),
                AttributeValueDiff::Same { .. } => has_data = true,
                AttributeValueDiff::Null => has_null = true,
                AttributeValueDiff::Value { .. } => return Self::Value,
            }
        }
        return match (has_data, has_null) {
            (_, false) => Self::Same,
            (false, true) => Self::Null,
            (true, true) => Self::Value,
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::attribute::builder::AttributeValueDiff;

    #[test]
    fn aggregate_attribute_none() {
        let values = [AttributeValueDiff::Undefined, AttributeValueDiff::Undefined];
        let result = super::aggregate_attribute(&values);
        assert_eq!(None, result)
    }

    #[test]
    fn aggregate_attribute_same() {
        let values = [
            AttributeValueDiff::Undefined,
            AttributeValueDiff::Same("same".into()),
        ];
        let result = super::aggregate_attribute(&values);
        assert_eq!(None, result)
    }

    #[test]
    fn aggregate_attribute_same_and_value() {
        let values = [
            AttributeValueDiff::Undefined,
            AttributeValueDiff::Same("same".into()),
            AttributeValueDiff::Value("other".into()),
        ];
        let result = super::aggregate_attribute(&values);
        assert_eq!(Some(Some("same other".to_owned())), result)
    }

    #[test]
    fn aggregate_attribute_same_and_null() {
        let values = [
            AttributeValueDiff::Undefined,
            AttributeValueDiff::Same("same".into()),
            AttributeValueDiff::Null,
        ];
        let result = super::aggregate_attribute(&values);
        assert_eq!(Some(Some("same".to_owned())), result)
    }

    #[test]
    fn aggregate_attribute_null() {
        let values = [AttributeValueDiff::Null, AttributeValueDiff::Undefined];
        let result = super::aggregate_attribute(&values);
        assert_eq!(Some(None), result)
    }
}
