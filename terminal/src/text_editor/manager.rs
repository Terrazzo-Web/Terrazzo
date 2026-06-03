#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::file_path::FilePath;
use super::fsio;
use super::fsio::FileMetadata;
use super::notify::server_fn::EventKind;
use super::notify::server_fn::FileEventKind;
use super::notify::ui::NotifyService;
use super::search::state::EditorSearchState;
use super::search::state::SearchState;
use super::side;
use super::side::SideViewList;
use super::side::SvnStatus;
use super::synchronized_state::SynchronizedState;
use crate::frontend::remotes::Remote;
use crate::text_editor::fsio::client::file_exists;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::tiles::signals::TilePtr;
use crate::utils::more_path::MorePath as _;

pub(super) struct TextEditorManager {
    pub tile: TilePtr,
    pub remote: Remote,
    pub path: FilePath<XSignal<Arc<str>>>,
    pub force_edit_path: XSignal<bool>,
    pub editor_state: XSignal<EditorState>,
    pub synchronized_state: XSignal<SynchronizedState>,
    pub side_view: XSignal<Arc<SideViewList>>,
    pub notify_service: Ptr<NotifyService>,
    pub search: Ptr<SearchState>,
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
    pub path: FilePath<Arc<str>>,
    pub data: Arc<fsio::File>,
}

impl TextEditorManager {
    #[autoclone]
    pub fn add_to_side_view(
        self: &Ptr<Self>,
        metadata: &Arc<FileMetadata>,
        path: &FilePath<Arc<str>>,
    ) {
        let this = self.clone();
        let notify_registration = self.notify_service.watch_file(path, move |event| {
            autoclone!(path);
            let EventKind::File(FileEventKind::Delete | FileEventKind::Error) = event.kind else {
                return;
            };
            // Remove from side view on deletion notification.
            spawn_local(async move {
                autoclone!(this, path);
                if !file_exists(this.remote.clone(), path.clone())
                    .await
                    .unwrap_or(true)
                {
                    this.remove_from_side_view(path.file.as_ref());
                }
            });
        });
        self.side_view.update(|tree| {
            let file_path = Path::new(path.file.as_ref())
                .iter()
                .map(|leg| Arc::from(leg.to_owned_string()))
                .collect::<Vec<_>>();
            Some(side::mutation::add_file(
                tree.clone(),
                file_path.as_slice(),
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Opened,
                    },
                    item: SvnItem::File {
                        metadata: metadata.clone(),
                        notify_registration: notify_registration.into(),
                    },
                },
            ))
        });
        self.force_edit_path.set(false);
    }

    // Remove from side view when we click the close button on the side panel in the UI.
    pub fn remove_from_side_view(&self, file_path: impl AsRef<Path>) {
        let file_path = file_path.as_ref();
        self.side_view.update(|side_view| {
            let file_path_vec: Vec<Arc<str>> = file_path
                .iter()
                .map(|leg| leg.to_owned_string().into())
                .collect();
            side::mutation::remove_file(side_view.clone(), &file_path_vec).ok()
        });
        self.path.file.update(|old| {
            if Path::new(old.as_ref()) == file_path {
                let parent = file_path.parent().unwrap_or("/".as_ref()).to_owned_string();
                Some(parent.into())
            } else {
                None
            }
        });
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
