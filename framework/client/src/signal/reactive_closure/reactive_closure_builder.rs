use std::panic::Location;
use std::sync::atomic::AtomicUsize;

use autoclone::autoclone;
use scopeguard::defer;

use super::ReactiveClosure;
use crate::debug_correlation_id::DebugCorrelationId;
use crate::prelude::OrElseLog as _;
use crate::prelude::diagnostics::debug;
use crate::prelude::diagnostics::debug_span;
use crate::prelude::diagnostics::trace;
use crate::signal::ProducedSignal;
use crate::signal::XSignal;
use crate::signal::producers::consumer::Consumer;
use crate::signal::producers::producer::Producer;
use crate::signal::version::Version;
use crate::string::XString;
use crate::template::IsTemplate;
use crate::utils::Ptr;

/// A builder for ReactiveClosure.
///
/// The closure initially takes multiple parameters that must be bound to signals until we are left with a `Fn()`.
#[must_use]
pub struct ReactiveClosureBuilder<F> {
    /// A name, for troubleshooting purposes only.
    name: DebugCorrelationId<XString>,

    /// The closure
    reactive_closure: F,

    /// The list of signals that the final closure needs to subscribe to.
    producers: Vec<Producer<ProducedSignal>>,
}

/// Binds the parameter of a reactive closure to a signal.
///
/// This is mainly used by `#[template]` code-generated code.
pub trait BindReactiveClosure<F, BF, I, O>
where
    F: Fn() -> BF,
    BF: FnOnce(I) -> O,
{
    fn bind(self, signal: XSignal<I>) -> ReactiveClosureBuilder<impl Fn() -> O>;
}

impl<F, BF, I, O> BindReactiveClosure<F, BF, I, O> for ReactiveClosureBuilder<F>
where
    F: Fn() -> BF,
    BF: FnOnce(I) -> O,
    I: Clone + 'static,
{
    fn bind(self, signal: XSignal<I>) -> ReactiveClosureBuilder<impl Fn() -> O> {
        let span = debug_span!("Bind", closure = %self.name, signal = %signal.0.producer.name());
        let _span = span.clone().entered();
        let reactive_closure = self.reactive_closure;
        trace!("Bind");
        let signal_weak = signal.downgrade();
        let immutable_value = signal.0.immutable_value.clone();
        let bound_closure = move || {
            let current_value = {
                if let Some(signal) = signal_weak.upgrade() {
                    let lock = &signal.0.current_value.lock().or_throw("current_value");
                    lock.value().clone()
                } else {
                    // Signal -> ReactiveClosure
                    // ReactiveClosure -> Weak<Signal>: to read the value
                    // ReactiveClosure -> Weak<Signal>: to unsubscribe if dropped
                    let _span = span.enter();
                    debug!("Signal is dropped, keep previous value");
                    let immutable_value = immutable_value.lock().or_throw("immutable_value");
                    immutable_value.as_ref().or_throw("immutable_value").clone()
                }
            };
            reactive_closure()(current_value)
        };
        let mut producers = self.producers;
        producers.push(signal.0.producer.clone());
        return ReactiveClosureBuilder {
            name: self.name,
            reactive_closure: bound_closure,
            producers,
        };
    }
}

impl<F: Fn() + 'static> ReactiveClosureBuilder<F> {
    /// Subscribes the reactive closure to all its signals.
    /// There is no way to call it manually. The only way to get the closure to run is the change the signals.
    #[autoclone]
    pub fn register(self, template: impl IsTemplate) -> Consumers {
        let _span = debug_span!("Register", closure = %self.name).entered();
        let Self {
            name,
            reactive_closure,
            producers,
        } = self;
        let reactive_closure = Ptr::new(ReactiveClosure {
            name,
            reactive_closure,
            last_version: AtomicUsize::new(0),
        });
        trace!("Call");
        reactive_closure.call(Version::current());

        defer!(trace!("Add consumers: Done."));
        trace!("Add consumers");
        let mut consumers = vec![];
        let consumer_name: XString = template.debug_id().to_string().into();
        for producer in producers {
            consumers.push(producer.register(
                DebugCorrelationId::new(|| consumer_name.clone()),
                template.depth(),
                move |version| {
                    autoclone!(reactive_closure);
                    reactive_closure.call(version)
                },
            ));
        }
        return Consumers(consumers);
    }
}

/// Creates a new reactive closure builder.
///
/// This is mainly used by `#[template]` code-generated code.
#[track_caller]
pub fn make_reactive_closure() -> ReactiveClosureBuilderWantClosure {
    ReactiveClosureBuilderWantClosure {
        name: NameOrCallSite::CallSite(std::panic::Location::caller()),
    }
}

#[must_use]
pub struct ReactiveClosureBuilderWantClosure {
    name: NameOrCallSite,
}

enum NameOrCallSite {
    Name(XString),
    CallSite(&'static Location<'static>),
}

impl ReactiveClosureBuilderWantClosure {
    pub fn named(self, name: impl Into<XString>) -> Self {
        Self {
            name: NameOrCallSite::Name(name.into()),
        }
    }

    pub fn closure<F>(self, closure: F) -> ReactiveClosureBuilder<F> {
        closure.into_reactive_closure_builder(match self.name {
            NameOrCallSite::Name(name) => name,
            NameOrCallSite::CallSite(location) => {
                format!("{}:{}", location.file(), location.line()).into()
            }
        })
    }
}

/// Turns a closure into a reactive closure builder.
trait ToReactiveClosureBuilder: Sized {
    fn into_reactive_closure_builder(
        self,
        name: impl Into<XString>,
    ) -> ReactiveClosureBuilder<Self>;
}

impl<F> ToReactiveClosureBuilder for F {
    fn into_reactive_closure_builder(
        self,
        name: impl Into<XString>,
    ) -> ReactiveClosureBuilder<Self> {
        let name = DebugCorrelationId::new(|| name.into());
        debug!(closure = %name, "ReactiveClosure new");
        ReactiveClosureBuilder {
            name,
            reactive_closure: self,
            producers: vec![],
        }
    }
}

/// A struct that holds consumers, i.e. callbacks that are executed when producers execute.
///
/// This is used by the signaling mechanism.
#[derive(Default)]
pub struct Consumers(pub(crate) Vec<Consumer<ProducedSignal>>);

impl std::fmt::Debug for Consumers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let consumers: Vec<String> = self.0.iter().map(|c| format!("{:?}", c)).collect();
        write!(f, "[{}]", consumers.join(", "))
    }
}

/// Safe because Javascript is single-threaded.
unsafe impl Send for Consumers {}

/// Safe because Javascript is single-threaded.
unsafe impl Sync for Consumers {}

impl Consumers {
    pub fn append(mut self, mut other: Self) -> Self {
        self.0.append(&mut other.0);
        return self;
    }
}
