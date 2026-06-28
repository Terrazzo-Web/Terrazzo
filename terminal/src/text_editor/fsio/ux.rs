#![cfg(feature = "client")]

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::diagnostics;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::element_capture::ElementCapture;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use web_sys::KeyboardEvent;

use self::diagnostics::error;
use self::diagnostics::warn;
use super::File;
use crate::assets::icons;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::EditorState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::style;
use crate::tiles::app::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CreateEntryKind {
    File,
    Folder,
}

#[html]
#[template(tag = div)]
pub fn create_entry_controls(
    manager: Ptr<TextEditorManager>,
    #[signal] editor_state: EditorState,
) -> XElement {
    let EditorState::Data(data) = editor_state else {
        return tag(style::display = "none", style::visibility = "hidden");
    };
    if !matches!(&*data.data, File::Folder(_)) {
        return tag(style::display = "none", style::visibility = "hidden");
    }

    let active: XSignal<Option<CreateEntryKind>> = XSignal::new("create-entry-active", None);
    let input: Rc<RefCell<Option<ElementCapture<HtmlInputElement>>>> = Default::default();
    return tag(
        class = style::PATH_SELECTOR,
        style::flex_basis %= flex_basis(active.clone()),
        create_entry_icon(
            input.clone(),
            active.clone(),
            CreateEntryKind::File,
            icons::new_file(),
            "Create file",
            "create-file-icon",
        ),
        create_entry_icon(
            input.clone(),
            active.clone(),
            CreateEntryKind::Folder,
            icons::new_folder(),
            "Create folder",
            "create-folder-icon",
        ),
        create_entry_input(manager, input, data.path, active.clone(), active),
    );

    #[template(wrap = true)]
    fn flex_basis(#[signal] active: Option<CreateEntryKind>) -> XAttributeValue {
        if active.is_none() { "0" } else { "auto" }
    }
}

#[autoclone]
#[html]
fn create_entry_icon(
    input: Rc<RefCell<Option<ElementCapture<HtmlInputElement>>>>,
    active: XSignal<Option<CreateEntryKind>>,
    kind: CreateEntryKind,
    icon: icons::Icon,
    title: &'static str,
    _test_class: &'static str,
) -> XElement {
    return img(
        class = style::CREATE_ENTRY_ICON,
        #[cfg(not(feature = "client-prod"))]
        class = _test_class,
        class %= icon_class(active.clone(), kind),
        src = icon,
        title = title,
        click = move |_| {
            autoclone!(active, input);
            let is_active = active.get_value_untracked() == Some(kind);
            active.set((!is_active).then_some(kind));
            if is_active {
                return;
            }
            if let Some(input) = &*input.borrow() {
                input.try_with(|input| {
                    input.set_value("");
                    let focused = input.focus();
                    focused.unwrap_or_else(|error| error!("Failed to focus: {error:?}"))
                });
            }
        },
    );

    #[template(wrap = true)]
    fn icon_class(
        #[signal] active: Option<CreateEntryKind>,
        kind: CreateEntryKind,
    ) -> XAttributeValue {
        (active == Some(kind)).then_some(style::ACTIVE)
    }
}

#[autoclone]
#[html]
#[template(tag = div)]
fn create_entry_input(
    manager: Ptr<TextEditorManager>,
    input: Rc<RefCell<Option<ElementCapture<HtmlInputElement>>>>,
    folder_path: FilePath<Arc<Path>>,
    input_active: XSignal<Option<CreateEntryKind>>,
    #[signal] active: Option<CreateEntryKind>,
) -> XElement {
    let input = {
        let capture = ElementCapture::default();
        *input.borrow_mut() = Some(capture.clone());
        capture
    };
    tag(
        class = style::PATH_SELECTOR_WIDGET,
        class = style::PATH_SELECTOR_INPUT,
        style = active
            .is_none()
            .then_some("display: none; visibility: hidden;"),
        key = match active {
            Some(CreateEntryKind::File) => "create-entry-file",
            Some(CreateEntryKind::Folder) => "create-entry-folder",
            None => "create-entry-none",
        },
        input(
            before_render = input.capture(),
            r#type = "text",
            class = style::PATH_SELECTOR_FIELD,
            #[cfg(not(feature = "client-prod"))]
            class = "create-entry-field",
            keydown = move |event: KeyboardEvent| {
                autoclone!(input_active);
                if event.key() == "Escape" {
                    event.prevent_default();
                    input_active.set(None);
                }
            },
            change = move |_event: web_sys::Event| {
                autoclone!(manager, input, input_active, folder_path);
                let name = input.try_with(|input| input.value()).unwrap_or_default();
                submit_create_entry(
                    &manager,
                    &input,
                    &input_active,
                    folder_path.clone(),
                    active,
                    name,
                );
            },
            blur = move |_| input_active.set(None),
        ),
    )
}

fn submit_create_entry(
    manager: &Ptr<TextEditorManager>,
    input: &ElementCapture<HtmlInputElement>,
    input_active: &XSignal<Option<CreateEntryKind>>,
    folder_path: FilePath<Arc<Path>>,
    active_kind: Option<CreateEntryKind>,
    name: String,
) {
    if let Some(kind) = active_kind {
        let name = name.trim().to_owned();
        close_create_entry(input_active, input);
        spawn_local(create_entry(manager.clone(), folder_path, name, kind));
    }
}

fn close_create_entry(
    input_active: &XSignal<Option<CreateEntryKind>>,
    input: &ElementCapture<HtmlInputElement>,
) {
    input_active.set(None);
    let _ = input.try_with(|input| input.blur());
}

async fn create_entry(
    manager: Ptr<TextEditorManager>,
    path: FilePath<Arc<Path>>,
    name: String,
    kind: CreateEntryKind,
) {
    if name.is_empty() {
        return;
    }

    let result = match kind {
        CreateEntryKind::File => {
            super::client::create_file(manager.remote.clone(), path.clone(), name).await
        }
        CreateEntryKind::Folder => {
            super::client::create_folder(manager.remote.clone(), path.clone(), name).await
        }
    };
    if let Err(error) = result {
        warn!("Failed to create entry: {error}");
        return;
    }
    if manager.path.as_ref().map(|s| s.get_value_untracked()) == path {
        manager.path.file.force(path.file);
    } else {
        manager.tile.app.force(App::TextEditor);
    }
}
