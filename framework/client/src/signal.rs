//! Reactive signals

use std::sync::Mutex;

use scopeguard::defer;

use self::batch::Batch;
use self::batch::NotBatched;
use self::depth::Depth;
use self::producers::producer::ProducedValue;
use self::producers::producer::Producer;
use self::version::Version;
use super::string::XString;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::Consumers;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::debug;
use crate::prelude::diagnostics::debug_span;
use crate::prelude::diagnostics::trace;
use crate::utils::Ptr;

pub mod batch;
pub mod depth;
pub mod derive;
pub mod mutable_signal;
mod producers;
pub mod reactive_closure;
mod tests;
mod version;
pub mod weak;

use self::inner::XSignalInner;

/// A mutable value that callbacks can subscribe to.
///
/// - Derived signals
/// - ReactiveClosures re-compute and update HTML nodes when signals change
pub struct XSignal<T>(Ptr<XSignalInner<T>>);

mod inner {
    use std::ops::Deref;
    use std::sync::Mutex;

    use super::ProducedSignal;
    use super::XSignal;
    use super::XSignalValue;
    use super::producers::producer::Producer;
    use crate::utils::Ptr;

    pub struct XSignalInner<T> {
        pub(super) current_value: Mutex<XSignalValue<T>>,
        pub(super) producer: Producer<ProducedSignal>,
        pub(super) immutable_value: Ptr<Mutex<Option<T>>>,
        pub(super) on_drop: Mutex<Vec<Box<dyn FnOnce()>>>,
    }

    impl<T> Deref for XSignal<T> {
        type Target = XSignalInner<T>;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
}

pub struct ProducedSignal;

impl ProducedValue for ProducedSignal {
    /// The SortKey is [Depth], ensuring that parent nodes are recomputed before child nodes.
    type SortKey = Depth;
    type Value = Version;
}

#[derive(Clone, Debug)]
struct XSignalValue<T> {
    value: Option<T>,
    version: Version,
}

impl<T> XSignalValue<T> {
    fn value(&self) -> &T {
        self.value
            .as_ref()
            .or_throw("Value should never be null until dropped")
    }

    fn value_mut(&mut self) -> &mut T {
        self.value
            .as_mut()
            .or_throw("Value should never be null until dropped")
    }
}

impl<T> XSignal<T> {
    /// Create a new signal.
    ///
    /// The name of the signal is used in console logs, does not have to be unique.
    pub fn new(name: impl Into<XString>, value: T) -> Self {
        Self(Ptr::new(XSignalInner {
            current_value: Mutex::new(XSignalValue {
                value: Some(value),
                version: Version::current(),
            }),
            producer: Producer::new(name.into()),
            immutable_value: Ptr::default(),
            on_drop: Mutex::new(vec![]),
        }))
    }

    /// Registers a callback that will trigger when the signal is updated.
    #[must_use]
    pub fn add_subscriber(&self, closure: impl Fn(T) + 'static) -> Consumers
    where
        T: Clone + 'static,
    {
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering::SeqCst;
        let weak = self.downgrade();
        let last_version = AtomicUsize::new(0);
        let closure = move |version: Version| {
            let Some(this) = weak.upgrade() else { return };
            let version = version.number();
            let last_version = last_version.swap(version, SeqCst);
            if last_version < version {
                defer!(trace!("End"));
                trace!(last_version, version, "Start");
                closure(this.get_value_untracked())
            } else {
                debug!(last_version, version, "Skip");
            }
        };
        Consumers(vec![self.producer.register(
            DebugCorrelationId::new(|| "[closure]".into()),
            Depth::zero(),
            closure,
        )])
    }

    /// Gets the current value of the signal.
    ///
    /// Reactive behavior should use [XSignal::add_subscriber].
    pub fn get_value_untracked(&self) -> T
    where
        T: Clone,
    {
        self.current_value
            .lock()
            .or_throw("get_value_untracked()")
            .value()
            .clone()
    }
}

impl<T> Clone for XSignal<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: std::fmt::Debug + 'static> XSignal<T> {
    /// Updates the signal by setting a new value.
    pub fn set(&self, new_value: impl Into<T>)
    where
        T: Eq,
    {
        let _span = debug_span!("Set", signal = %self.producer.name()).entered();
        self.update_impl(|old_value| {
            let new_value = new_value.into();
            (new_value != *old_value).then_some(new_value)
        });
    }

    /// Updates the signal by computing a new value from the old one.
    pub fn update<R, U>(&self, compute: impl FnOnce(&T) -> U) -> R
    where
        U: Into<UpdateSignalResult<Option<T>, R>>,
    {
        let _span = debug_span!("Update", signal = %self.producer.name()).entered();
        self.update_impl(|t| compute(t))
    }

