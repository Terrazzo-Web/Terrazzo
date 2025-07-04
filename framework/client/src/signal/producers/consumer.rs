use nameth::NamedType as _;
use nameth::nameth;

use super::consumer_id::ConsumerId;
use super::producer::ProducedValue;
use super::producer::Producer;
use super::producer_weak::ProducerWeak;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::trace;
use crate::string::XString;
use crate::utils::Ptr;
use crate::utils::PtrWeak;

#[must_use]
#[nameth]
pub struct Consumer<V: ProducedValue> {
    inner: Ptr<ConsumerInner<V, dyn Fn(V::Value)>>,
}

impl<V: ProducedValue> Consumer<V> {
    pub fn new<F>(
        consumer_name: DebugCorrelationId<XString>,
        producer: &Producer<V>,
        sort_key: V::SortKey,
        closure: F,
    ) -> Self
    where
        F: Fn(V::Value) + 'static,
    {
        let consumer_id = ConsumerId::new();
        trace!(%consumer_id, "New consumer");
        Self {
            inner: Ptr::new(ConsumerInner {
                id: consumer_id,
                name: consumer_name,
                sort_key,
                producer: producer.downgrade(),
                closure,
            }),
        }
    }

    pub fn consume(&self, value: V::Value) {
        (self.inner.closure)(value)
    }
}

impl<V: ProducedValue> Clone for Consumer<V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct ConsumerInner<V: ProducedValue, F: Fn(V::Value) + ?Sized> {
    id: ConsumerId,
    name: DebugCorrelationId<XString>,
    sort_key: V::SortKey,
    producer: ProducerWeak<V>,
    closure: F,
}

impl<V: ProducedValue> Consumer<V> {
    pub(super) fn composite_key(&self) -> (&V::SortKey, ConsumerId) {
        (&self.inner.sort_key, self.inner.id)
    }
}

impl<V: ProducedValue, F: Fn(V::Value) + ?Sized> Drop for ConsumerInner<V, F> {
    fn drop(&mut self) {
        trace!(consumer_id = %self.id, consumer_name = %self.name, "Drop consumer");
        if let Some(producer) = self.producer.upgrade() {
            let mut producer_lock = producer.inner.1.lock().or_throw("producer_lock");
            let consumers = Ptr::try_unwrap(std::mem::take(&mut producer_lock.consumers))
                .unwrap_or_else(|consumers| consumers.as_ref().clone())
                .into_iter()
                .filter(|consumer| {
                    let Some(consumer) = consumer.upgrade() else {
                        return false;
                    };
                    return consumer.inner.id != self.id;
                })
                .collect();
            producer_lock.consumers = Ptr::new(consumers);
        }
    }
}

pub struct ConsumerWeak<V: ProducedValue> {
    inner: PtrWeak<ConsumerInner<V, dyn Fn(V::Value)>>,
}

impl<V: ProducedValue> Consumer<V> {
    pub(super) fn downgrade(&self) -> ConsumerWeak<V> {
        ConsumerWeak {
            inner: Ptr::downgrade(&self.inner),
        }
    }
}

impl<V: ProducedValue> ConsumerWeak<V> {
    pub(super) fn upgrade(&self) -> Option<Consumer<V>> {
        self.inner
            .upgrade()
            .map(|consumer| Consumer { inner: consumer })
    }
}

impl<V: ProducedValue> Clone for ConsumerWeak<V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<V: ProducedValue> std::fmt::Debug for Consumer<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(Self::type_name())
            .field("id", &self.inner.id)
            .field("name", &self.inner.name)
            .finish()
    }
}
