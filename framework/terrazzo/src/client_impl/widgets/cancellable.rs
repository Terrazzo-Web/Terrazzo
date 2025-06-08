//! Utils to cancel function calls

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use terrazzo_client::prelude::Ptr;

use self::inner::CancellableInner;
use self::inner::CancellableState;
use super::debounce::DoDebounce;

pub struct Cancellable<S>(Ptr<CancellableInner<S>>);

impl<D: DoDebounce> Cancellable<D> {
    pub(super) fn of(do_debounce: D) -> Self {
        Self(Ptr::new(CancellableInner {
            version: AtomicUsize::new(0),
            state: do_debounce,
        }))
    }

    pub fn wrap<T: 'static>(&self, f: impl Fn(T) + 'static) -> impl Fn(T) + 'static {
        let this = self.clone();
        let f = move |(version, arg)| {
            if version == this.current_version() {
                f(arg);
            }
        };
        let f = self.state.debounce(f);

        let this = self.clone();
        let f = move |arg| f((this.current_version(), arg));
        return f;
    }
}

mod inner {
    use std::ops::Deref;
    use std::sync::atomic::AtomicUsize;

    use super::Cancellable;

    pub struct CancellableInner<D> {
        pub(super) version: AtomicUsize,
        pub(super) state: D,
    }

    impl<D> Deref for Cancellable<D> {
        type Target = CancellableInner<D>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    #[derive(PartialEq, Eq)]
    pub(super) struct CancellableState(pub(super) usize);
}

impl Cancellable<()> {
    pub fn new() -> Self {
        ().cancellable()
    }

    pub fn capture<I, O>(&self, f: impl Fn(I) -> O + 'static) -> impl Fn(I) -> Option<O> + 'static {
        let this = self.clone();
        let v = this.current_version();
        move |i| {
            if this.current_version() == v {
                Some(f(i))
            } else {
                None
            }
        }
    }
}

impl Default for Cancellable<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Cancellable<S> {
    pub fn cancel(&self) {
        self.version.fetch_add(1, SeqCst);
    }

    fn current_version(&self) -> CancellableState {
        CancellableState(self.version.load(SeqCst))
    }
}

impl<S> Clone for Cancellable<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::widgets::debounce::DoDebounce;

    #[test]
    fn cancellable_function() {
        let handle = ().cancellable();
        let f = handle.capture(|a: i32| a * a);
        assert_eq!(Some(4), f(2));
        assert_eq!(Some(4), f(2));
        handle.cancel();
        assert_eq!(None, f(2));
    }
}
