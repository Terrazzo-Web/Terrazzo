#![cfg(feature = "client")]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::debounce::DoDebounce;
use terrazzo::widgets::element_capture::ElementCapture;
use terrazzo::widgets::tabs::TabsOptions;
use terrazzo::widgets::tabs::tabs;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlTextAreaElement;

use self::diagnostics::warn;
use super::api::Conversions;
use super::content_state;
use crate::converter::api::Language;
use crate::converter::conversion_tabs::ConversionsState;
use crate::frontend::menu::menu;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::frontend::remotes::Remote;
use crate::frontend::remotes_ui::show_remote;
use crate::frontend::resize_bar::resize_bar_horz;
use crate::tiles::APP_COLLAPSIBLE_CONTENT;
use crate::tiles::signals::TilePtr;

terrazzo_css::import_style!(pub(super) style, "converter.scss");

/// The UI for the converter app.
#[html]
#[template(tag = div)]
pub fn converter(tile: TilePtr) -> XElement {
    let conversions = XSignal::new("conversions", Conversions::default());
    let preferred_language = XSignal::new("preferred-language", None);
    let resize_manager = MousemoveManager::new();
    let conversions_requester = ConversionsRequester::new();
    tag(
        class = style::OUTER,
        converter_impl(
            tile.clone(),
            conversions,
            preferred_language,
            resize_manager,
            conversions_requester,
        ),
    )
}

#[html]
#[template(tag = div)]
fn converter_impl(
    tile: TilePtr,
    conversions: XSignal<Conversions>,
    preferred_language: XSignal<Option<Language>>,
    resize_manager: MousemoveManager,
    conversions_requester: ConversionsRequester,
) -> XElement {
    div(
        class = style::INNER,
        key = "converter",
        div(
            class = style::HEADER,
            menu(tile.clone()),
            show_remote(tile.remote.clone()),
        ),
        div(
            class = style::BODY,
            class = APP_COLLAPSIBLE_CONTENT,
            show_input(
                tile.clone(),
                tile.remote.clone(),
                conversions.clone(),
                resize_manager.clone(),
                conversions_requester,
            ),
            resize_bar_horz(resize_manager, Default::default()),
            show_conversions(conversions, preferred_language),
        ),
    )
}

#[autoclone]
#[html]
#[template(tag = textarea)]
fn show_input(
    tile: TilePtr,
    #[signal] remote: Remote,
    conversions: XSignal<Conversions>,
    resize_manager: MousemoveManager,
    conversions_requester: ConversionsRequester,
) -> XElement {
    let element = ElementCapture::<HtmlTextAreaElement>::default();
    let input_since_render = Rc::new(Cell::new(false));
    tag(
        #[cfg(not(feature = "client-prod"))]
        class = "converter-input",
        style::flex %= width(resize_manager.delta.clone()),
        before_render = element.capture(),
        input = move |_: web_sys::InputEvent| {
            autoclone!(
                tile,
                remote,
                element,
                conversions,
                input_since_render,
                conversions_requester
            );
            input_since_render.set(true);
            let value: Arc<str> = element.with(|e| e.value().into());
            spawn_local(async move {
                autoclone!(tile, remote, value);
                let _set = content_state::set(tile.id.into(), remote.clone(), value.clone()).await;
            });
            conversions_requester.request(remote.clone(), value, conversions.clone());
        },
        after_render = move |_| {
            spawn_local(async move {
                autoclone!(
                    tile,
                    remote,
                    element,
                    conversions,
                    input_since_render,
                    conversions_requester
                );
                let Ok(content) = content_state::get(tile.id.into(), remote.clone()).await else {
                    warn!("Failed to load converter content");
                    return;
                };
                if input_since_render.get() {
                    return;
                }
                if element.try_with(|e| e.set_value(&content)).is_some() {
                    conversions_requester.request(remote.clone(), content, conversions.clone());
                }
            });
        },
    )
}

#[template(wrap = true)]
fn width(#[signal] mut position: Option<Position>) -> XAttributeValue {
    position.map(|position| format!("0 0 calc(50% + {}px)", position.x))
}

#[html]
#[template(tag = div)]
fn show_conversions(
    #[signal] conversions: Conversions,
    preferred_language: XSignal<Option<Language>>,
) -> XElement {
    let state = ConversionsState::new(&conversions, preferred_language);
    div(
        class = style::CONVERSIONS,
        tabs(
            conversions,
            state,
            Ptr::new(TabsOptions {
                tabs_class: Some(style::TABS.into()),
                titles_class: Some(style::TITLES.into()),
                title_class: Some(style::TITLE.into()),
                items_class: Some(style::ITEMS.into()),
                item_class: Some(style::ITEM.into()),
                selected_class: Some(style::SELECTED.into()),
                ..TabsOptions::default()
            }),
        ),
    )
}

fn spawn_conversions_request(
    GetConversionsUiRequest {
        remote,
        content,
        conversions: conversions_mut,
        generation,
        latest_generation,
    }: GetConversionsUiRequest,
) {
    spawn_local(async move {
        let conversions = super::api::get_conversions(remote, content).await;
        if latest_generation.get() != generation {
            return;
        }
        match conversions {
            Ok(conversions) => conversions_mut.force(conversions),
            Err(error) => {
                warn!("Failed to get conversions: {error}");
                conversions_mut.force(Conversions::default())
            }
        }
    })
}

static DEBOUNCE_DELAY: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(700)
} else {
    Duration::from_millis(200)
};

struct GetConversionsUiRequest {
    remote: Remote,
    content: Arc<str>,
    conversions: XSignal<Conversions>,
    generation: u64,
    latest_generation: Rc<Cell<u64>>,
}

#[derive(Clone)]
struct ConversionsRequester {
    latest_generation: Rc<Cell<u64>>,
    debounced: Rc<dyn Fn(GetConversionsUiRequest)>,
}

impl ConversionsRequester {
    fn new() -> Self {
        Self {
            latest_generation: Rc::new(Cell::new(0)),
            debounced: Rc::new(DEBOUNCE_DELAY.debounce(spawn_conversions_request)),
        }
    }

    fn request(&self, remote: Remote, content: Arc<str>, conversions: XSignal<Conversions>) {
        let generation = self.latest_generation.get().wrapping_add(1);
        self.latest_generation.set(generation);
        (self.debounced)(GetConversionsUiRequest {
            remote,
            content,
            conversions,
            generation,
            latest_generation: self.latest_generation.clone(),
        });
    }
}
