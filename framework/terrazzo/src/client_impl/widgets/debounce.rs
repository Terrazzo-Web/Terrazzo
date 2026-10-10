//! Utils to debounce function calls

use std::cell::Cell;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use autoclone::autoclone;
use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::Shared;
use pin_project::pin_project;
use scopeguard::guard;
use terrazzo_client::prelude::OrElseLog as _;
use terrazzo_client::prelude::Ptr;
use terrazzo_client::prelude::diagnostics::warn;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;
use web_sys::Performance;
use web_sys::Window;

use super::cancellable::Cancellable;

static WINDOW: LazyLock<Window> = LazyLock::new(|| web_sys::window().or_throw("window"));
static PERFORMANCE: LazyLock<Performance> =
    LazyLock::new(|| WINDOW.performance().or_throw("performance"));

/// Avoids executing a function too often.
/// Goal is to avoid flickering and improve UI performance.
///
/// ```ignore
/// let f = Duration::from_secs(1).debounce(|i| println!("{i}"));
/// f(1); // This won't show anything
/// // wait > 1 second ...
/// f(2); // Now this executes the callback and prints "2". "1" never gets printed.
/// ```
pub trait DoDebounce: Copy + 'static {
    fn debounce<T: 'static>(self, callback: impl Fn(T) + 'static) -> impl Fn(T);
    /// Debounces pending calls using their latest argument. Callbacks run serially;
    /// calls received during execution form the next batch, which starts no sooner
    /// than the configured delay after completion. Each batch shares its result.
    fn async_debounce<T, F, FR, R>(self, callback: F) -> impl Fn(T) -> Shared<BoxFuture<R>>
    where
        T: 'static,
        F: Fn(T) -> FR + 'static,
        FR: Future<Output = R> + 'static,
        R: Clone + Send + Sync + 'static;
    fn with_max_delay(self) -> impl DoDebounce;
    fn cancellable(self) -> Cancellable<Self> {
        Cancellable::of(self)
    }
}

type BoxFuture<R> = Pin<Box<dyn Future<Output = R> + Send + Sync>>;

/// Advanced usage for [DoDebounce].
#[derive(Clone, Copy)]
pub struct Debounce {
    /// The inactive delay before the callback gets executed.
    pub delay: Duration,

    /// The max delay before the callback gets executed.
    ///
    /// For example, if keystrokes are debounced, the callback isn't executed as long as the user keeps typing.
    /// The `max_delay` configures that the callback should get executed eventually, even in absence of inactivity period.
    pub max_delay: Option<Duration>,
}

impl DoDebounce for Duration {
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        Debounce {
            delay: self,
            max_delay: None,
        }
        .debounce(f)
    }

    fn async_debounce<T, F, FR, R>(self, callback: F) -> impl Fn(T) -> Shared<BoxFuture<R>>
    where
        T: 'static,
        F: Fn(T) -> FR + 'static,
        FR: Future<Output = R> + 'static,
        R: Clone + Send + Sync + 'static,
    {
        Debounce {
            delay: self,
            max_delay: None,
        }
        .async_debounce(callback)
    }

    fn with_max_delay(self) -> impl DoDebounce {
        Debounce {
            delay: self,
            max_delay: Some(self),
        }
    }
}

