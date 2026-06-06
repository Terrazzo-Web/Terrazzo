use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::text_editor::file_path::FilePath;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;

pub fn live_side_view_rec(
    manager: &impl SideViewNotify,
    base: &Arc<Path>,
    file_path: PathBuf,
    node: &SideViewNode<()>,
) -> Arc<SideViewNode> {
    Arc::new(SideViewNode {
        properties: node.properties.clone(),
        item: match &node.item {
            SvnItem::Folder { folder, notify: () } => SvnItem::Folder {
                folder: {
                    let mut active = SideViewList::default();
                    for (name, child) in folder.iter() {
                        active.insert(
                            name.clone(),
                            live_side_view_rec(manager, base, file_path.join(name), child),
                        );
                    }
                    Arc::new(active)
                },
                notify: manager.watch_side_view_folder(&FilePath {
                    base: base.clone(),
                    file: file_path.into(),
                }),
            },
            SvnItem::File { metadata } => SvnItem::File {
                metadata: metadata.clone(),
            },
        },
    })
}

pub fn stored_side_view_rec(node: &SideViewNode) -> Arc<SideViewNode<()>> {
    Arc::new(SideViewNode {
        properties: node.properties.clone(),
        item: match &node.item {
            SvnItem::Folder { folder, notify: () } => SvnItem::Folder {
                folder: {
                    let mut active = SideViewList::default();
                    for (name, child) in folder.iter() {
                        active.insert(name.clone(), stored_side_view_rec(child));
                    }
                    Arc::new(active)
                },
                notify: (),
            },
            SvnItem::File { metadata } => SvnItem::File {
                metadata: metadata.clone(),
            },
        },
    })
}