    /// Updates the signal by computing a new value from the old one.
    ///
    /// The old value is mutable and can be reused to compute the new value.
    pub fn update_mut<R, U>(&self, compute: impl FnOnce(&mut T) -> U) -> R
    where
        U: Into<UpdateSignalResult<T, R>>,
    {
        let _span = debug_span!("Update mut", signal = %self.producer.name()).entered();
        self.update_impl(|t| {
            let UpdateSignalResult { new_value, result } = compute(t).into();
            UpdateSignalResult {
                new_value: Some(new_value),
                result,
            }
        })
    }

    fn update_impl<R, U>(&self, compute: impl FnOnce(&mut T) -> U) -> R
    where
        U: Into<UpdateSignalResult<Option<T>, R>>,
    {
        let (version, result) = {
            let mut current = self.current_value.lock().or_throw("current_value");
            let current_version = current.version.number();
            let current_value = current.value_mut();
            let UpdateSignalResult { new_value, result } = compute(current_value).into();
            let Some(new_value) = new_value else {
                debug! { "Signal value is not changing current@{current_version}={current_value:?}" };
                return result;
            };
            let new_version: Version = Version::next();
            debug!(
                "Signal value has changed old@{}={current_value:?} vs new@{}={new_value:?}",
                current_version,
                new_version.number()
            );
            current.version = new_version;
            current.value = Some(new_value);
            (new_version, result)
        };
        self.process_or_batch(version);
        return result;
    }

    /// Updates the signal by setting a new value.
    ///
    /// Contrary to [XSignal::set], the signal triggers even if the value didn't change.
    pub fn force(&self, new_value: impl Into<T>) {
        let _span = debug_span!("Force", signal = %self.producer.name()).entered();
        let new_value = new_value.into();
        let version = {
            let mut current = self.current_value.lock().or_throw("current_value");
            current.value = Some(new_value);
            current.version = Version::next();
            debug! { "Signal value was forced to version:{} value:{:?}", current.version.number(), current.value() };
            current.version
        };
        self.process_or_batch(version);
    }

    fn process_or_batch(&self, version: Version) {
        Batch::try_push(|| {
            let this = self.to_owned();
            trace!("Update is batched");
            move |version| this.process(version)
        })
        .unwrap_or_else(|NotBatched { .. }| {
            trace!("Update is applied immediately");
            self.process(version)
        });
    }

    fn process(&self, version: Version) {
        self.producer.process(version);
    }
}

/// A struct that represents the result of [updating a signal].
///
/// By default, updating a signal means assigning some new value and returning `()`.
///
/// [updating a signal]: XSignal::update
pub struct UpdateSignalResult<T, R> {
    pub new_value: T,
    pub result: R,
}

impl<T> From<T> for UpdateSignalResult<T, ()> {
    fn from(new_value: T) -> Self {
        Self {
            new_value,
            result: (),
        }
    }
}

/// A shortcut to run some update logic on a signal and return a non-void value.
///
/// ```
/// # use terrazzo_client::prelude::*;
/// let signal = XSignal::new("signal", "1".to_owned());
/// let new = signal.update(|old| {
///     let old = old.parse::<i32>().unwrap();
///     let new = old + 1;
///     return Some(new.to_string()).and_return(new);
/// });
///
/// // We got the updated value as an integer while the signal contains a string.
/// assert_eq!(new, 2);
/// ```
pub trait UpdateAndReturn {
    type NewValue;
    fn and_return<R>(self, result: R) -> UpdateSignalResult<Self::NewValue, R>;
}

impl<T> UpdateAndReturn for T {
    type NewValue = T;

    fn and_return<R>(self, result: R) -> UpdateSignalResult<Self::NewValue, R> {
        UpdateSignalResult {
            new_value: self,
            result,
        }
    }
}

impl<T> Drop for XSignalInner<T> {
    fn drop(&mut self) {
        debug!(signal = %self.producer.name(), "Dropped");
        if Ptr::strong_count(&self.immutable_value) > 1 {
            let mut immutable_value = self.immutable_value.lock().or_throw("immutable_value");
            *immutable_value = self.current_value.lock().or_throw("current").value.take();
        }
        let mut on_drop = self.on_drop.lock().or_throw("on_drop");
        for on_drop in std::mem::take(&mut *on_drop) {
            on_drop()
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for XSignal<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("XSignal")
            .field(self.current_value.lock().or_throw("current_value").value())
            .finish()
    }
}

/// Safe because Javascript is single-threaded.
unsafe impl<T: Send> Send for XSignal<T> {}

/// Safe because Javascript is single-threaded.
unsafe impl<T: Sync> Sync for XSignal<T> {}
