use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use self::diagnostics::warn;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;

pub fn add_node_rec(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    mut relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    node: SideViewNode,
) -> Arc<SideViewList> {
    match relative_path.next() {
        None => add_node_rec(manager, tree, path, Path::new("/").iter().peekable(), node),
        Some(item) => {
            if relative_path.peek().is_none() {
                add_node_leaf(tree, node, item)
            } else {
                add_node_rec_folder(manager, tree, path, relative_path, node, item)
            }
        }
    }
}

fn add_node_leaf(
    tree: Arc<SideViewList>,
    node: SideViewNode,
    child_name: &OsStr,
) -> Arc<SideViewList> {
    let child_name = child_name.to_string_lossy();
    #[cfg(debug_assertions)]
    match tree.get(child_name.as_ref()) {
        Some(child) => match &child.item {
            SvnItem::Folder { .. } => warn!("Replace folder {child_name}"),
            SvnItem::File { .. } => debug!("Replace file {child_name}"),
        },
        None => debug!("Add new file {child_name}"),
    }
    let mut new_tree = (*tree).clone();
    new_tree.insert(child_name.into(), Arc::new(node));
    Arc::new(new_tree)
}

fn add_node_rec_folder(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    node: SideViewNode,
    folder_name: &OsStr,
) -> Arc<SideViewList> {
    let folder_name = folder_name.to_string_lossy();
    let folder = match tree.get(folder_name.as_ref()) {
        Some(child) => match &child.item {
            SvnItem::Folder { folder, notify: _ } => {
                debug!("Adding to folder {folder_name}");
                folder.clone()
            }
            SvnItem::File { .. } => {
                #[cfg(debug_assertions)]
                warn!("Replace file {folder_name}");
                Arc::default()
            }
        },
        None => {
            #[cfg(debug_assertions)]
            debug!("Add new folder {folder_name}");
            Arc::default()
        }
    };
    let mut new_tree = (*tree).clone();
    let folder = add_node_rec(manager, folder, path, relative_path, node);
    new_tree.insert(
        folder_name.into(),
        Arc::new({
            SideViewNode {
                properties: SvnProperties {
                    status: SvnStatus::Active,
                },
                item: SvnItem::Folder {
                    folder,
                    notify: manager.watch_side_view_folder(path).into(),
                },
            }
        }),
    );
    Arc::new(new_tree)
}
