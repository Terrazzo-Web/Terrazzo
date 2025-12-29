use std::cell::LazyCell;
use std::collections::HashMap;

use super::XAttribute;
use super::template::LiveElement;
use crate::attribute::XAttributeName;
use crate::attribute::XAttributeValue;
use crate::attribute::aggregate_attribute;
use crate::attribute::attribute_diff_to_option;
use crate::attribute::attribute_value_to_option;
use crate::attribute::set_attribute;
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
            std::mem::replace(&mut old_attribute.name, XAttributeName::zero()),
            std::mem::replace(&mut old_attribute.value, XAttributeValue::Null),
        );
    }

    *element.attributes.borrow_mut() = Default::default();
    let css_style = LazyCell::new(|| element.css_style());

    for chunk in chunks_mut(new_attributes) {
        let tmp: Option<String>;
        let value_acc = match chunk.chunk_kind {
            ChunkKind::Dynamic => {
                for new_attribute in chunk.attributes {
                    let old_attribute_value = old_attributes_map.remove(&new_attribute.name);
                    new_attribute.merge(depth, element, old_attribute_value);
                }
                tmp = aggregate_attribute(
                    element
                        .attributes
                        .borrow()
                        .get_chunk(chunk.index)
                        .iter()
                        .map(attribute_diff_to_option),
                );
                tmp.as_deref()
            }
            ChunkKind::Static => {
                tmp = aggregate_attribute(chunk.attributes.iter().map(attribute_value_to_option));
                tmp.as_deref()
            }
            ChunkKind::Single => attribute_value_to_option(&chunk.attributes[0]).map(|s| s.into()),
        };

        set_attribute(
            &element.html,
            &css_style,
            &chunk.name,
            chunk.kind,
            value_acc,
        );
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

fn chunks_mut(attributes: &mut [XAttribute]) -> impl Iterator<Item = Chunk<'_>> {
    return attributes
        .chunk_by_mut(|x, y| x.name.kind == y.name.kind && x.name.name == y.name.name)
        .map(|chunk| {
            let first = &chunk[0];
            Chunk {
                chunk_kind: ChunkKind::of(chunk),
                name: first.name.name.clone(),
                kind: first.name.kind,
                index: first.name.index,
                attributes: chunk,
            }
        });
}

struct Chunk<'t> {
    chunk_kind: ChunkKind,
    name: XString,
    kind: XAttributeKind,
    index: usize,
    attributes: &'t mut [XAttribute],
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
