use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;

use scopeguard::guard;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::Element;

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::warn;
use super::editor_render::EditorBodyContext;
use super::editor_render::EditorType;
use super::editor_render::make_pdf_body;
use super::fsio;
use super::fsio::client::store_file;
use super::style;
use crate::frontend::input_overlay::InputOverlay;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::EditorDataState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::server_fn::EventKind;
use crate::text_editor::notify::server_fn::FileEventKind;
use crate::text_editor::notify::server_fn::NotifyResponse;
use crate::text_editor::state::PreviewMode;
use crate::text_editor::synchronized_state::SynchronizedState;
use crate::text_editor::ui::ROOT_FILE_PATH;

#[derive(Clone)]
pub(super) enum EditorDocument {
    Text {
        original: Option<Arc<str>>,
        content: Arc<str>,
    },
    Pdf(Arc<str>),
}

pub(super) trait EditorBody {
    fn set_content(&self, content: String);

    fn flush_changes(&self) {}

    fn has_pending_changes(&self) -> bool {
        false
    }

    fn insert_text(&self, _text: String) {}

    fn focus(&self) {}

    fn cargo_check(&self, _diagnostics: JsValue) {}

    fn set_collapse_unchanged(&self, _collapse: bool) {}
}

#[autoclone]
#[html]
#[template(tag = div, key = {
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering::SeqCst;
    static NEXT: AtomicI32 = AtomicI32::new(1);
    format!("editor-{}", NEXT.fetch_add(1, SeqCst))
})]
pub fn editor(
    manager: Ptr<TextEditorManager>,
    editor_state: EditorDataState,
    document: EditorDocument,
    show_editor_diff: bool,
    show_html_preview: PreviewMode,
) -> XElement {
    let is_html = editor_state.is_html();
    let is_markdown = editor_state.is_markdown();
    let EditorDataState {
        path,
        cursor_position,
        ..
    } = editor_state;
    let editor_type = if matches!(document, EditorDocument::Pdf(_)) {
        EditorType::Pdf
    } else if is_markdown {
        EditorType::Markdown
    } else if is_html {
        EditorType::Html
    } else {
        EditorType::Text
    };
    let preview_pane = match (&document, editor_type) {
        (EditorDocument::Text { content, .. }, EditorType::Html) => {
            Some(super::html_viewer::html_viewer(content.clone()))
        }
        (_, EditorType::Markdown) => Some(div(
            class = super::milkdown::style::MILKDOWN_WYSIWYG_PANE,
            #[cfg(not(feature = "client-prod"))]
            class = "milkdown-wysiwyg-pane",
        )),
        _ => None,
    };
    let source_pane = match editor_type {
        EditorType::Html => Some(div(
            class = super::html_viewer::style::HTML_SOURCE_PANE,
            #[cfg(not(feature = "client-prod"))]
            class = "html-source-pane",
        )),
        EditorType::Markdown => Some(div(
            class = super::milkdown::style::MILKDOWN_SOURCE_PANE,
            #[cfg(not(feature = "client-prod"))]
            class = "milkdown-source-pane",
        )),
        _ => None,
    };

    // Count edits waiting (debounced) to be committed. Notifications can arrive
    // out of causal order, so don't refresh CodeMirror while local edits are pending.
    let writing = Arc::new(AtomicU32::new(0));

    let editor_body: Ptr<Mutex<Option<Box<dyn EditorBody>>>> = Ptr::new(Mutex::new(None));
    let collapse_registration = manager.collapse_unchanged.add_subscriber(move |collapse| {
        autoclone!(editor_body);
        if let Some(body) = &*editor_body.lock().unwrap() {
            body.set_collapse_unchanged(collapse);
        }
    });
    let flush_registration = manager.flush_editor_changes.add_subscriber({
        let editor_body = editor_body.clone();
        move |_| {
            if let Some(body) = &*editor_body.lock().unwrap() {
                body.flush_changes();
            }
        }
    });
    let focus_editor: Ptr<dyn Fn()> = Ptr::new(move || {
        autoclone!(editor_body);
        if let Some(editor_body) = &*editor_body.lock().unwrap() {
            editor_body.focus();
        }
    });
    let (input_overlay_html, input_overlay) = if editor_type.use_overlay(show_html_preview) {
        let send_to_editor: Ptr<dyn Fn(String)> = Ptr::new(move |text| {
            autoclone!(editor_body);
            if let Some(editor_body) = &*editor_body.lock().unwrap() {
                editor_body.insert_text(text);
            }
        });

        let InputOverlay {
            is_open: is_input_overlay_open,
            html: input_overlay_html,
            textarea: input_overlay_textarea,
        } = InputOverlay::new(send_to_editor, focus_editor.clone());
        (
            Some(input_overlay_html),
            Some((is_input_overlay_open, input_overlay_textarea)),
        )
    } else {
        (None, None)
    };

    let edits_notify_registration = manager.notify_service.watch_file(
        &path,
        make_edits_notify_handler(&manager, &editor_body, &path, &writing),
    );
    let base_path = FilePath {
        base: path.base.clone(),
        file: ROOT_FILE_PATH.clone(),
    };
    let diagnostics_notify_registration = manager.notify_service.watch_file(
        &base_path,
        make_diagnostics_notify_handler(&editor_body, &base_path),
    );

    tag(
        class = style::EDITOR,
        class = editor_type.class(),
        class = editor_type.preview_mode_class(show_html_preview),
        #[cfg(not(feature = "client-prod"))]
        class = (editor_type == EditorType::Pdf).then_some("pdf-viewer"),
        #[cfg(not(feature = "client-prod"))]
        class = (editor_type == EditorType::Html).then_some("html-viewer"),
        #[cfg(not(feature = "client-prod"))]
        class = (editor_type == EditorType::Markdown).then_some("milkdown-editor"),
        #[cfg(not(feature = "client-prod"))]
        class = (editor_type == EditorType::Text).then_some("code-mirror-editor"),
        preview_pane..,
        source_pane..,
        input_overlay_html..,
        mouseenter = move |_| {
            if let Some((is_input_overlay_open, input_overlay_textarea)) = &input_overlay
                && is_input_overlay_open.get_value_untracked()
            {
                input_overlay_textarea.try_with(|textarea| {
                    textarea.focus().unwrap_or_else(|error| {
                        warn!("Failed to focus: {error:?}");
                    })
                });
            } else if editor_type != EditorType::Markdown {
                focus_editor()
            }
        },
        after_render = move |element| {
            autoclone!(path);
            let _moved = &collapse_registration;
            let _moved = &flush_registration;
            let _moved = &edits_notify_registration;
            let _moved = &diagnostics_notify_registration;
            let context = EditorBodyContext {
                manager: &manager,
                path: &path,
                writing: &writing,
                element: &element,
                cursor_position,
                editor_type,
                show_editor_diff,
                show_html_preview,
            };
            let body = match &document {
                EditorDocument::Text { original, content } => {
                    context.make_text_body(original, content)
                }
                EditorDocument::Pdf(base64) => Some(make_pdf_body(&element, base64)),
            };
            if let Some(body) = &body {
                body.set_collapse_unchanged(manager.collapse_unchanged.get_value_untracked());
            }
            *editor_body.lock().unwrap() = body;
        },
    )
}

