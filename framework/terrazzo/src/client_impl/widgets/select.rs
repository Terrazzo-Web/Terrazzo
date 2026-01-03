use std::collections::HashMap;

use autoclone::autoclone;
use autoclone::envelope;
use terrazzo_client::prelude::*;
use terrazzo_macro::html;
use terrazzo_macro::template;
use web_sys::HtmlSelectElement;

use crate::widgets::element_capture::ElementCapture;

#[envelope]
pub struct Select<O: Option> {
    pub select: ElementCapture<HtmlSelectElement>,
    options: Vec<O>,
    options_map: HashMap<XString, O>,
    pub selected: XSignal<O>,
}

impl<O: Option> SelectPtr<O> {
    pub fn new(options: Vec<O>, selected: std::option::Option<O>) -> Self {
        let selected = XSignal::new("selected", selected.unwrap_or_else(|| options[0].clone()));
        let options_map = options
            .iter()
            .map(|option| (option.name(), option.clone()))
            .collect();
        Select {
            select: ElementCapture::default(),
            options,
            options_map,
            selected,
        }
        .into()
    }
}

impl<O: Option> SelectPtr<O> {
    pub fn show(&self) -> XElement {
        show_select(self)
    }
}

pub trait Option: Clone + std::fmt::Debug + Eq + 'static {
    fn show(&self) -> XElement;
    fn name(&self) -> XString;
}

#[autoclone]
#[html]
fn show_select<O: Option>(this: &SelectPtr<O>) -> XElement {
    let options_show = this
        .options
        .iter()
        .map(|option| show_option(option.clone(), this.selected.clone()))
        .collect::<Vec<_>>();
    select(
        before_render = this.select.capture(),
        options_show..,
        change = move |_| {
            autoclone!(this);
            onchange(&this)
        },
    )
}

pub fn onchange<O: Option>(select: &Select<O>) {
    let html = select.select.get();
    let new_selected = html.value();
    let new_selected = select
        .options_map
        .get(new_selected.as_str())
        .or_else_throw(|_| "Option not found{ {new_selected}");
    select.selected.set(new_selected.clone());
}

#[html]
#[template(tag = option)]
fn show_option<O: Option>(option: O, #[signal] selected: O) -> XElement {
    tag(
        value = option.name(),
        selected = (option == selected).then_some("true"),
        option.show(),
    )
}
