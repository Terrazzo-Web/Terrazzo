
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
use crate::tiles::signals::TilePtr;

terrazzo_css::import_style!(pub(super) style, "converter.scss");

/// The UI for the converter app.
#[html]
#[template(tag = div)]
pub fn converter(tile: TilePtr) -> XElement {
    let conversions = XSignal::new("conversions", Conversions::default());
    let preferred_language = XSignal::new("preferred-language", None);
    let resize_manager = MousemoveManager::new();
    tag(
        class = style::OUTER,
        converter_impl(
            tile.clone(),
            conversions,
            preferred_language,
            resize_manager,
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
            show_input(
                tile.clone(),
                tile.remote.clone(),
                conversions.clone(),
                resize_manager.clone(),
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
) -> XElement {
    let element = ElementCapture::<HtmlTextAreaElement>::default();
    tag(
        #[cfg(not(feature = "client-prod"))]
        class = "converter-input",
        style::flex %= width(resize_manager.delta.clone()),
        before_render = element.capture(),
        input = move |_: web_sys::InputEvent| {
            autoclone!(tile, remote, element, conversions);
            let value: Arc<str> = element.with(|e| e.value().into());
            spawn_local(async move {
                autoclone!(tile, remote, value);
                let _set = content_state::set(tile.id.into(), remote.clone(), value.clone()).await;
            });
            get_conversions(remote.clone(), value, conversions.clone());
        },
        after_render = move |_| {
            spawn_local(async move {
                autoclone!(tile, remote, element, conversions);
                let Ok(content) = content_state::get(tile.id.into(), remote.clone()).await else {
                    warn!("Failed to load converter content");
                    return;
                };
                if element.try_with(|e| e.set_value(&content)).is_some() {
                    get_conversions(remote.clone(), content, conversions.clone());
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

fn get_conversions(remote: Remote, content: Arc<str>, conversions: XSignal<Conversions>) {
    let debounced = get_conversions_debounced();
    debounced(GetConversionsUiRequest {
        remote,
        content,
        conversions,
    })
}

fn get_conversions_debounced() -> &'static dyn Fn(GetConversionsUiRequest) {
    use std::sync::OnceLock;
    static DEBOUNCED: OnceLock<DebouncedGetConversions> = OnceLock::new();
    let debounced = DEBOUNCED.get_or_init(|| {
        DebouncedGetConversions(Box::new(DEBOUNCE_DELAY.debounce(spawn_conversions_request)))
    });
    &*debounced.0
}

fn spawn_conversions_request(
    GetConversionsUiRequest {
        remote,
        content,
        conversions: conversions_mut,
    }: GetConversionsUiRequest,
) {
    spawn_local(async move {
        let conversions = super::api::get_conversions(remote, content).await;
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
}

struct DebouncedGetConversions(Box<dyn Fn(GetConversionsUiRequest)>);
unsafe impl Send for DebouncedGetConversions {}
unsafe impl Sync for DebouncedGetConversions {}
