#![cfg(feature = "client")]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::prelude::Ptr;

use super::SideViewNode;
use super::SvnProperties;
use super::SvnStatus;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::ROOT_FILE_PATH;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::ui::RemoveBehavior;

impl TextEditorManager {
    // Adds the given path and item to be tracked on the side view
    // - When a file is opened
    // - When a file is changed
    pub fn add_to_side_view(
        self: &Ptr<Self>,
        path: &FilePath<Arc<Path>>,
        make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    ) {
        self.side_view.update(|side_view| {
            let new_node = super::add_node(self, side_view.as_deref(), path, make);
            new_node.map(|new_node| Some(Arc::new(new_node)))
        });
        self.force_edit_path.set(false);
    }

    pub fn remove_from_side_view(
        self: &Ptr<Self>,
        path: &FilePath<Arc<Path>>,
        behavior: RemoveBehavior,
    ) {
        let file_path = path.file.as_ref();
        self.side_view.update(|side_view| {
            let new_node = match behavior {
                RemoveBehavior::HARD => super::remove_node(side_view.as_deref(), file_path),
                RemoveBehavior::SOFT => super::add_node(self, side_view.as_deref(), path, |old| {
                    old.map(|old| SideViewNode {
                        properties: SvnProperties {
                            status: SvnStatus::Show,
                        },
                        item: old.item.clone(),
                    })
                }),
            };
            new_node.map(|new_node| Some(Arc::new(new_node)))
        });
        self.path.file.update(|old| {
            if old.as_ref() == file_path {
                let parent = file_path.parent().unwrap_or_else(|| &ROOT_FILE_PATH);
                Some(parent.into())
            } else {
                None
            }
        });
    }

    pub fn live_side_view(
        self: &Ptr<Self>,
        base: &Arc<Path>,
        side_view: Option<Arc<SideViewNode<()>>>,
    ) -> Option<Arc<SideViewNode>> {
        Some(super::live::live_side_view_rec(
            self,
            base,
            PathBuf::default(),
            side_view?.as_ref(),
        ))
    }

    pub fn stored_side_view(side_view: Option<Arc<SideViewNode>>) -> Option<Arc<SideViewNode<()>>> {
        Some(super::live::stored_side_view_rec(side_view?.as_ref()))
    }
}
