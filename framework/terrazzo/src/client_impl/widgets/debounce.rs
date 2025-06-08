//! Utils to debounce function calls

use std::cell::Cell;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use autoclone::autoclone;
use futures::channel::oneshot;
use scopeguard::guard;
use terrazzo_client::prelude::OrElseLog as _;
use terrazzo_client::prelude::Ptr;
use tracing::debug;
use tracing::warn;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;

use self::helpers::BoxFuture;
use self::helpers::IsThreadSafe;
use super::cancellable::Cancellable;

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
    fn async_debounce<T, F, R>(self, callback: F) -> impl Fn(T) -> BoxFuture + Send + Sync
    where
        T: IsThreadSafe,
        F: Fn(T) -> R + IsThreadSafe,
        R: Future<Output = ()> + IsThreadSafe;
    fn with_max_delay(self) -> impl DoDebounce;
    fn cancellable(self) -> Cancellable<Self> {
        Cancellable::of(self)
    }
}

mod helpers {
    use std::pin::Pin;

    pub trait IsThreadSafe: Send + Sync + 'static {}
    impl<T: Send + Sync + 'static> IsThreadSafe for T {}

    pub type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;
}

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

    fn async_debounce<T, F, R>(self, callback: F) -> impl Fn(T) -> BoxFuture + Send + Sync
    where
        T: IsThreadSafe,
        F: Fn(T) -> R + IsThreadSafe,
        R: Future<Output = ()> + IsThreadSafe,
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
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        let window = web_sys::window().or_throw("window");
        let performance = window.performance().or_throw("performance");
        let state = Ptr::new(Cell::new(DebounceState::default()));
        let delay_millis = self.delay.as_secs_f64() * 1000.;
        let max_delay_millis = self.max_delay.map(|d| d.as_secs_f64() * 1000.);
        let closure: Closure<dyn Fn()> = Closure::new({
            let state = state.clone();
            let performance = performance.clone();
            move || {
                let mut state = guard(state.take(), |new_state| state.set(new_state));
                f(state.scheduled_run.take().or_throw("scheduled_run").arg);
                state.last_run = performance.now();
            }
        });
        move |arg| {
            let now = performance.now();
            let mut state = guard(state.take(), |new_state| state.set(new_state));
            if let Some(max_delay_millis) = max_delay_millis {
                if now - state.last_run - delay_millis > max_delay_millis {
                    // If max delay is exceeded and there is already a task running, let it run.
                    if let Some(scheduled_run) = &mut state.scheduled_run {
                        scheduled_run.arg = arg;
                        return;
                    }
                }
            }

            if let Some(ScheduledRun { timeout_id, .. }) = state.scheduled_run {
                window.clear_timeout_with_handle(timeout_id);
            }
            let timeout_id = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    (self.delay.as_secs_f64() * 1000.) as i32,
                )
                .or_throw("set_timeout");
            state.scheduled_run = Some(ScheduledRun { timeout_id, arg });
        }
    }

    #[autoclone]
    fn async_debounce<T, F, R>(self, callback: F) -> impl Fn(T) -> BoxFuture + Send + Sync
    where
        T: IsThreadSafe,
        F: Fn(T) -> R + IsThreadSafe,
        R: Future<Output = ()> + IsThreadSafe,
    {
        let async_state: Arc<Mutex<AsyncState<T>>> = Arc::default();
        let async_result: Arc<Mutex<Option<oneshot::Receiver<()>>>> = Arc::default();

        // The callback is debounced.
        // When the callback is actually executed, the result is populated.
        let callback = ThreadSafeCallback(Arc::new(self.debounce(move |a| {
            autoclone!(async_result);
            let (tx, rx) = oneshot::channel();
            {
                let mut async_result = async_result.lock().or_throw("async_result 1");
                if async_result.is_some() {
                    warn!("The debounced async result was not awaited");
                }
                *async_result = Some(rx);
            }
            debug!("Spawning the async debounced callback");
            let result: BoxFuture = Box::pin(callback(a));
            wasm_bindgen_futures::spawn_local(complete_async_debounce(tx, result));
        })));
        return make_async_debounced(async_state, async_result, callback);
    }

    fn with_max_delay(self) -> impl DoDebounce {
        Debounce {
            delay: self.delay,
            max_delay: Some(self.delay),
        }
    }
}

