//! Utils to cancel function calls

use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use self::inner::CancellableInner;
use self::inner::CancellableState;
use super::debounce::DoDebounce;

#[derive(Clone)]
pub struct Cancellable<D>(Rc<CancellableInner<D>>);

impl<D: DoDebounce> Cancellable<D> {
    pub(super) fn new(do_debounce: D) -> Self {
        Self(Rc::new(CancellableInner {
            version: AtomicUsize::new(0),
            do_debounce,
        }))
    }

    pub fn wrap<T: 'static>(&self, f: impl Fn(T) + 'static) -> impl Fn(T) + 'static {
        let this = self.clone();
        let f = move |(version, arg)| {
            if version == this.current_version() {
                f(arg);
            }
        };
        let f = self.do_debounce.debounce(f);

        let this = self.clone();
        let f = move |arg| f((this.current_version(), arg));
        return f;
    }

    pub fn cancel(&self) {
        self.version.fetch_add(1, SeqCst);
    }

    fn current_version(&self) -> CancellableState {
        CancellableState(self.version.load(SeqCst))
    }
}

mod inner {
    use std::ops::Deref;
    use std::sync::atomic::AtomicUsize;

    use super::Cancellable;

    pub struct CancellableInner<D> {
        pub(super) version: AtomicUsize,
        pub(super) do_debounce: D,
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
