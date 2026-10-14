use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use terrazzo::prelude::Ptr;
use wasm_bindgen::JsValue;
use web_sys::Element;

use super::code_mirror::CodeMirrorJs;
use super::editor::EditorBody;
use super::editor::make_on_change;
use super::editor::make_on_cursor_position_change;
use super::milkdown::MilkdownJs;
use super::pdf_viewer::PdfJs;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::state::PreviewMode;
use crate::utils::more_path::MorePath as _;

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

    fn set_collapse_unchanged(&self, collapse: bool) {
        self.source.set_collapse_unchanged(collapse);
    }

    fn cargo_check(&self, diagnostics: JsValue) {
        self.source.cargo_check(diagnostics);
    }
}

pub(super) struct EditorBodyContext<'a> {
    pub(super) manager: &'a Ptr<TextEditorManager>,
    pub(super) path: &'a FilePath<Arc<Path>>,
    pub(super) writing: &'a Arc<AtomicU32>,
    pub(super) element: &'a Element,
    pub(super) cursor_position: Option<fsio::CursorPosition>,
    pub(super) editor_type: EditorType,
    pub(super) show_editor_diff: bool,
    pub(super) show_html_preview: PreviewMode,
}

struct SourceDocument {
    original: JsValue,
    content: JsValue,
    cursor_position: JsValue,
    base_path: String,
    full_path: String,
}

impl EditorBodyContext<'_> {
    pub(super) fn make_text_body(
        &self,
        original: &Option<Arc<str>>,
        content: &Arc<str>,
    ) -> Option<Box<dyn EditorBody>> {
        if self.editor_type == EditorType::Pdf {
            return None;
        }
        let original = if self.show_editor_diff
            && original
                .as_ref()
                .is_some_and(|original| original != content)
        {
            original
                .as_deref()
                .map(JsValue::from)
                .unwrap_or(JsValue::null())
        } else {
            JsValue::null()
        };
        let document = SourceDocument {
            original,
            content: content.as_ref().into(),
            cursor_position: self
                .cursor_position
                .and_then(|position| serde_wasm_bindgen::to_value(&position).ok())
                .unwrap_or(JsValue::null()),
            base_path: self.path.base.as_ref().to_owned_string(),
            full_path: self.path.as_deref().full_path().to_owned_string(),
        };
        Some(if self.editor_type == EditorType::Markdown {
            self.make_markdown_body(document)
        } else {
            self.make_code_mirror_body(document)
        })
    }

    fn make_markdown_body(&self, document: SourceDocument) -> Box<dyn EditorBody> {
        let wysiwyg_pane = self
            .element
            .query_selector(&format!(
                ".{}",
                super::milkdown::style::MILKDOWN_WYSIWYG_PANE
            ))
            .expect("Invalid Milkdown preview pane selector")
            .expect("Missing Milkdown preview pane");
        let source_pane = self
            .element
            .query_selector(&format!(
                ".{}",
                super::milkdown::style::MILKDOWN_SOURCE_PANE
            ))
            .expect("Invalid Milkdown source pane selector")
            .expect("Missing Milkdown source pane");
        Box::new(MilkdownJs::new(
            wysiwyg_pane,
            source_pane,
            document.original,
            document.content,
            make_on_change(self.manager, self.path, self.writing, None),
            make_on_cursor_position_change(self.manager, self.path),
            document.cursor_position,
            document.base_path,
            document.full_path,
            self.show_html_preview == PreviewMode::Editor,
        ))
    }

    fn make_code_mirror_body(&self, document: SourceDocument) -> Box<dyn EditorBody> {
        let (source_element, preview) = if self.editor_type == EditorType::Html {
            let source = self
                .element
                .query_selector(&format!(".{}", super::html_viewer::style::HTML_SOURCE_PANE))
                .expect("Invalid HTML source pane selector")
                .expect("Missing HTML source pane");
            let preview = self
                .element
                .query_selector(&format!(
                    ".{}",
                    super::html_viewer::style::HTML_PREVIEW_PANE
                ))
                .expect("Invalid HTML preview pane selector")
                .expect("Missing HTML preview pane");
            (source, Some(preview))
        } else {
            (self.element.clone(), None)
        };
        let source = CodeMirrorJs::new(
            source_element,
            document.original,
            document.content,
            make_on_change(self.manager, self.path, self.writing, preview.clone()),
            make_on_cursor_position_change(self.manager, self.path),
            document.cursor_position,
            document.base_path,
            document.full_path,
        );
        if let Some(preview) = preview {
            Box::new(HtmlEditorBody { source, preview })
        } else {
            Box::new(source)
        }
    }
}

pub(super) fn make_pdf_body(element: &Element, base64: &Arc<str>) -> Box<dyn EditorBody> {
    Box::new(PdfJs::new(element.clone(), base64))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorType {
    Html,
    Pdf,
    Text,
    Markdown,
}

impl EditorType {
    pub(super) fn use_overlay(self, preview_mode: PreviewMode) -> bool {
        match self {
            EditorType::Html => preview_mode.shows_editor(),
            EditorType::Pdf => false,
            EditorType::Text | EditorType::Markdown => true,
        }
    }

    pub(super) fn class(self) -> Option<&'static str> {
        Some(match self {
            EditorType::Html => super::html_viewer::style::HTML_VIEWER,
            EditorType::Pdf => super::pdf_viewer::style::PDF_VIEWER,
            EditorType::Markdown => super::milkdown::style::MILKDOWN_EDITOR,
            EditorType::Text => return None,
        })
    }

    pub(super) fn preview_mode_class(self, preview_mode: PreviewMode) -> Option<&'static str> {
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
