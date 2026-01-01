use super::attribute::XAttribute;
use super::builder::AttributeValueDiff;
use super::builder::aggregate_attribute;
use super::id::XAttributeId;
use super::name::XAttributeName;
use super::value::XAttributeValue;
use crate::element::template::LiveElement;

pub trait AttributeValueDiffStore {
    fn set(&mut self, attribute_id: &XAttributeId, value: AttributeValueDiff);
    fn aggregate_attribute(&self, index: usize) -> Option<Option<impl AsRef<str>>>;
}

pub struct DynamicBackend<'t> {
    element: &'t LiveElement,
}

impl<'t> DynamicBackend<'t> {
    pub fn new(element: &'t LiveElement) -> Self {
        Self { element }
    }
}

impl AttributeValueDiffStore for DynamicBackend<'_> {
    fn set(&mut self, attribute_id: &XAttributeId, value: AttributeValueDiff) {
        *self.element.attributes.borrow_mut().get_mut(attribute_id) = value;
    }

    fn aggregate_attribute(&self, index: usize) -> Option<Option<impl AsRef<str>>> {
        aggregate_attribute(self.element.attributes.borrow().get_chunk(index))
    }
}

pub struct StaticBackend {
    values: Vec<AttributeValueDiff>,
}

impl StaticBackend {
    pub fn new(chunk: &Chunk<'_>) -> Self {
        Self {
            values: Vec::with_capacity(chunk.attributes.len()),
        }
    }
}

impl AttributeValueDiffStore for StaticBackend {
    fn set(&mut self, attribute_id: &XAttributeId, value: AttributeValueDiff) {
        if cfg!(feature = "diagnostics") {
            assert!(
                attribute_id.sub_index == self.values.len(),
                "StaticBackend sub_index error. attribute_id.sub_index:{} != self.values.len():{}",
                attribute_id.sub_index,
                self.values.len()
            );
        }
        self.values.push(value);
    }

    fn aggregate_attribute(&self, _index: usize) -> Option<Option<impl AsRef<str>>> {
        aggregate_attribute(&self.values)
    }
}

pub struct Chunk<'t> {
    pub chunk_kind: ChunkKind,
    pub name: XAttributeName,
    pub index: usize,
    pub attributes: &'t mut [XAttribute],
}

#[derive(Debug)]
pub enum ChunkKind {
    Dynamic,
    Static,
    Single,
}

impl ChunkKind {
    pub fn of(chunk: &[XAttribute]) -> ChunkKind {
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
