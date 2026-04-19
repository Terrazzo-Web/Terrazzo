#![cfg(feature = "client")]

use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use futures::StreamExt as _;
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
use super::api::Conversion;
use super::api::Conversions;
use super::content_state;
use crate::converter::api::Language;
use crate::converter::conversion_tabs::ConversionsState;
use crate::frontend::menu::menu;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::frontend::remotes::Remote;
use crate::frontend::remotes_ui::show_remote;
use crate::utils::ndjson::NdjsonBuffer;

stylance::import_style!(pub(super) style, "converter.scss");

/// The UI for the converter app.
#[html]
#[template]
pub fn converter(remote: XSignal<Remote>) -> XElement {
    let conversions = XSignal::new("conversions", Conversions::default());
    let preferred_language = XSignal::new("preferred-language", None);
    let active_request_id = Arc::new(AtomicU64::new(0));
    div(
        class = style::outer,
        converter_impl(remote, conversions, preferred_language, active_request_id),
    )
}

#[html]
#[template(tag = div)]
fn converter_impl(
    remote: XSignal<Remote>,
    conversions: XSignal<Conversions>,
    preferred_language: XSignal<Option<Language>>,
    active_request_id: Arc<AtomicU64>,
) -> XElement {
    div(
        class = style::inner,
        key = "converter",
        div(class = style::header, menu(), show_remote(remote.clone())),
        div(
            class = style::body,
            show_input(remote, conversions.clone(), active_request_id),
            show_resize_bar(),
            show_conversions(conversions, preferred_language),
        ),
    )
}

#[autoclone]
#[html]
#[template(tag = textarea)]
fn show_input(
    #[signal] remote: Remote,
    conversions: XSignal<Conversions>,
    active_request_id: Arc<AtomicU64>,
) -> XElement {
    let element = ElementCapture::<HtmlTextAreaElement>::default();
    tag(
        class = "converter-input",
        style::flex %= width(RESIZE_MANAGER.delta.clone()),
        before_render = element.capture(),
        input = move |_: web_sys::InputEvent| {
            autoclone!(remote, element, conversions, active_request_id);
            let value: Arc<str> = element.with(|e| e.value().into());
            spawn_local(async move {
                autoclone!(remote, value);
                let _set = content_state::set(remote.clone(), value.clone()).await;
            });
            get_conversions(
                remote.clone(),
                value,
                conversions.clone(),
                active_request_id.clone(),
            );
        },
        after_render = move |_| {
            autoclone!(remote, element, conversions, active_request_id);
            spawn_local(async move {
                autoclone!(remote, element, conversions, active_request_id);
                let Ok(content) = content_state::get(remote.clone()).await else {
                    warn!("Failed to load converter content");
                    return;
                };
                element.with(|e| e.set_value(&content));
                get_conversions(
                    remote.clone(),
                    content,
                    conversions.clone(),
                    active_request_id.clone(),
                );
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
        class = style::conversions,
        tabs(
            conversions,
            state,
            Ptr::new(TabsOptions {
                tabs_class: Some(style::tabs.into()),
                titles_class: Some(style::titles.into()),
                title_class: Some(style::title.into()),
                items_class: Some(style::items.into()),
                item_class: Some(style::item.into()),
                selected_class: Some(style::selected.into()),
                ..TabsOptions::default()
            }),
        ),
    )
}

fn get_conversions(
    remote: Remote,
    content: Arc<str>,
    conversions: XSignal<Conversions>,
    active_request_id: Arc<AtomicU64>,
) {
    let debounced = get_conversions_debounced();
    debounced(GetConversionsUiRequest {
        remote,
        content,
        conversions,
        active_request_id,
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
        active_request_id,
    }: GetConversionsUiRequest,
) {
    spawn_local(async move {
        let request_id = active_request_id.fetch_add(1, Ordering::SeqCst) + 1;
        conversions_mut.force(Conversions::default());
        let stream = super::api::get_conversions(remote, content).await;
        match stream {
            Ok(stream) => consume_conversions_stream(
                stream,
                conversions_mut,
                active_request_id,
                request_id,
            )
            .await,
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
    active_request_id: Arc<AtomicU64>,
}

struct DebouncedGetConversions(Box<dyn Fn(GetConversionsUiRequest)>);
unsafe impl Send for DebouncedGetConversions {}
unsafe impl Sync for DebouncedGetConversions {}

async fn consume_conversions_stream(
    stream: server_fn::codec::TextStream<server_fn::ServerFnError>,
    conversions: XSignal<Conversions>,
    active_request_id: Arc<AtomicU64>,
    request_id: u64,
) {
    let mut parser = NdjsonBuffer::<Conversion>::default();
    let mut stream = stream.into_inner();
    while let Some(chunk) = stream.next().await {
        if active_request_id.load(Ordering::SeqCst) != request_id {
            return;
        }
        match chunk {
            Ok(chunk) => {
                for conversion in parser.push_chunk(&chunk) {
                    match conversion {
                        Ok(conversion) => {
                            conversions.update(|current| {
                                let mut next = current.conversions.as_ref().clone();
                                next.push(conversion);
                                Some(Conversions {
                                    conversions: Arc::new(next),
                                })
                            });
                        }
                        Err(error) => warn!("Failed to parse conversion stream line: {error}"),
                    }
                }
            }
            Err(error) => {
                warn!("Conversion stream failed: {error}");
                if active_request_id.load(Ordering::SeqCst) == request_id {
                    conversions.force(Conversions::default());
                }
                return;
            }
        }
    }
}

#[html]
fn show_resize_bar() -> XElement {
    div(
        class = style::resize_bar,
        mousedown = RESIZE_MANAGER.mousedown(),
        dblclick = |_| RESIZE_MANAGER.delta.set(None),
        div(div()),
    )
}

static RESIZE_MANAGER: LazyLock<MousemoveManager> = LazyLock::new(MousemoveManager::new);
