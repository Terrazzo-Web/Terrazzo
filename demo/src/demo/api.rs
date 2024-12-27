use std::cell::Cell;
use std::rc::Rc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::more_event::MoreEvent as _;
use web_sys::HtmlInputElement;

use crate::api;

#[autoclone]
#[template(tag = div)]
#[html]
pub fn api_demo() -> XElement {
    let result = XSignal::new("result", String::default());
    let a: Rc<Cell<String>> = Default::default();
    let b: Rc<Cell<String>> = Default::default();

    return div(
        h1("API call"),
        label(r#for = "a", "A="),
        input(
            r#type = "text",
            name = "a",
            change = move |ev: web_sys::Event| {
                autoclone!(a, b, result);
                let current_target: HtmlInputElement =
                    ev.current_target_element("'A'").or_throw("value of 'A'");
                a.set(current_target.value());
                compute(&a, &b, result.clone());
            },
        ),
        br(),
        label(r#for = "b", "B="),
        input(
            r#type = "text",
            name = "b",
            change = move |ev: web_sys::Event| {
                autoclone!(a, b, result);
                let current_target: HtmlInputElement =
                    ev.current_target_element("'B'").or_throw("value of 'B'");
                b.set(current_target.value());
                compute(&a, &b, result.clone());
            },
        ),
        br(),
        label(r#for = "result", "Result="),
        input(
            r#type = "text",
            name = "result",
            value %= move |t| {
                autoclone!(result);
                value(t, result.clone())
            },
        ),
    );

    #[template]
    fn value(#[signal] result: String) -> XAttributeValue {
        result
    }

    fn compute(a: &Cell<String>, b: &Cell<String>, result: XSignal<String>) {
        let (a, b) = {
            let a = scopeguard::guard(a.take(), |v| a.set(v));
            let b = scopeguard::guard(b.take(), |v| b.set(v));
            let Ok(a): Result<i32, _> = a.as_str().parse() else {
                result.set("'A' is not a number".to_owned());
                return;
            };
            let Ok(b): Result<i32, _> = b.as_str().parse() else {
                result.set("'B' is not a number".to_owned());
                return;
            };
            (a, b)
        };
        wasm_bindgen_futures::spawn_local(async move {
            match api::client::mult::mult(a, b).await {
                Ok(ok) => result.set(ok.to_string()),
                Err(error) => result.set(error.to_string()),
            }
        });
    }
}
