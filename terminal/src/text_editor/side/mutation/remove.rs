use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;

#[nameth]
#[derive(thiserror::Error, Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoveFileError {
    #[error("[{n}] File can't be a child of file {0}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    ExpectedFolder(Arc<str>),

    #[error("[{n}] Parent folder does not exist: {0:?}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "P"))]
    ParentNotFound(PathBuf),

    #[error("[{n}] The file was not found", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    FileNotFound,
}

pub fn remove_node_rec(
    tree: Arc<SideViewList>,
    mut relative_path: std::iter::Peekable<std::path::Iter<'_>>,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    match relative_path.next() {
        None => remove_node_rec(tree, Path::new("/").iter().peekable()),
        Some(item) => {
            if relative_path.peek().is_none() {
                remove_node_leaf(tree, item.as_ref())
            } else {
                remove_node_rec_folder(tree, item.as_ref(), relative_path)
            }
        }
    }
}

fn remove_node_leaf(
    tree: Arc<SideViewList>,
    child_name: &Path,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    #[cfg(debug_assertions)]
    match tree.get(child_name) {
        Some(child) => match &child.item {
            SvnItem::Folder { .. } => debug!("Remove folder {child_name:?}"),
            SvnItem::File { .. } => debug!("Remove file {child_name:?}"),
        },
        None => {
            debug!("The file wasn't here {child_name:?}");
            return Err(RemoveFileError::FileNotFound);
        }
    }
    let mut new_tree = (*tree).clone();
    new_tree.remove(child_name);
    Ok(new_tree.into())
}

fn remove_node_rec_folder(
    tree: Arc<SideViewList>,
    folder_name: &Path,
    relative_path: std::iter::Peekable<std::path::Iter<'_>>,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    let (children, children_notify) = match tree.get(folder_name) {
        Some(child) => match &child.item {
            SvnItem::Folder { folder, notify } => {
                debug!("Removing from folder {folder_name:?}");
                (folder.clone(), notify.clone())
            }
            SvnItem::File { metadata } => {
                return Err(RemoveFileError::ExpectedFolder(metadata.name.clone()));
            }
        },
        None => {
            return Err(RemoveFileError::ParentNotFound(folder_name.to_owned()));
        }
    };
    let mut new_tree = (*tree).clone();
    let children = remove_node_rec(children, relative_path)?;
    new_tree.insert(
        folder_name.into(),
        SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Active,
            },
            item: SvnItem::Folder {
                folder: children,
                notify: children_notify,
            },
        }
        .into(),
    );
    Ok(new_tree.into())
}
