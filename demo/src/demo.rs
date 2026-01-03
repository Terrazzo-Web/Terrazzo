#![cfg(feature = "client")]

use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;

mod api;
mod attributes;
mod counter;
mod timer;

#[template]
#[html]
pub fn run() -> XElement {
    let demo = Demo::new();
    return div(
        counter::counter_demo(demo.counter),
        timer::timer_demo(demo.enable_timer, demo.timer),
        api::api_demo(),
        attributes::attributes_demo(),
    );
}

#[template(tag = span)]
#[html]
fn show_counter(#[signal] c: i32) -> XElement {
    tag("Value: ", "{c}", "!")
}

#[derive(Clone)]
struct Demo {
    counter: XSignal<i32>,
    enable_timer: XSignal<bool>,
    timer: XSignal<i32>,
}

impl Demo {
    fn new() -> Self {
        Self {
            counter: XSignal::new("counter", 0),
            enable_timer: XSignal::new("enable_timer", false),
            timer: XSignal::new("timer", 0),
        }
    }
}
