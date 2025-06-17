use std::cell::RefCell;
use std::marker::PhantomData;

use super::version::Version;
use crate::prelude::OrElseLog as _;
use crate::prelude::tracing::debug;
use crate::prelude::tracing::debug_span;
use crate::prelude::tracing::span::EnteredSpan;

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
    _span: EnteredSpan,
    _not_send: PhantomData<*mut u8>,
}

#[derive(Default)]
struct BatchedCallbacks(Vec<Box<dyn FnOnce(Version)>>);

thread_local! {
    static WAITING_BATCH: RefCell<Option<BatchedCallbacks>> = const { RefCell::new(None) };
}

impl Batch {
    pub fn use_batch(name: &str) -> Self {
        let span = debug_span!("Batch", batch = name).entered();
        debug!("Starting batch");
        WAITING_BATCH.with_borrow_mut(move |batch| {
            let new_batch = Self {
                prev: batch.take(),
                _span: span,
                _not_send: PhantomData,
            };
            *batch = Some(BatchedCallbacks::default());
            return new_batch;
        })
    }

    pub(super) fn try_push<M: FnOnce() -> P, P: FnOnce(Version) + 'static>(
        make_callback: M,
    ) -> Result<(), NotBatched> {
        WAITING_BATCH.with_borrow_mut(|batch| {
            if let Some(batch) = batch {
                batch.0.push(Box::new(make_callback()));
                return Ok(());
            } else {
                return Err(NotBatched(()));
            };
        })
    }
}

#[derive(Debug)]
pub(super) struct NotBatched(());

impl Drop for Batch {
    fn drop(&mut self) {
        debug!("Processing batch...");
        let batch = WAITING_BATCH
            .replace(self.prev.take())
            .or_throw("WAITING_BATCH");
        for process in batch.0 {
            process(Version::current());
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

        Batch::try_push(|| {
            assert!(false, "Batch is not active");
            move |_version| panic!("Batch is not active")
        })
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

        Batch::try_push(|| {
            assert!(false, "Batch is not active");
            move |_version| panic!("Batch is not active")
        })
        .expect_err("Batch is not active");
        assert_eq!(vec!["init", "batched", "batched2"], *v.borrow());
    }
}
