//! Templates

use crate::debug_correlation_id::DebugCorrelationId;
use crate::signal::depth::Depth;

/// A trait implemented by element templates, e.g. node and attribute templates.
pub trait IsTemplate: Clone + 'static {
    type Value;
    fn apply<R: Into<Self::Value>>(self, new: impl FnOnce() -> R);
    fn depth(&self) -> Depth;
    fn debug_id(&self) -> &DebugCorrelationId<impl std::fmt::Display>;
}

/// A trait implemented by templatable elements, e.g. node and attributes.
pub trait IsTemplated {
    type Template: IsTemplate<Value = Self>;
}
