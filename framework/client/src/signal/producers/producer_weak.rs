use std::sync::Mutex;

use super::producer::ProducedValue;
use super::producer::Producer;
use super::producer::ProducerInner;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::string::XString;
use crate::utils::Ptr;
use crate::utils::PtrWeak;

impl<V: ProducedValue> Producer<V> {
    pub(super) fn downgrade(&self) -> ProducerWeak<V> {
        ProducerWeak {
            inner: Ptr::downgrade(&self.inner),
        }
    }
}

pub struct ProducerWeak<V: ProducedValue> {
    inner: PtrWeak<(DebugCorrelationId<XString>, Mutex<ProducerInner<V>>)>,
}

impl<V: ProducedValue> ProducerWeak<V> {
    pub(super) fn upgrade(&self) -> Option<Producer<V>> {
        self.inner
            .upgrade()
            .map(|producer| Producer { inner: producer })
    }
}

impl<V: ProducedValue> Clone for ProducerWeak<V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
