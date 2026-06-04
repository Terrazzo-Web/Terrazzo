#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::Ptr;

use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::manager::TextEditorManager;

impl TextEditorManager {
    pub fn add_to_side_view(
        self: &Ptr<Self>,
        metadata: &Arc<FileMetadata>,
        path: &FilePath<Arc<Path>>,
    ) {
        self.side_view.update(|tree| {
            Some(super::mutation::add_node(
                self,
                tree.clone(),
                path,
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Active,
                    },
                    item: SvnItem::File {
                        metadata: metadata.clone(),
                    },
                },
            ))
        });
        self.force_edit_path.set(false);
    }

    // Remove from side view when we click the close button on the side panel in the UI.
    pub fn remove_from_side_view(&self, file_path: impl AsRef<Path>) {
        let file_path = file_path.as_ref();
        self.side_view
            .update(|side_view| super::mutation::remove_node(side_view.clone(), file_path).ok());
        self.path.file.update(|old| {
            if old.as_ref() == file_path {
                let parent = file_path.parent().unwrap_or_else(|| "/".as_ref());
                Some(parent.into())
            } else {
                None
            }
        });
    }
}
