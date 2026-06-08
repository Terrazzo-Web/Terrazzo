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

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::warn;
use super::code_mirror::CodeMirrorJs;
use super::fsio;
use super::fsio::client::store_file;
use super::pdf_viewer::PdfJs;
use super::style;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::EditorDataState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::server_fn::EventKind;
use crate::text_editor::notify::server_fn::FileEventKind;
use crate::text_editor::notify::server_fn::NotifyResponse;
use crate::text_editor::synchronized_state::SynchronizedState;
use crate::text_editor::ui::ROOT_FILE_PATH;
use crate::utils::more_path::MorePath as _;

#[derive(Clone)]
pub(super) enum EditorDocument {
    Text {
        original: Option<Arc<str>>,
        content: Arc<str>,
    },
    Pdf(Arc<str>),
}

trait EditorBody {
    fn set_content(&self, content: String);

    fn cargo_check(&self, _diagnostics: JsValue) {}
}

impl EditorBody for CodeMirrorJs {
    fn set_content(&self, content: String) {
        self.set_content(content);
    }

    fn cargo_check(&self, diagnostics: JsValue) {
        self.cargo_check(diagnostics);
    }
}

impl EditorBody for PdfJs {
    fn set_content(&self, content: String) {
        self.set_content(content);
    }
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
    #[signal] show_editor_diff: bool,
    show_html_preview: bool,
) -> XElement {
    let EditorDataState { path, .. } = editor_state;
    let is_pdf = matches!(document, EditorDocument::Pdf(_));
    let is_html_preview = path.file.extension() == Some("html".as_ref()) && show_html_preview;
    let html_preview = match &document {
        EditorDocument::Text { content, .. } if is_html_preview => {
            super::html_viewer::html_viewer(content.clone())
        }
        _ => span(style::display = "none", style::visibility = "hidden"),
    };

    // Count edits waiting (debounced) to be committed. Notifications can arrive
    // out of causal order, so don't refresh CodeMirror while local edits are pending.
    let writing = Arc::new(AtomicU32::new(0));

    let editor_body: Ptr<Mutex<Option<Box<dyn EditorBody>>>> = Ptr::new(Mutex::new(None));

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
        class = is_pdf.then_some(super::pdf_viewer::style::PDF_VIEWER),
        class = is_html_preview.then_some(super::html_viewer::style::HTML_VIEWER),
        #[cfg(not(feature = "client-prod"))]
        class = is_pdf.then_some("pdf-viewer"),
        #[cfg(not(feature = "client-prod"))]
        class = is_html_preview.then_some("html-viewer"),
        #[cfg(not(feature = "client-prod"))]
        class = (!is_pdf && !is_html_preview).then_some("code-mirror-editor"),
        html_preview,
        after_render = move |element| {
            autoclone!(path);
            let _moved = &edits_notify_registration;
            let _moved = &diagnostics_notify_registration;
            let body: Option<Box<dyn EditorBody>> = match &document {
                EditorDocument::Text { .. } if is_html_preview => None,
                EditorDocument::Text { original, content } => {
                    let original = if show_editor_diff {
                        original
                            .as_deref()
                            .map(JsValue::from)
                            .unwrap_or(JsValue::null())
                    } else {
                        JsValue::null()
                    };
                    Some(Box::new(CodeMirrorJs::new(
                        element.clone(),
                        original,
                        content.as_ref().into(),
                        make_on_change(&manager, &path, &writing),
                        path.base.as_ref().to_owned_string(),
                        path.as_deref().full_path().to_owned_string(),
                    )))
                }
                EditorDocument::Pdf(base64) => Some(Box::new(PdfJs::new(element.clone(), base64))),
            };
            *editor_body.lock().unwrap() = body;
        },
    )
}

#[autoclone]
fn make_on_change(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    writing: &Arc<AtomicU32>,
) -> Closure<dyn FnMut(JsValue)> {
    Closure::new(move |content: JsValue| {
        autoclone!(manager, path, writing);
        let Some(content) = content.as_string() else {
            debug!("Changed content is not a string");
            return;
        };
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
        })) => {
            debug!("Loaded modified file");
            let Some(editor_body) = &*editor_body.lock().unwrap() else {
                debug!("The modified file has no mutable editor body, force reload");
                manager.path.file.force(path.file);
                return;
            };
            if writing.load(SeqCst) == 0 {
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