/// Builds the full-document callback throttled in JavaScript to at most once per
/// second while typing, pasting, or undoing/redoing in CodeMirror or Milkdown.
/// Each burst delivers its latest document even if typing stops. Mode changes and
/// editor teardown explicitly flush pending edits and may bypass the interval.
/// Cursor-only updates and disk reloads do not invoke it.
///
/// CodeMirror flattens the document only when the throttle emits. Each invocation
/// still copies the whole body into Rust, so its cost scales with file size.
/// Persistence independently coalesces bodies in 500 ms windows (1500 ms in debug
/// builds); that window is not reset by keystrokes. Markdown source/preview
/// synchronization stays immediate within JavaScript.
#[autoclone]
pub(super) fn make_on_change(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    writing: &Arc<AtomicU32>,
    html_preview: Option<Element>,
) -> Closure<dyn FnMut(JsValue)> {
    Closure::new(move |content: JsValue| {
        autoclone!(manager, path, writing);
        let Some(content) = content.as_string() else {
            debug!("Changed content is not a string");
            return;
        };
        if let Some(html_preview) = &html_preview {
            let _ = html_preview.set_attribute("srcdoc", &content);
        }
        manager.update_content(&path, Arc::from(content.as_str()));
        writing.fetch_add(1, SeqCst);
        let writing_done = guard((), move |()| {
            autoclone!(writing);
            writing.fetch_sub(1, SeqCst);
        });
        let write = async move {
            autoclone!(manager, path);
            let synchronized_state_done =
                SynchronizedState::enqueue(manager.synchronized_state.clone());
            let () = store_file(
                manager.remote.clone(),
                path,
                content,
                guard((), move |()| ()),
                (writing_done, synchronized_state_done),
            )
            .await;
        };
        spawn_local(write.in_current_span());
    })
}

