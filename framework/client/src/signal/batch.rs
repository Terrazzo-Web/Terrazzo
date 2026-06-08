use std::marker::PhantomData;
use std::sync::LazyLock;
use std::sync::Mutex;

use super::version::Version;
use crate::prelude::diagnostics::debug;
use crate::prelude::diagnostics::debug_span;
use crate::prelude::diagnostics::error;
use crate::prelude::diagnostics::span::Span;

/// Allows batching several signal writes into one refresh.
///
/// This is an optimization to avoid executing the merge and rendering logic several times.
///
/// The rendering logic is delayed until the [Batch] object is droppped.
///
/// Example:
/// ```
/// # use terrazzo_client::prelude::*;
/// // Define some signals in the application logic
/// let signal1 = XSignal::new("my string signal", "value");
/// let signal2 = XSignal::new("my integer signal", 0);
///
/// // Open a new batch
/// let batch = Batch::use_batch("Update nodes");
///
/// // Updating signals has no side effects...
/// signal1.set("new value");
/// signal2.set(1);
///
/// // ... until the batch is dropped, which triggers all the batched updates and refreshes the UI.
/// drop(batch);
/// ```
pub struct Batch {
    prev: Option<BatchedCallbacks>,
    forget: bool,
    span: Span,
    _not_send: PhantomData<*mut u8>,
}

#[derive(Default)]
struct BatchedCallbacks(Vec<Box<dyn FnOnce(Version)>>);

/// Safe because Javascript is single-threaded.
unsafe impl Send for BatchedCallbacks {}

static WAITING_BATCH: LazyLock<Mutex<Option<BatchedCallbacks>>> =
    LazyLock::new(|| Mutex::new(None));

impl Batch {
    pub fn use_batch(name: &str) -> Self {
        let span = debug_span!("Batch", batch = name);
        let _span = span.clone().entered();
        debug!("Starting batch");
        let Ok(mut batch) = WAITING_BATCH.lock().map_err(|error| {
            error!("Batch::use_batch failed to lock WAITING_BATCH: {error}");
        }) else {
            return Self {
                prev: None,
                forget: true,
                span,
                _not_send: PhantomData,
            };
        };
        let new_batch = Self {
            prev: batch.take(),
            forget: false,
            span,
            _not_send: PhantomData,
        };
        *batch = Some(BatchedCallbacks::default());
        return new_batch;
    }

    pub(super) fn try_push<M: FnOnce() -> P, P: FnOnce(Version) + 'static>(
        make_callback: M,
    ) -> Result<(), NotBatched> {
        let Ok(mut batch) = WAITING_BATCH.lock().map_err(|error| {
            error!("Batch::try_push failed to lock WAITING_BATCH: {error}");
        }) else {
            return Err(NotBatched(()));
        };
        if let Some(batch) = &mut *batch {
            batch.0.push(Box::new(make_callback()));
            return Ok(());
        } else {
            return Err(NotBatched(()));
        };
    }

    pub fn forget(&mut self) {
        self.set_forget(true);
    }

    pub fn set_forget(&mut self, forget: bool) {
        self.forget = forget;
    }
}

#[derive(Debug)]
pub(super) struct NotBatched(());

impl Drop for Batch {
    fn drop(&mut self) {
        let _span = self.span.enter();
        debug!("Processing batch...");
        let Ok(mut waiting_batch) = WAITING_BATCH.lock().map_err(|error| {
            error!("Batch::drop failed to lock WAITING_BATCH: {error}");
        }) else {
            return;
        };
        let Some(callbacks) = std::mem::replace(&mut *waiting_batch, self.prev.take()) else {
            error!("Batch::drop failed because WAITING_BATCH was empty");
            return;
        };
        if self.forget {
            debug!("Processing batch: Skipped");
            return;
        }
        for callback in callbacks.0 {
            callback(Version::current());
        }
        debug!("Processing batch: DONE");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use autoclone::autoclone;

    use super::Batch;
    use crate::utils::Ptr;

    #[test]
    #[autoclone]
    fn batch() {
        let v = Ptr::new(RefCell::new(vec![]));
        v.borrow_mut().push("init");

        {
            let _batch = Batch::use_batch("First batch");
            Batch::try_push(|| {
                move |_version| {
                    autoclone!(v);
                    v.borrow_mut().push("batched")
                }
            })
            .unwrap();
            assert_eq!(vec!["init"], *v.borrow());
        }
        assert_eq!(vec!["init", "batched"], *v.borrow());

        Batch::try_push(|| move |_version| panic!("Batch is not active"))
            .expect_err("Batch is not active");
        assert_eq!(vec!["init", "batched"], *v.borrow());

        {
            let _batch = Batch::use_batch("Second batch");
            Batch::try_push(|| {
                move |_version| {
                    autoclone!(v);
                    v.borrow_mut().push("batched2")
                }
            })
            .unwrap();
            assert_eq!(vec!["init", "batched"], *v.borrow());
        }
        assert_eq!(vec!["init", "batched", "batched2"], *v.borrow());

        Batch::try_push(|| move |_version| panic!("Batch is not active"))
            .expect_err("Batch is not active");
        assert_eq!(vec!["init", "batched", "batched2"], *v.borrow());
    }

    #[test]
    #[autoclone]
    fn forget() {
        let v = Ptr::new(RefCell::new(vec![]));
        v.borrow_mut().push("init");

        {
            let _batch = Batch::use_batch("First batch");
            Batch::try_push(|| {
                move |_version| {
                    autoclone!(v);
                    v.borrow_mut().push("batched")
                }
            })
            .unwrap();
            assert_eq!(vec!["init"], *v.borrow());
        }
        assert_eq!(vec!["init", "batched"], *v.borrow());

        {
            let mut batch = Batch::use_batch("Second batch");
            Batch::try_push(|| {
                move |_version| {
                    autoclone!(v);
                    v.borrow_mut().push("batched2")
                }
            })
            .unwrap();
            batch.forget();
            assert_eq!(vec!["init", "batched"], *v.borrow());
        }
        assert_eq!(vec!["init", "batched"], *v.borrow());
    }
}
