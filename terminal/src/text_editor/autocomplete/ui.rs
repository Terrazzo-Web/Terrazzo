
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::debounce::DoDebounce;
use terrazzo::widgets::element_capture::ElementCapture;
use wasm_bindgen_futures::spawn_local;
use web_sys::FocusEvent;
use web_sys::HtmlInputElement;
use web_sys::MouseEvent;

use self::diagnostics::Instrument as _;
use self::diagnostics::info;
use super::server_fn::AutocompleteItem;
use super::server_fn::autocomplete_path;
use crate::text_editor::fsio::ROOT_FILE_PATH;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::path_selector::schema::PathSelector;
use crate::text_editor::style;

#[autoclone]
#[html]
#[template(tag = ul)]
pub fn show_autocomplete(
    manager: Ptr<TextEditorManager>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
    input: ElementCapture<HtmlInputElement>,
    autocomplete_sig: XSignal<Option<Vec<AutocompleteItem>>>,
    #[signal] autocomplete: Option<Vec<AutocompleteItem>>,
    path: XSignal<Arc<Path>>,
) -> XElement {
    let Some(autocomplete) = autocomplete else {
        return tag(style::visibility = "hidden", style::display = "none");
    };
    let items = autocomplete.into_iter().map(|item| {
        let item_display = if item.is_dir && item.path != "/" {
            format!("{}/", item.path)
        } else if item.path.trim().is_empty() {
            // Unicode character U+00A0, called NO-BREAK SPACE.
            "\u{00A0}".into()
        } else {
            item.path.to_owned()
        };
        li(
            "{item_display}",
            mousedown = move |ev: MouseEvent| {
                autoclone!(manager, input, autocomplete_sig, prefix, path);
                ev.prevent_default();
                ev.stop_propagation();
                {
                    input.with(|i| i.set_value(&item.path));
                    path.set(Arc::from(Path::new(item.path.as_str().trim())));
                }
                do_autocomplete_impl(
                    manager.clone(),
                    kind,
                    prefix.clone(),
                    input.clone(),
                    autocomplete_sig.clone(),
                );
            },
        )
    });
    tag(class = style::PATH_SELECTOR_AUTOCOMPLETE, items..)
}

#[autoclone]
pub fn start_autocomplete(
    manager: Ptr<TextEditorManager>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
    path: XSignal<Arc<Path>>,
    input: ElementCapture<HtmlInputElement>,
    autocomplete: XSignal<Option<Vec<AutocompleteItem>>>,
) -> impl Fn(FocusEvent) {
    move |_| {
        info!("Start autocomplete {kind:?}");
        let input_element_blur = scopeguard::guard(input.clone(), move |input| {
            autoclone!(path, autocomplete);
            do_stop_autocomplete(kind, &path, &input, &autocomplete);
        });
        let before_menu = &manager.tile.menu.before;
        before_menu.add(move || drop(input_element_blur));
        autocomplete.set(Some(Default::default()));
        do_autocomplete_impl(
            manager.clone(),
            kind,
            prefix.clone(),
            input.clone(),
            autocomplete.clone(),
        );
    }
}

pub fn stop_autocomplete(
    kind: PathSelector,
    path: XSignal<Arc<Path>>,
    input: ElementCapture<HtmlInputElement>,
    autocomplete: XSignal<Option<Vec<AutocompleteItem>>>,
) -> impl Fn(FocusEvent) {
    move |_| do_stop_autocomplete(kind, &path, &input, &autocomplete)
}

pub fn do_stop_autocomplete(
    kind: PathSelector,
    path: &XSignal<Arc<Path>>,
    input: &ElementCapture<HtmlInputElement>,
    autocomplete: &XSignal<Option<Vec<AutocompleteItem>>>,
) {
    let Some(value) = input.try_with(|i| i.value()) else {
        return;
    };
    let value = value.trim();
    info!("Update {kind:?} path to {value}");
    path.set(Arc::from(Path::new(value)));
    autocomplete.set(None);
}

pub fn do_autocomplete(
    manager: Ptr<TextEditorManager>,
    input: ElementCapture<HtmlInputElement>,
    autocomplete: XSignal<Option<Vec<AutocompleteItem>>>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
) -> impl Fn(()) {
    Duration::from_millis(250).debounce(move |()| {
        do_autocomplete_impl(
            manager.clone(),
            kind,
            prefix.clone(),
            input.clone(),
            autocomplete.clone(),
        )
    })
}

#[autoclone]
fn do_autocomplete_impl(
    manager: Ptr<TextEditorManager>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
    input: ElementCapture<HtmlInputElement>,
    autocomplete: XSignal<Option<Vec<AutocompleteItem>>>,
) {
    let value = input.with(|i| i.value());
    let do_autocomplete_async = async move {
        autoclone!(autocomplete);
        let autocompletes = autocomplete_path(
            manager.remote.clone(),
            kind,
            prefix
                .as_ref()
                .map(XSignal::get_value_untracked)
                .unwrap_or(ROOT_FILE_PATH.clone()),
            value,
        )
        .await
        .or_else_throw(|error| format!("Autocomplete failed: {error}"));
        autocomplete.update(|old| {
            if old.is_some() {
                Some(Some(autocompletes))
            } else {
                None
            }
        });
    };
    spawn_local(do_autocomplete_async.in_current_span());
}
