use std::cell::LazyCell;
use std::collections::HashSet;

use web_sys::CssStyleDeclaration;

use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::trace;
use self::diagnostics::trace_span;
use self::diagnostics::warn;
use super::XAttribute;
use super::template::LiveElement;
use crate::attribute::attribute::set_attribute;
use crate::attribute::diff_store::AttributeValueDiffStore;
use crate::attribute::diff_store::Chunk;
use crate::attribute::diff_store::ChunkKind;
use crate::attribute::diff_store::DynamicBackend;
use crate::attribute::diff_store::SingleBackend;
use crate::attribute::diff_store::StaticBackend;
use crate::attribute::value::XAttributeValue;
use crate::prelude::XAttributeKind;
use crate::prelude::XAttributeName;
use crate::prelude::diagnostics;
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
        // TODO: should be trace_span!
        let span = debug_span!("Chunk", chunk = %chunk.name, index = chunk.index, kind = ?chunk.chunk_kind);
        let _span = span.enter();
        match chunk.chunk_kind {
            ChunkKind::Dynamic => {
                merge_chunk(
                    chunk,
                    depth,
                    element,
                    &css_style,
                    old_attributes,
                    &mut old_attributes_set,
                    DynamicBackend::new(element),
                    &mut i,
                );
            }
            ChunkKind::Static => {
                let backend = StaticBackend::new(&chunk);
                merge_chunk(
                    chunk,
                    depth,
                    element,
                    &css_style,
                    old_attributes,
                    &mut old_attributes_set,
                    backend,
                    &mut i,
                );
            }
            ChunkKind::Single => {
                let backend = SingleBackend::default();
                merge_chunk(
                    chunk,
                    depth,
                    element,
                    &css_style,
                    old_attributes,
                    &mut old_attributes_set,
                    backend,
                    &mut i,
                );
            }
        };
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

fn merge_chunk(
    chunk: Chunk<'_>,
    depth: Depth,
    element: &LiveElement,
    css_style: &LazyCell<CssStyleDeclaration, impl FnOnce() -> CssStyleDeclaration>,
    old_attributes: &mut [XAttribute],
    old_attributes_set: &mut HashSet<XAttributeName>,
    mut backend: impl AttributeValueDiffStore,
    i: &mut usize,
) {
    for new_attribute in chunk.attributes {
        old_attributes_set.remove(&new_attribute.id.name);
        let old_attribute_value = find_old_attribute(new_attribute, *i, old_attributes);
        new_attribute.merge(depth, element, &mut backend, old_attribute_value);
        *i += 1;
    }
    let value_acc = backend.aggregate_attribute(chunk.index);
    let value_acc = value_acc.as_ref().map(|v| v.as_ref().map(|v| v.as_ref()));
    debug!("Merge chunk value: {value_acc:?}");
    let Some(value_acc) = value_acc else {
        return;
    };
    set_attribute(&element.html, &css_style, &chunk.name, value_acc);
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