#[autoclone]
pub(super) fn make_on_cursor_position_change(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
) -> Closure<dyn FnMut(JsValue)> {
    Closure::new(move |cursor_position: JsValue| {
        autoclone!(manager, path);
        let Ok(cursor_position) = serde_wasm_bindgen::from_value(cursor_position) else {
            debug!("Changed cursor position is invalid");
            return;
        };
        let write = async move {
            autoclone!(manager, path);
            let synchronized_state_done =
                SynchronizedState::enqueue(manager.synchronized_state.clone());
            let () =
                fsio::client::store_cursor_position(manager.remote.clone(), path, cursor_position)
                    .await;
            drop(synchronized_state_done);
        };
        spawn_local(write.in_current_span());
    })
}

#[autoclone]
fn make_edits_notify_handler(
    manager: &Ptr<TextEditorManager>,
    editor_body: &Ptr<Mutex<Option<Box<dyn EditorBody>>>>,
    path: &FilePath<Arc<Path>>,
    writing: &Arc<AtomicU32>,
) -> impl Fn(&NotifyResponse) + 'static {
    move |event| {
        autoclone!(manager, editor_body, path, writing);
        let _span = debug_span!("Editor notifier", ?path).entered();
        let EventKind::File(FileEventKind::Create | FileEventKind::Modify) = event.kind else {
            return;
        };
        spawn_local(
            notify_edit(
                manager.clone(),
                editor_body.clone(),
                path.clone(),
                writing.clone(),
            )
            .in_current_span(),
        );
    }
}

/// Reloads the full file once for each Create/Modify event delivered by the file
/// watcher, including events caused by our own saves. Events are not debounced
/// here, so one disk operation may trigger multiple reloads.
///
/// Each invocation performs a full-file server read and response transfer, which
/// scale with file size. Pending local writes suppress applying the response to
/// the editor, but the load has already happened. For multi-megabyte files, bursts
/// of notifications can therefore cause repeated large transfers even when the
/// response is discarded; this path does not send incremental changes.
async fn notify_edit(
    manager: Ptr<TextEditorManager>,
    editor_body: Ptr<Mutex<Option<Box<dyn EditorBody>>>>,
    path: FilePath<Arc<Path>>,
    writing: Arc<AtomicU32>,
) {
    debug!("Loading modified file");
    match fsio::client::load_file(manager.remote.clone(), path.clone()).await {
        Ok(Some(fsio::File::TextFile {
            metadata: _,
            original: _,
            content,
            ..
        })) => {
            debug!("Loaded modified file");
            let Some(editor_body) = &*editor_body.lock().unwrap() else {
                debug!("The modified file has no mutable editor body, force reload");
                manager.path.file.force(path.file);
                return;
            };
            if writing.load(SeqCst) == 0 && !editor_body.has_pending_changes() {
                manager.update_content(&path, content.clone());
                editor_body.set_content(content.to_string());
            }
        }
        Ok(Some(fsio::File::PdfFile { base64, .. })) => {
            debug!("Loaded modified file");
            let Some(editor_body) = &*editor_body.lock().unwrap() else {
                return;
            };
            editor_body.set_content(base64.to_string());
        }
        Ok(None) => {
            debug!("The modified file is gone");
            manager.path.file.update(|file_path| {
                let parent = file_path.parent().unwrap_or_else(|| "/".as_ref());
                Some(Arc::from(parent))
            })
        }
        Ok(Some(fsio::File::Folder { .. })) => {
            debug!("The modified file is a folder, force reload");
            manager.path.file.force(path.file);
        }
        Ok(Some(fsio::File::Error(error))) => {
            warn!("Loading file returned {error}");
        }
        Err(error) => {
            warn!("Failed to load file: {error}")
        }
    };
}

#[autoclone]
fn make_diagnostics_notify_handler(
    editor_body: &Ptr<Mutex<Option<Box<dyn EditorBody>>>>,
    path: &FilePath<Arc<Path>>,
) -> impl Fn(&NotifyResponse) + 'static {
    move |event| {
        autoclone!(editor_body, path);
        let _span = debug_span!("Diagnostics notifier", ?path).entered();
        let EventKind::CargoCheck(diagnostics) = &event.kind else {
            return;
        };
        if let Ok(diagnostics) = serde_wasm_bindgen::to_value(diagnostics)
            && let Some(editor_body) = &*editor_body.lock().unwrap()
        {
            editor_body.cargo_check(diagnostics);
        }
    }
}
