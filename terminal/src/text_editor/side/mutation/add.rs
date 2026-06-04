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
    tree: &SideViewList,
    path: &FilePath<Arc<Path>>,
    mut relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    node: impl FnOnce(Option<&Arc<SideViewNode>>) -> Option<SideViewNode>,
) -> Option<Arc<SideViewList>> {
    match relative_path.next() {
        None => add_node_rec(manager, tree, path, Path::new("/").iter().peekable(), node),
        Some(item) => {
            if relative_path.peek().is_none() {
                add_node_leaf(tree, node, item.as_ref())
            } else {
                add_node_rec_folder(manager, tree, path, relative_path, node, item.as_ref())
            }
        }
    }
}

fn add_node_leaf(
    tree: &SideViewList,
    node: impl FnOnce(Option<&Arc<SideViewNode>>) -> Option<SideViewNode>,
    child_name: &Path,
) -> Option<Arc<SideViewList>> {
    #[cfg(debug_assertions)]
    match tree.get(child_name) {
        Some(child) => match &child.item {
            SvnItem::Folder { .. } => warn!("Replace folder {child_name:?}"),
            SvnItem::File { .. } => debug!("Replace file {child_name:?}"),
        },
        None => debug!("Add new file {child_name:?}"),
    }
    let old_node = tree.get(child_name);
    let new_node = node(old_node)?;
    let mut new_tree = (*tree).clone();
    new_tree.insert(child_name.into(), new_node.into());
    Some(new_tree.into())
}

fn add_node_rec_folder(
    manager: &impl SideViewNotify,
    tree: &SideViewList,
    path: &FilePath<Arc<Path>>,
    relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    node: impl FnOnce(Option<&Arc<SideViewNode>>) -> Option<SideViewNode>,
    folder_name: &Path,
) -> Option<Arc<SideViewList>> {
    let folder = match tree.get(folder_name) {
        Some(child) => match &child.item {
            SvnItem::Folder { folder, notify: _ } => {
                debug!("Adding to folder {folder_name:?}");
                folder.clone()
            }
            SvnItem::File { .. } => {
                #[cfg(debug_assertions)]
                warn!("Replace file {folder_name:?}");
                Arc::default()
            }
        },
        None => {
            #[cfg(debug_assertions)]
            debug!("Add new folder {folder_name:?}");
            Arc::default()
        }
    };
    let folder = add_node_rec(manager, &folder, path, relative_path, node)?;
    let mut new_tree = tree.clone();
    new_tree.insert(
        folder_name.into(),
        SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Active,
            },
            item: SvnItem::Folder {
                folder,
                notify: manager.watch_side_view_folder(path).into(),
            },
        }
        .into(),
    );
    Some(new_tree.into())
}
