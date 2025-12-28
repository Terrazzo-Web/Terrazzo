use std::cell::LazyCell;
use std::collections::HashMap;

use super::XAttribute;
use super::template::LiveElement;
use crate::attribute::XAttributeName;
use crate::attribute::XAttributeValue;
use crate::prelude::XAttributeKind;
use crate::prelude::diagnostics::trace;
use crate::prelude::diagnostics::warn;
use crate::signal::depth::Depth;

pub fn merge(
    depth: Depth,
    new_attributes: &mut [XAttribute],
    old_attributes: &mut [XAttribute],
    element: &LiveElement,
) {
    trace!(
        new_count = new_attributes.len(),
        old_count = old_attributes.len(),
        "Attributes"
    );

    let mut old_attributes_map = HashMap::new();
    for old_attribute in old_attributes {
        old_attributes_map.insert(
            std::mem::replace(
                &mut old_attribute.name,
                XAttributeName {
                    name: Default::default(),
                    kind: XAttributeKind::Attribute,
                    index: Default::default(),
                    sub_index: Default::default(),
                },
            ),
            std::mem::replace(&mut old_attribute.value, XAttributeValue::Null),
        );
    }

    let css_style = LazyCell::new(|| element.css_style());

    for new_attribute in new_attributes {
        let attribute_name = &new_attribute.name;
        let old_attribute_value = old_attributes_map.remove(attribute_name);
        new_attribute.merge(depth, old_attribute_value, element);
    }

    for removed_old_attribute_name in old_attributes_map.keys() {
        let name = &removed_old_attribute_name.name;
        match removed_old_attribute_name.kind {
            XAttributeKind::Attribute => match element.html.remove_attribute(name) {
                Ok(()) => trace!("Removed attribute {name}"),
                Err(error) => warn!("Removed attribute {name} failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.remove_property(name) {
                Ok(value) => trace!("Removed style {name}: {value}"),
                Err(error) => warn!("Removed style {name} failed: {error:?}"),
            },
        }
    }
}
