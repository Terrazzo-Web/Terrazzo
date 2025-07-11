use std::collections::HashMap;

use wasm_bindgen::JsCast as _;
use web_sys::Element;
use web_sys::HtmlElement;

use super::XAttribute;
use crate::attribute::XAttributeName;
use crate::attribute::XAttributeValue;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::trace;
use crate::prelude::diagnostics::warn;
use crate::signal::depth::Depth;
use crate::string::XString;

pub fn merge(
    depth: Depth,
    new_attributes: &mut [XAttribute],
    old_attributes: &mut [XAttribute],
    element: &Element,
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
                XAttributeName::Attribute(XString::default()),
            ),
            std::mem::replace(&mut old_attribute.value, XAttributeValue::Null),
        );
    }

    for new_attribute in new_attributes {
        let attribute_name = &new_attribute.name;
        let old_attribute_value = old_attributes_map.remove(attribute_name);
        new_attribute.merge(depth, old_attribute_value, element);
    }

    for removed_old_attribute_name in old_attributes_map.keys() {
        match removed_old_attribute_name {
            XAttributeName::Attribute(name) => match element.remove_attribute(name.as_str()) {
                Ok(()) => trace!("Removed attribute {name}"),
                Err(error) => warn!("Removed attribute {name} failed: {error:?}"),
            },
            XAttributeName::Style(name) => {
                let html_element: &HtmlElement = element.dyn_ref().or_throw("HtmlElement");
                let style = html_element.style();
                match style.remove_property(name) {
                    Ok(value) => trace!("Removed style {name}: {value}"),
                    Err(error) => warn!("Removed style {name} failed: {error:?}"),
                }
            }
        }
    }
}
