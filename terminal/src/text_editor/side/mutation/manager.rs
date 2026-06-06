#![cfg(feature = "client")]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::prelude::Ptr;

use super::SideViewNode;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::TextEditorManager;

impl TextEditorManager {
    // Adds the given path and item to be tracked on the side view
    // - When a file is opened
    // - When a file is changed
    pub fn add_to_side_view(
        self: &Ptr<Self>,
        path: &FilePath<Arc<Path>>,
        make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    ) {
        self.side_view
            .update(|side_view| super::add_node(self, &side_view, path, make).map(Arc::new));
        self.force_edit_path.set(false);
    }

    pub fn remove_from_side_view(&self, file_path: impl AsRef<Path>) {
        let file_path = file_path.as_ref();
        self.side_view
            .update(|side_view| super::remove_node(&side_view, file_path).map(Arc::new));
        self.path.file.update(|old| {
            if old.as_ref() == file_path {
                let parent = file_path.parent().unwrap_or_else(|| Path::new(""));
                Some(parent.into())
            } else {
                None
            }
        });
    }

    pub fn live_side_view(
        self: &Ptr<Self>,
        base: &Arc<Path>,
        side_view: Arc<SideViewNode<()>>,
    ) -> Arc<SideViewNode> {
        super::live::live_side_view_rec(self, base, PathBuf::default(), &side_view)
    }

    pub fn stored_side_view(side_view: Arc<SideViewNode>) -> Arc<SideViewNode<()>> {
        super::live::stored_side_view_rec(&side_view)
    }
}
