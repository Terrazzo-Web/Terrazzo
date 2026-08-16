#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::*;

use super::file_path::FilePath;
use super::fsio;
use super::notify::ui::NotifyService;
use super::search::state::EditorSearchState;
use super::search::state::SearchState;
use super::side::SideViewNode;
use super::synchronized_state::SynchronizedState;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::remotes::Remote;
use crate::tiles::signals::TilePtr;

pub(super) struct TextEditorManager {
    pub tile: TilePtr,
    pub remote: Remote,
    pub path: FilePath<XSignal<Arc<Path>>>,
    pub force_edit_path: XSignal<bool>,
    pub editor_state: XSignal<EditorState>,
    pub show_editor_diff: XSignal<bool>,
    pub show_html_preview: XSignal<PreviewMode>,
    pub synchronized_state: XSignal<SynchronizedState>,
    pub side_view: XSignal<Option<Arc<SideViewNode>>>,
    pub notify_service: Ptr<NotifyService>,
    pub search: Ptr<SearchState>,
    pub side_view_resize_manager: MousemoveManager,
}

#[derive(Clone, Debug, Default)]
pub(super) enum EditorState {
    Data(EditorDataState),
    Search(EditorSearchState),
    #[default]
    Empty,
}

impl EditorState {
    pub(super) fn is_html(&self) -> bool {
        matches!(self, Self::Data(editor_state) if editor_state.is_html())
    }

    pub(super) fn supports_preview(&self) -> bool {
        matches!(self, Self::Data(editor_state) if editor_state.supports_preview())
    }
}

#[derive(Clone)]
pub(super) struct EditorDataState {
    pub path: FilePath<Arc<Path>>,
    pub data: Arc<fsio::File>,
    pub cursor_position: Option<fsio::CursorPosition>,
}

impl EditorDataState {
    pub(super) fn is_html(&self) -> bool {
        self.path.file.extension() == Some("html".as_ref())
    }

    pub(super) fn is_markdown(&self) -> bool {
        self.path.file.extension() == Some("md".as_ref())
    }

    pub(super) fn supports_preview(&self) -> bool {
        self.is_html() || self.is_markdown()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PreviewMode {
    #[default]
    Preview,
    Editor,
    SideBySide,
}

impl PreviewMode {
    pub(super) fn next(self) -> Self {
        match self {
            Self::Preview => Self::Editor,
            Self::Editor => Self::SideBySide,
            Self::SideBySide => Self::Preview,
        }
    }

    pub(super) fn shows_editor(self) -> bool {
        matches!(self, Self::Editor | Self::SideBySide)
    }
}

impl std::fmt::Debug for EditorDataState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("path", &self.path)
            .field("data", &self.data)
            .finish()
    }
}
