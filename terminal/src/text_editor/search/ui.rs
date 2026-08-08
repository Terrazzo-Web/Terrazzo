use std::future::ready;
use std::ops::Not;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::Stream;
use futures::StreamExt;
use futures::channel::oneshot;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::debounce::DoDebounce as _;
use terrazzo::widgets::element_capture::ElementCapture;
use wasm_bindgen_futures::spawn_local;
use web_sys::FocusEvent;
use web_sys::HtmlInputElement;
use web_sys::KeyboardEvent;

use super::state::EditorSearchState;
use crate::api::client_address::ClientAddress;
use crate::assets::icons;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::manager::EditorState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::style;

impl TextEditorManager {
    #[autoclone]
    #[html]
    pub fn search_selector(self: &Ptr<Self>) -> XElement {
        let is_active = XSignal::new("is-search-active", false);
        let input: ElementCapture<HtmlInputElement> = ElementCapture::default();

        return div(
            class = style::PATH_SELECTOR,
            style::flex_basis %= flex_basis(is_active.clone()),
            img(
                class = style::PATH_SELECTOR_ICON,
                class = style::SEARCH_ICON,
                src = icons::search(),
                click = move |_| {
                    autoclone!(is_active, input);
                    is_active.set(true);
                    let () = input.with(|i| i.focus()).or_throw("focus");
                },
            ),
            search_selector_input(self.clone(), input, self.path.base.clone(), is_active),
        );

        #[template(wrap = true)]
        pub fn flex_basis(#[signal] is_active: bool) -> XAttributeValue {
            is_active.not().then_some("0")
        }
    }
}

#[autoclone]
#[html]
#[template(tag = div)]
fn search_selector_input(
    manager: Ptr<TextEditorManager>,
    input: ElementCapture<HtmlInputElement>,
    #[signal] base: Arc<Path>,
    #[signal] mut is_active: bool,
) -> XElement {
    if !is_active {
        return tag(style::display = "none", style::visibility = "hidden");
    }
    let do_search = Ptr::new(do_search(manager.clone(), base, input.clone()));
    let editor_state = manager.editor_state.clone();
    tag(
        class = style::PATH_SELECTOR_WIDGET,
        class = style::PATH_SELECTOR_INPUT,
        key = "search",
        input(
            before_render = input.capture(),
            r#type = "text",
            class = style::PATH_SELECTOR_FIELD,
            keydown = move |event: KeyboardEvent| {
                autoclone!(editor_state, is_active_mut, input, do_search);
                if event.key() == "Escape" {
                    event.prevent_default();
                    close_search(&editor_state, &is_active_mut);
                    let () = input.with(|i| i.blur()).or_throw("blur");
                    return;
                }
                do_search()
            },
            blur = move |_: FocusEvent| {
                autoclone!(editor_state);
                close_search(&editor_state, &is_active_mut);
            },
            focus = move |_: FocusEvent| start_search(&editor_state, &do_search),
        ),
    )
}

fn start_search(editor_state: &XSignal<EditorState>, do_search: &Ptr<impl Fn()>) {
    editor_state.update(|editor_state| {
        if let EditorState::Search { .. } = editor_state {
            return None;
        }
        Some(EditorState::Search(EditorSearchState {
            prev: Box::new(editor_state.clone()),
            results: Default::default(),
        }))
    });
    do_search()
}

fn close_search(editor_state: &XSignal<EditorState>, is_active_mut: &MutableSignal<bool>) {
    let batch = Batch::use_batch("close-search");
    editor_state.update(|editor_state| {
        let EditorState::Search(EditorSearchState { prev, .. }) = editor_state else {
            return None;
        };
        Some(prev.as_ref().clone())
    });
    is_active_mut.set(false);
    drop(batch);
}

fn do_search(
    manager: Ptr<TextEditorManager>,
    base: Arc<Path>,
    input: ElementCapture<HtmlInputElement>,
) -> impl Fn() {
    let cancel_last = Mutex::new(oneshot::channel().0);
    let callback = Duration::from_millis(250)
        .with_max_delay()
        .async_debounce(move |cancel_rx| {
            do_search_impl(manager.clone(), base.clone(), input.clone(), cancel_rx)
        });
    move || {
        let cancel_rx = {
            let mut lock = cancel_last.lock().unwrap();
            let cancel_new = oneshot::channel();
            let cancel_last = std::mem::replace(&mut *lock, cancel_new.0);
            let _ = cancel_last.send(());
            cancel_new.1
        };
        spawn_local(callback(cancel_rx))
    }
}

async fn do_search_impl(
    manager: Ptr<TextEditorManager>,
    base: Arc<Path>,
    input: ElementCapture<HtmlInputElement>,
    cancel_rx: oneshot::Receiver<()>,
) {
    let mut results = run_query(manager.remote.clone(), base, input)
        .await
        .take_until(cancel_rx);
    while let Some(results) = results.next().await {
        manager.editor_state.update_mut(move |editor_state| {
            let EditorState::Search(search_state) = editor_state else {
                return std::mem::take(editor_state);
            };
            search_state.results = results.into();
            std::mem::take(editor_state)
        });
    }
}

async fn run_query(
    remote: ClientAddress,
    base: Arc<Path>,
    input: ElementCapture<HtmlInputElement>,
) -> impl Stream<Item = Vec<FileMetadata>> {
    let input = input.with(|i| i.value());
    let stream = match super::client::search(remote, base, input).await {
        Ok(stream) => stream.left_stream(),
        Err(error) => futures::stream::once(ready(Err(error))).right_stream(),
    };
    let mut accu = vec![];
    stream.ready_chunks(10).map(move |items| {
        for item in items {
            accu.push(item.unwrap_or_else(failed_file_metadata));
        }
        accu.clone()
    })
}

fn failed_file_metadata(error: impl ToString) -> FileMetadata {
    FileMetadata {
        name: error.to_string().into(),
        ..FileMetadata::default()
    }
}
