use std::cell::LazyCell;
use std::collections::HashSet;

use scopeguard::guard;

use super::XAttribute;
use super::template::LiveElement;
use crate::attribute::attribute::set_attribute;
use crate::attribute::builder::aggregate_attribute;
use crate::attribute::value::XAttributeValue;
use crate::prelude::XAttributeKind;
use crate::prelude::XAttributeName;
use crate::prelude::diagnostics::trace;
use crate::prelude::diagnostics::trace_span;
use crate::prelude::diagnostics::warn;
use crate::signal::depth::Depth;

pub fn merge(
    depth: Depth,
    new_attributes: &mut [XAttribute],
    old_attributes: &mut [XAttribute],
    element: &LiveElement,
) {
    let _span = trace_span!("Attributes").entered();
    trace!(
        new_count = new_attributes.len(),
        old_count = old_attributes.len(),
        "Count"
    );

    let mut old_attributes_set = HashSet::new();
    for old_attribute in old_attributes.iter_mut() {
        old_attributes_set.insert(std::mem::replace(
            &mut old_attribute.id.name,
            XAttributeName::zero(),
        ));
    }

    *element.attributes.borrow_mut() = Default::default();
    let css_style = LazyCell::new(|| element.css_style());

    let mut i = 0;
    for chunk in chunks_mut(new_attributes) {
        let _span = trace_span!("Chunk", chunk = %chunk.name, index = chunk.index).entered();
        let tmp: Option<Option<String>>;
        let value_acc = match chunk.chunk_kind {
            ChunkKind::Dynamic | ChunkKind::Static | ChunkKind::Single => {
                for new_attribute in chunk.attributes {
                    let i = guard(&mut i, |i| *i += 1);
                    old_attributes_set.remove(&new_attribute.id.name);
                    let old_attribute_value =
                        find_old_attribute(new_attribute, **i, old_attributes);
                    new_attribute.merge(depth, element, old_attribute_value);
                }

                tmp = aggregate_attribute(element.attributes.borrow().get_chunk(chunk.index));
                trace!("Dynamic chunk value: {tmp:?}");
                let Some(tmp) = &tmp else {
                    continue;
                };
                tmp.as_deref()
            } // TODO: optimization in case attribute is static or single, no need to build the attribute diff list
              // ChunkKind::Static => {
              //     tmp = aggregate_attribute(chunk.attributes.iter().map(attribute_value_to_option));
              //     tmp.as_deref()
              // }
              // ChunkKind::Single => {
              //     attribute_value_to_option(&chunk.attributes[0]).map(|s| s.into())
              // },
        };

        set_attribute(&element.html, &css_style, &chunk.name, value_acc);
    }

    #[cfg(feature = "diagnostics")]
    assert!(i == new_attributes.len());

    for XAttributeName { name, kind } in old_attributes_set {
        match kind {
            XAttributeKind::Attribute => match element.html.remove_attribute(&name) {
                Ok(()) => trace!("Removed attribute {name}"),
                Err(error) => warn!("Removed attribute {name} failed: {error:?}"),
            },
            XAttributeKind::Style => match css_style.remove_property(&name) {
                Ok(value) => trace!("Removed style {name}: {value}"),
                Err(error) => warn!("Removed style {name} failed: {error:?}"),
            },
        }
    }
}

fn chunks_mut(attributes: &mut [XAttribute]) -> impl Iterator<Item = Chunk<'_>> {
    return attributes
        .chunk_by_mut(|x, y| x.id.name == y.id.name)
        .map(|chunk| {
            let first = &chunk[0];
            Chunk {
                chunk_kind: ChunkKind::of(chunk),
                name: first.id.name.clone(),
                index: first.id.index,
                attributes: chunk,
            }
        });
}

struct Chunk<'t> {
    chunk_kind: ChunkKind,
    name: XAttributeName,
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

fn find_old_attribute(
    new_attribute: &XAttribute,
    i: usize,
    old_attributes: &mut [XAttribute],
) -> Option<XAttributeValue> {
    let old_attribute = old_attributes.get_mut(i)?;
    if old_attribute.id == new_attribute.id {
        return Some(std::mem::replace(
            &mut old_attribute.value,
            XAttributeValue::Null,
        ));
    }
    return None;
}