impl DoDebounce for Debounce {
    #[autoclone]
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        let state = Ptr::new(Cell::new(DebounceState::default()));
        let clear_timeout_on_drop = ClearDebounceOnDrop(state.clone());
        let max_delay_millis = self.max_delay.map(|d| d.as_secs_f64() * 1000.);
        let closure: Closure<dyn Fn()> = Closure::new(move || {
            autoclone!(state);
            let mut state = guard(state.take(), |new_state| state.set(new_state));
            f(state.scheduled_run.take().or_throw("scheduled_run").arg);
            state.last_run = PERFORMANCE.now();
        });
        move |arg| {
            let _ = &clear_timeout_on_drop;
            let now = PERFORMANCE.now();
            let mut state = guard(state.take(), |new_state| state.set(new_state));
            if let Some(max_delay_millis) = max_delay_millis
                && now - state.last_run  > max_delay_millis
                // If max delay is exceeded and there is already a task running, let it run.
                && let Some(scheduled_run) = &mut state.scheduled_run
            {
                scheduled_run.arg = arg;
                return;
            }

            if let Some(ScheduledRun { timeout_id, .. }) = state.scheduled_run {
                WINDOW.clear_timeout_with_handle(timeout_id);
            }
            let timeout_id = WINDOW
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    (self.delay.as_secs_f64() * 1000.) as i32,
                )
                .or_throw("set_timeout");
            state.scheduled_run = Some(ScheduledRun { timeout_id, arg });
        }
    }

    #[autoclone]
    fn async_debounce<T, F, FR, R>(self, user_callback: F) -> impl Fn(T) -> Shared<BoxFuture<R>>
    where
        T: 'static,
        F: Fn(T) -> FR + 'static,
        FR: Future<Output = R> + 'static,
        R: Clone + Send + Sync + 'static,
    {
        let async_state: Arc<Mutex<AsyncState<T, R>>> = Arc::default();
        let user_callback = Arc::new(user_callback);
        let debounced_callback = ThreadSafe(self.debounce(move |()| {
            autoclone!(async_state);
            async_state.lock().or_throw("async_state start").running = true;
            wasm_bindgen_futures::spawn_local(async move {
                autoclone!(async_state, user_callback);
                run_async_callbacks(
                    async_state,
                    |arg| user_callback(arg),
                    || async {
                        if let Err(error) = super::sleep::sleep(self.delay).await {
                            warn!("Failed to delay async callback: {error}");
                        }
                    },
                )
                .await;
            });
        }));
        move |arg| {
            autoclone!(async_state);
            let mut async_state = async_state.lock().or_throw("async_state enqueue");
            let future_result = async_state.enqueue(arg);
            if !async_state.running {
                debounced_callback(());
            }
            future_result
        }
    }

    fn with_max_delay(self) -> impl DoDebounce {
        Debounce {
            delay: self.delay,
            max_delay: Some(self.delay),
        }
    }
}

struct ClearDebounceOnDrop<T>(Ptr<Cell<DebounceState<T>>>);

impl<T> Drop for ClearDebounceOnDrop<T> {
    fn drop(&mut self) {
        if let Some(ScheduledRun { timeout_id, .. }) = self.0.take().scheduled_run {
            WINDOW.clear_timeout_with_handle(timeout_id);
        }
    }
}

struct AsyncState<T, R> {
    running: bool,
    pending: Option<AsyncPending<T, R>>,
}

impl<T, R> Default for AsyncState<T, R> {
    fn default() -> Self {
        Self {
            running: false,
            pending: None,
        }
    }
}

struct AsyncPending<T, R> {
    arg: T,
    tx: oneshot::Sender<R>,
    rx: Shared<BoxFuture<R>>,
}

impl<T, R: Clone + Send + Sync + 'static> AsyncState<T, R> {
    fn enqueue(&mut self, arg: T) -> Shared<BoxFuture<R>> {
        if let Some(pending) = &mut self.pending {
            pending.arg = arg;
            return pending.rx.clone();
        }
        let (tx, rx) = oneshot::channel();
        let rx: BoxFuture<R> = Box::pin(rx.map(|r| r.or_throw("Async debounce state canceled!")));
        let rx = rx.shared();
        self.pending = Some(AsyncPending {
            arg,
            tx,
            rx: rx.clone(),
        });
        rx
    }
}

async fn run_async_callbacks<T, R, F, FR, D, DR>(
    async_state: Arc<Mutex<AsyncState<T, R>>>,
    callback: F,
    delay: D,
) where
    F: Fn(T) -> FR,
    FR: Future<Output = R>,
    D: Fn() -> DR,
    DR: Future<Output = ()>,
{
    loop {
        let pending = async_state
            .lock()
            .or_throw("async_state start callback")
            .pending
            .take()
            .or_throw("async_state pending callback");
        let result = callback(pending.arg).await;
        if pending.tx.send(result).is_err() {
            warn!("Failed to send debounced async callback completion");
        }
        {
            let mut async_state = async_state.lock().or_throw("async_state completion");
            if async_state.pending.is_none() {
                async_state.running = false;
                return;
            }
        }
        // Keep the worker active during the delay so new calls only update the
        // next batch, rather than scheduling another worker.
        delay().await;
    }
}

