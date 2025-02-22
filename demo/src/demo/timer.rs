use std::cell::Cell;
use std::time::Duration;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen::JsCast as _;
use web_sys::MouseEvent;
use web_sys::window;

use super::show_counter;

#[autoclone]
#[template(tag = div)]
#[html]
pub fn timer_demo(#[signal] mut enable_timer: bool, counter: XSignal<i32>) -> XElement {
    let button = if enable_timer {
        let window = window().or_throw("window");
        let closure: Closure<dyn Fn()> = Closure::new(move || {
            autoclone!(counter);
            counter.update(|c| Some(*c + 1));
        });
        let handle = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                Duration::from_secs(1).as_millis() as i32,
            )
            .or_throw("set_interval");
        let enabled_state = Cell::new(Some((handle, closure)));
        button(
            click = move |_ev: MouseEvent| {
                let (handle, closure) = enabled_state.take().or_throw("enabled_state");
                window.clear_interval_with_handle(handle);
                drop(closure);
                enable_timer_mut.set(false);
            },
            "Stop",
        )
    } else {
        button(
            click = move |_ev: MouseEvent| {
                enable_timer_mut.set(true);
            },
            "Start",
        )
    };
    return div(h1("Timer"), show_counter(counter.clone()), button);
}
