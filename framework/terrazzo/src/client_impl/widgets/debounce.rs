//! Utils to debounce function calls

use std::cell::Cell;
use std::time::Duration;

use scopeguard::guard;
use terrazzo_client::prelude::OrElseLog as _;
use terrazzo_client::prelude::Ptr;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::Closure;

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
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T);
    fn with_max_delay(self) -> impl DoDebounce;
    fn cancellable(self) -> Cancellable<Self> {
        Cancellable::of(self)
    }
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

    fn with_max_delay(self) -> impl DoDebounce {
        Debounce {
            delay: self.delay,
            max_delay: Some(self.delay),
        }
    }
}

impl DoDebounce for () {
    fn debounce<T: 'static>(self, f: impl Fn(T) + 'static) -> impl Fn(T) {
        f
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
