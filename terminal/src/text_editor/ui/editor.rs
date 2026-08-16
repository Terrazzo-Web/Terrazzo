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
use super::milkdown::MilkdownJs;
use super::pdf_viewer::PdfJs;
use super::style;
use crate::frontend::input_overlay::InputOverlay;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::EditorDataState;
use crate::text_editor::manager::PreviewMode;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::server_fn::EventKind;
use crate::text_editor::notify::server_fn::FileEventKind;
use crate::text_editor::notify::server_fn::NotifyResponse;
use crate::text_editor::synchronized_state::SynchronizedState;
use crate::text_editor::ui::ROOT_FILE_PATH;
use crate::utils::more_path::MorePath as _;
use web_sys::Element;

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

    fn insert_text(&self, _text: String) {}

    fn focus(&self) {}

    fn cargo_check(&self, _diagnostics: JsValue) {}
}

struct HtmlEditorBody {
    source: CodeMirrorJs,
    preview: Element,
}

impl EditorBody for HtmlEditorBody {
    fn set_content(&self, content: String) {
        self.source.set_content(content.clone());
        let _ = self.preview.set_attribute("srcdoc", &content);
    }

    fn insert_text(&self, text: String) {
        self.source.insert_text(text);
    }

    fn focus(&self) {
        self.source.focus();
    }

    fn cargo_check(&self, diagnostics: JsValue) {
        self.source.cargo_check(diagnostics);
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
            } else {
                focus_editor()
            }
        },
        after_render = move |element| {
            autoclone!(path);
            let _moved = &edits_notify_registration;
            let _moved = &diagnostics_notify_registration;
            let body: Option<Box<dyn EditorBody>> = match &document {
                EditorDocument::Text { original, content }
                    if matches!(
                        editor_type,
                        EditorType::Text | EditorType::Markdown | EditorType::Html
                    ) =>
                {
                    let original = if show_editor_diff {
                        original
                            .as_deref()
                            .map(JsValue::from)
                            .unwrap_or(JsValue::null())
                    } else {
                        JsValue::null()
                    };
                    let cursor_position = cursor_position
                        .and_then(|cursor_position| {
                            serde_wasm_bindgen::to_value(&cursor_position).ok()
                        })
                        .unwrap_or(JsValue::null());
                    let base_path = path.base.as_ref().to_owned_string();
                    let full_path = path.as_deref().full_path().to_owned_string();
                    if editor_type == EditorType::Markdown {
                        let wysiwyg_pane = element
                            .query_selector(&format!(
                                ".{}",
                                super::milkdown::style::MILKDOWN_WYSIWYG_PANE
                            ))
                            .expect("Invalid Milkdown preview pane selector")
                            .expect("Missing Milkdown preview pane");
                        let source_pane = element
                            .query_selector(&format!(
                                ".{}",
                                super::milkdown::style::MILKDOWN_SOURCE_PANE
                            ))
                            .expect("Invalid Milkdown source pane selector")
                            .expect("Missing Milkdown source pane");
                        Some(Box::new(MilkdownJs::new(
                            wysiwyg_pane,
                            source_pane,
                            original,
                            content.as_ref().into(),
                            make_on_change(&manager, &path, &writing, None),
                            make_on_cursor_position_change(&manager, &path),
                            cursor_position,
                            base_path,
                            full_path,
                            show_html_preview == PreviewMode::Editor,
                        )))
                    } else {
                        let (source_element, preview) = if editor_type == EditorType::Html {
                            let source = element
                                .query_selector(&format!(
                                    ".{}",
                                    super::html_viewer::style::HTML_SOURCE_PANE
                                ))
                                .expect("Invalid HTML source pane selector")
                                .expect("Missing HTML source pane");
                            let preview = element
                                .query_selector(&format!(
                                    ".{}",
                                    super::html_viewer::style::HTML_PREVIEW_PANE
                                ))
                                .expect("Invalid HTML preview pane selector")
                                .expect("Missing HTML preview pane");
                            (source, Some(preview))
                        } else {
                            (element.clone(), None)
                        };
                        let source = CodeMirrorJs::new(
                            source_element,
                            original,
                            content.as_ref().into(),
                            make_on_change(&manager, &path, &writing, preview.clone()),
                            make_on_cursor_position_change(&manager, &path),
                            cursor_position,
                            base_path,
                            full_path,
                        );
                        if let Some(preview) = preview {
                            Some(Box::new(HtmlEditorBody { source, preview }))
                        } else {
                            Some(Box::new(source))
                        }
                    }
                }
                EditorDocument::Pdf(base64) => Some(Box::new(PdfJs::new(element.clone(), base64))),
                EditorDocument::Text { .. } => None,
            };
            *editor_body.lock().unwrap() = body;
        },
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorType {
    Html,
    Pdf,
    Text,
    Markdown,
}

impl EditorType {
    fn use_overlay(self, preview_mode: PreviewMode) -> bool {
        match self {
            EditorType::Html => preview_mode.shows_editor(),
            EditorType::Pdf => false,
            EditorType::Text | EditorType::Markdown => true,
        }
    }

    fn class(self) -> Option<&'static str> {
        Some(match self {
            EditorType::Html => super::html_viewer::style::HTML_VIEWER,
            EditorType::Pdf => super::pdf_viewer::style::PDF_VIEWER,
            EditorType::Markdown => super::milkdown::style::MILKDOWN_EDITOR,
            EditorType::Text => return None,
        })
    }

    fn preview_mode_class(self, preview_mode: PreviewMode) -> Option<&'static str> {
        match self {
            EditorType::Html => Some(match preview_mode {
                PreviewMode::Preview => super::html_viewer::style::PREVIEW_MODE,
                PreviewMode::Editor => super::html_viewer::style::EDITOR_MODE,
                PreviewMode::SideBySide => super::html_viewer::style::SIDE_BY_SIDE_MODE,
            }),
            EditorType::Markdown => Some(match preview_mode {
                PreviewMode::Preview => super::milkdown::style::PREVIEW_MODE,
                PreviewMode::Editor => super::milkdown::style::EDITOR_MODE,
                PreviewMode::SideBySide => super::milkdown::style::SIDE_BY_SIDE_MODE,
            }),
            EditorType::Pdf | EditorType::Text => None,
        }
    }
}

#[autoclone]
fn make_on_change(
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
fn make_on_cursor_position_change(
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