impl DoDebounce for () {
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        f
    }

    fn async_debounce<T, F, FR, R>(self, callback: F) -> impl Fn(T) -> Shared<BoxFuture<R>>
    where
        T: 'static,
        F: Fn(T) -> FR + 'static,
        FR: Future<Output = R> + 'static,
        R: Clone + Send + Sync + 'static,
    {
        move |a| {
            let result: BoxFuture<R> = Box::pin(ThreadSafe(callback(a)));
            return result.shared();
        }
    }

    fn with_max_delay(self) -> impl DoDebounce {
        self
    }
}

struct DebounceState<T> {
    scheduled_run: Option<ScheduledRun<T>>,
    last_run: f64,
}

struct ScheduledRun<T> {
    timeout_id: i32,
    arg: T,
}

impl<T> Default for DebounceState<T> {
    fn default() -> Self {
        Self {
            scheduled_run: None,
            last_run: 0.,
        }
    }
}

impl<T> std::fmt::Debug for DebounceState<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DebounceState")
            .field("scheduled_run", &self.scheduled_run.is_some())
            .field("last_run", &self.last_run)
            .finish()
    }
}

#[pin_project]
struct ThreadSafe<T>(#[pin] T);

/// Safe because Javascript is single-threaded.
unsafe impl<T> Send for ThreadSafe<T> {}

/// Safe because Javascript is single-threaded.
unsafe impl<T> Sync for ThreadSafe<T> {}

impl<T> std::ops::Deref for ThreadSafe<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<F: Future> Future for ThreadSafe<F> {
    type Output = F::Output;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn async_batches_wait_for_completion_and_keep_latest_argument() {
        let state: Arc<Mutex<AsyncState<i32, i32>>> = Arc::default();
        let enqueue = |arg| state.lock().unwrap().enqueue(arg);
        let first = enqueue(1);
        let first_coalesced = enqueue(2);
        state.lock().unwrap().running = true;

        let seen = Rc::new(RefCell::new(Vec::new()));
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        let replies = RefCell::new(VecDeque::from([first_rx, second_rx]));
        let (delay_tx, delay_rx) = oneshot::channel();
        let delay_rx = RefCell::new(Some(delay_rx));
        let mut worker = Box::pin(run_async_callbacks(
            state.clone(),
            |arg| {
                seen.borrow_mut().push(arg);
                let reply = replies.borrow_mut().pop_front().unwrap();
                async move { reply.await.unwrap() }
            },
            || {
                let delay_rx = delay_rx.borrow_mut().take().unwrap();
                async move { delay_rx.await.unwrap() }
            },
        ));

        assert!(worker.as_mut().now_or_never().is_none());
        assert_eq!(&*seen.borrow(), &[2]);
        let second = enqueue(3);
        let second_coalesced = enqueue(4);
        assert!(worker.as_mut().now_or_never().is_none());
        assert_eq!(&*seen.borrow(), &[2]);
        assert!(second.clone().now_or_never().is_none());

        first_tx.send(20).unwrap();
        assert!(worker.as_mut().now_or_never().is_none());
        assert_eq!(first.now_or_never(), Some(20));
        assert_eq!(first_coalesced.now_or_never(), Some(20));
        assert_eq!(&*seen.borrow(), &[2]);
        assert!(state.lock().unwrap().running);

        // Calls during the completion delay also belong to the next batch.
        let during_delay = enqueue(5);
        delay_tx.send(()).unwrap();
        assert!(worker.as_mut().now_or_never().is_none());
        assert_eq!(&*seen.borrow(), &[2, 5]);
        assert!(second.clone().now_or_never().is_none());

        second_tx.send(50).unwrap();
        assert_eq!(worker.as_mut().now_or_never(), Some(()));
        assert_eq!(second.now_or_never(), Some(50));
        assert_eq!(second_coalesced.now_or_never(), Some(50));
        assert_eq!(during_delay.now_or_never(), Some(50));
        assert!(!state.lock().unwrap().running);
        assert!(state.lock().unwrap().pending.is_none());

        // A later call starts a fresh batch instead of reusing a completed result.
        assert!(enqueue(6).now_or_never().is_none());
        assert_eq!(state.lock().unwrap().pending.as_ref().unwrap().arg, 6);
    }
}
