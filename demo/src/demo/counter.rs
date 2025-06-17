use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::diagnostics::info;
use terrazzo::prelude::*;
use terrazzo::template;
use web_sys::MouseEvent;

use super::show_counter;

#[autoclone]
#[template(tag = div)]
#[html]
pub fn counter_demo(counter: XSignal<i32>) -> XElement {
    div(
        h1("Counter"),
        button(
            click = move |_ev: MouseEvent| {
                autoclone!(counter);
                counter.set(0);
            },
            "Clear",
        ),
        button(
            click = move |_ev: MouseEvent| {
                autoclone!(counter);
                counter.update(|c| Some(*c - 1));
            },
            "-1",
        ),
        button(
            click = move |_ev: MouseEvent| {
                autoclone!(counter);
                counter.update(|c| Some(*c + 1));
            },
            "+1",
        ),
        show_counter(counter.clone()),
        before_render = |_: Element| info!("Before render"),
        after_render = |_: Element| info!("After render"),
    )
}
