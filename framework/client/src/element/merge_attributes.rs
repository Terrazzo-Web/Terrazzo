use std::cell::LazyCell;
use std::collections::HashMap;

use super::XAttribute;
use super::template::LiveElement;
use crate::attribute::XAttributeName;
use crate::attribute::XAttributeValue;
use crate::element::template::AttributeValueDiff;
use crate::prelude::XAttributeKind;
use crate::prelude::diagnostics::trace;
use crate::prelude::diagnostics::warn;
use crate::signal::depth::Depth;
use crate::string::XString;

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

    *element.attributes.borrow_mut() = Default::default();
    for new_attribute in new_attributes.iter_mut() {
        let attribute_name = &new_attribute.name;
        let old_attribute_value = old_attributes_map.remove(attribute_name);
        new_attribute.merge(depth, old_attribute_value, element);
    }

    for chunk in chunks(new_attributes) {
        let mut value_acc: Option<String> = None;
        for attribute in chunk.attributes {
            match builder.get(&attribute.name) {
                AttributeValueDiff::Same => unreachable!(),
                AttributeValueDiff::Null => {}
                AttributeValueDiff::Value(value) => match &mut value_acc {
                    Some(value_acc) => {
                        *value_acc += " ";
                        *value_acc += value.as_str();
                    }
                    None => value_acc = Some(value.to_string()),
                },
            }
        }
    }

    let css_style = LazyCell::new(|| element.css_style());
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

fn chunks(attributes: &[XAttribute]) -> impl Iterator<Item = Chunk<'_>> {
    return attributes
        .chunk_by(|x, y| x.name.kind == y.name.kind && x.name.name == y.name.name)
        .map(|chunk| {
            let first = &chunk[0];
            Chunk {
                chunk_kind: ChunkKind::of(chunk),
                name: &first.name.name,
                kind: first.name.kind,
                attributes: chunk,
            }
        });
}

struct Chunk<'t> {
    chunk_kind: ChunkKind,
    name: &'t XString,
    kind: XAttributeKind,
    attributes: &'t [XAttribute],
}

enum ChunkKind {
    Dynamic,
    Static,
    Single,
}
impl ChunkKind {
    fn of(chunk: &[XAttribute]) -> ChunkKind {
        for attribute in chunk {
            if let XAttributeValue::Dynamic { .. } = &attribute.value {
                return ChunkKind::Dynamic;
            }
        }
        if chunk.len() == 1 {
            ChunkKind::Single
        } else {
            ChunkKind::Static
        }
    }
}
