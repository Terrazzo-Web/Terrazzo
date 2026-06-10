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
    pub show_html_preview: XSignal<bool>,
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

#[derive(Clone)]
pub(super) struct EditorDataState {
    pub path: FilePath<Arc<Path>>,
    pub data: Arc<fsio::File>,
}

impl std::fmt::Debug for EditorDataState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("path", &self.path)
            .field("data", &self.data)
            .finish()
    }
}