#[autoclone]
fn make_async_debounced<T: IsThreadSafe>(
    async_state: Arc<Mutex<AsyncState<T>>>,
    async_result: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    callback: ThreadSafeCallback<Arc<impl Fn(T) + 'static>>,
) -> impl Fn(T) -> BoxFuture + Send + Sync {
    move |arg| {
        Box::pin(async move {
            autoclone!(async_state, async_result, callback);
            {
                let mut async_state = async_state.lock().or_throw("async_state 1");
                match &*async_state {
                    AsyncState::NotRunning => {
                        debug!("At call time: Not running");
                        *async_state = AsyncState::Running
                    }
                    AsyncState::Running => {
                        debug!("At call time: Running");
                        *async_state = AsyncState::CallAgain(arg);
                        return;
                    }
                    AsyncState::CallAgain { .. } => {
                        debug!("At call time: Call again");
                        *async_state = AsyncState::CallAgain(arg);
                        return;
                    }
                }
            }

            let mut arg = arg;
            loop {
                debug!("Calling the sync debounced callback");
                callback(arg);
                {
                    let async_result = async_result.lock().or_throw("async_result 2").take();
                    if let Some(async_result) = async_result {
                        debug!("The debounced async callback was executed");
                        let () = async_result.await.unwrap_or_else(|_| {
                            warn!("The debounced async callback was dropped");
                        });
                    } else {
                        debug!(
                            "The debounced async callback was not executed: AsyncState={:?}",
                            async_state.lock().or_throw("async_state 2")
                        );
                    }
                }

                {
                    let mut lock = async_state.lock().or_throw("async_state 3");
                    match std::mem::take(&mut *lock) {
                        AsyncState::NotRunning => {
                            debug!("Impossible state");
                            panic!("Impossible state");
                        }
                        AsyncState::Running => {
                            debug!("At return time: Running => NotRunning");
                            *lock = AsyncState::NotRunning;
                            return;
                        }
                        AsyncState::CallAgain(arg_again) => {
                            debug!("At return time: CallAgain => Running and calling it again");
                            *lock = AsyncState::Running;
                            arg = arg_again;
                        }
                    }
                }
            }
        })
    }
}

async fn complete_async_debounce(tx: oneshot::Sender<()>, result: BoxFuture) {
    let () = result.await;
    let Ok(()) = tx.send(()) else {
        warn!("Failed to send debounced async callback completion");
        return;
    };
}

#[derive(Default)]
enum AsyncState<T> {
    #[default]
    NotRunning,
    Running,
    CallAgain(T),
}

impl<T> std::fmt::Debug for AsyncState<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "NotRunning"),
            Self::Running => write!(f, "Running"),
            Self::CallAgain { .. } => write!(f, "CallAgain"),
        }
    }
}

impl DoDebounce for () {
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        f
    }

    fn async_debounce<T, F, R>(self, callback: F) -> impl Fn(T) -> BoxFuture + Send + Sync
    where
        T: IsThreadSafe,
        F: Fn(T) -> R + IsThreadSafe,
        R: Future<Output = ()> + IsThreadSafe,
    {
        move |a| Box::pin(callback(a))
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

struct ThreadSafeCallback<T>(T);

/// Safe because Javascript is single-threaded.
unsafe impl<T> Send for ThreadSafeCallback<T> {}

/// Safe because Javascript is single-threaded.
unsafe impl<T> Sync for ThreadSafeCallback<T> {}

impl<T> std::ops::Deref for ThreadSafeCallback<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
