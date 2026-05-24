//! Utils to debounce function calls

use std::cell::Cell;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use autoclone::autoclone;
use futures::FutureExt;
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
        let max_delay_millis = self.max_delay.map(|d| d.as_secs_f64() * 1000.);
        let closure: Closure<dyn Fn()> = Closure::new(move || {
            autoclone!(state);
            let mut state = guard(state.take(), |new_state| state.set(new_state));
            f(state.scheduled_run.take().or_throw("scheduled_run").arg);
            state.last_run = PERFORMANCE.now();
        });
        move |arg| {
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
        let async_state: Arc<Mutex<AsyncState<_>>> = Arc::default();
        let user_callback = Arc::new(user_callback);
        let debounced_callback = ThreadSafe(self.debounce(move |a| {
            autoclone!(async_state);
            wasm_bindgen_futures::spawn_local(async move {
                autoclone!(async_state);
                autoclone!(user_callback);
                let result = user_callback(a).await;
                let mut async_state = async_state.lock().or_throw("async_state lock 1");
                let AsyncState::Running { tx, rx: _ } = std::mem::take(&mut *async_state) else {
                    warn!("Expected the async debounce callback to be running");
                    panic!("Expected the async debounce callback to be running");
                };
                let Ok(()) = tx.send(result) else {
                    warn!("Failed to send debounced async callback completion");
                    return;
                };
            });
        }));
        move |a| {
            autoclone!(async_state);
            let mut async_state = async_state.lock().or_throw("async_state 1");
            let future_result = match &mut *async_state {
                AsyncState::NotRunning => {
                    let (tx, rx) = oneshot::channel();
                    let future_result: BoxFuture<R> =
                        Box::pin(rx.map(|r| r.or_throw("Async debounce state canceled!")));
                    let future_result = future_result.shared();
                    *async_state = AsyncState::Running {
                        tx,
                        rx: future_result.clone(),
                    };
                    future_result
                }
                AsyncState::Running { tx: _, rx } => rx.clone(),
            };
            debounced_callback(a);
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

#[derive(Default)]
enum AsyncState<R> {
    #[default]
    NotRunning,
    Running {
        tx: oneshot::Sender<R>,
        rx: Shared<BoxFuture<R>>,
    },
}

impl<R> std::fmt::Debug for AsyncState<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "NotRunning"),
            Self::Running { .. } => write!(f, "Running"),
        }
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
