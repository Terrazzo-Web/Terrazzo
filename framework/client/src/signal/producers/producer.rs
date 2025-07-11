use std::iter::once;
use std::sync::Mutex;

use super::consumer::Consumer;
use super::consumer::ConsumerWeak;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::debug_span;
use crate::prelude::diagnostics::trace;
use crate::string::XString;
use crate::utils::Ptr;

pub trait ProducedValue {
    type SortKey: Clone + Ord;
    type Value;
}

pub struct Producer<V: ProducedValue> {
    pub(super) inner: Ptr<(DebugCorrelationId<XString>, Mutex<ProducerInner<V>>)>,
}

impl<V: ProducedValue> Clone for Producer<V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

pub(super) struct ProducerInner<V: ProducedValue> {
    pub consumers: Ptr<Vec<ConsumerWeak<V>>>,
    state: ConsumersState,
}

#[derive(Debug)]
enum ConsumersState {
    Sorted,
    NotSorted,
}

impl<V: ProducedValue> Producer<V> {
    pub fn new(name: XString) -> Self {
        Self {
            inner: Ptr::new((
                DebugCorrelationId::new(|| name),
                Mutex::new(ProducerInner {
                    consumers: Ptr::default(),
                    state: ConsumersState::Sorted,
                }),
            )),
        }
    }

    pub fn register<F: Fn(V::Value) + 'static>(
        &self,
        consumer_name: DebugCorrelationId<XString>,
        sort_key: V::SortKey,
        closure: F,
    ) -> Consumer<V> {
        let _span =
            debug_span! { "Register", producer_name = %self.name(), %consumer_name }.entered();
        let mut producer_lock = self.inner.1.lock().or_throw("producer_lock");
        let consumer = Consumer::new(consumer_name, self, sort_key, closure);
        let is_still_sorted = producer_lock
            .consumers
            .last()
            .and_then(|consumer| consumer.upgrade())
            .map(|last| last.composite_key() < consumer.composite_key())
            .unwrap_or(true);
        if is_still_sorted {
            trace!("Still: {:?}", producer_lock.state);
        } else {
            trace!("Not sorted");
            producer_lock.state = ConsumersState::NotSorted;
        }
        let consumers = std::mem::take(&mut producer_lock.consumers);
        let consumers = match Ptr::try_unwrap(consumers) {
            Ok(mut consumers) => {
                consumers.push(consumer.downgrade());
                consumers
            }
            Err(consumers) => consumers
                .iter()
                .cloned()
                .chain(once(consumer.downgrade()))
                .collect::<Vec<_>>(),
        };
        trace!("Consumers count: {}", consumers.len());
        producer_lock.consumers = Ptr::new(consumers);
        return consumer;
    }

    pub fn consumers(&self) -> impl Iterator<Item = Consumer<V>>
    where
        V: 'static,
    {
        let consumers = {
            let mut producer_lock = self.inner.1.lock().or_throw("producer_lock");
            if let ConsumersState::NotSorted = producer_lock.state {
                let mut consumers = Ptr::try_unwrap(std::mem::take(&mut producer_lock.consumers))
                    .unwrap_or_else(|consumers| consumers.as_ref().clone());
                consumers.sort_by(|a, b| {
                    Ord::cmp(
                        &a.upgrade().as_ref().map(|a| a.composite_key()),
                        &b.upgrade().as_ref().map(|b| b.composite_key()),
                    )
                });
                producer_lock.consumers = Ptr::new(consumers);
                producer_lock.state = ConsumersState::Sorted;
            }
            producer_lock.consumers.clone()
        };
        ConsumersIterator::new(consumers).filter_map(|consumer| consumer.upgrade())
    }

    pub fn process(&self, value: V::Value)
    where
        V: 'static,
        V::Value: Clone,
    {
        for consumer in self.consumers() {
            consumer.consume(value.clone());
        }
    }

    pub fn name(&self) -> &DebugCorrelationId<XString> {
        &self.inner.0
    }
}

struct ConsumersIterator<V: ProducedValue> {
    consumers: Ptr<Vec<ConsumerWeak<V>>>,
    index: usize,
}

impl<V: ProducedValue> ConsumersIterator<V> {
    fn new(consumers: Ptr<Vec<ConsumerWeak<V>>>) -> Self {
        Self {
            consumers,
            index: 0,
        }
    }
}

impl<V: ProducedValue> Iterator for ConsumersIterator<V> {
    type Item = ConsumerWeak<V>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.consumers.len() {
            return None;
        } else {
            let result = Some(self.consumers[self.index].clone());
            self.index += 1;
            return result;
        }
    }
}
