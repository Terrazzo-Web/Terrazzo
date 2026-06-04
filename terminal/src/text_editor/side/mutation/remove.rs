use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use nameth::nameth;
use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use crate::text_editor::side::{SideViewList, SideViewNode, SvnItem, SvnProperties, SvnStatus};

#[nameth]
#[derive(thiserror::Error, Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoveFileError {
    #[error("[{n}] File can't be a child of file {0}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    ExpectedFolder(Arc<str>),

    #[error("[{n}] Parent folder does not exist: {0}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "P"))]
    ParentNotFound(Arc<str>),

    #[error("[{n}] The file was not found", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    FileNotFound,
}

pub fn remove_file_rec(
    tree: Arc<SideViewList>,
    mut relative_path: std::iter::Peekable<std::path::Iter<'_>>,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    match relative_path.next() {
        None => remove_file_rec(tree, Path::new("/").iter().peekable()),
        Some(item) => {
            if relative_path.peek().is_none() {
                remove_file_rec_file(tree, item)
            } else {
                remove_file_rec_node(tree, item, relative_path)
            }
        }
    }
}

fn remove_file_rec_file(
    tree: Arc<SideViewList>,
    child_name: &OsStr,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    let child_name = child_name.to_string_lossy();
    #[cfg(debug_assertions)]
    match tree.get(child_name.as_ref()) {
        Some(child) => match &child.item {
            SvnItem::Folder { .. } => debug!("Remove folder {child_name}"),
            SvnItem::File { .. } => debug!("Remove file {child_name}"),
        },
        None => {
            debug!("The file wasn't here {child_name}");
            return Err(RemoveFileError::FileNotFound);
        }
    }
    let mut new_tree = (**tree).clone();
    new_tree.remove(child_name.as_ref());
    Ok(Arc::new(new_tree.into()))
}

fn remove_file_rec_node(
    tree: Arc<SideViewList>,
    folder_name: &OsStr,
    relative_path: std::iter::Peekable<std::path::Iter<'_>>,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    let folder_name = folder_name.to_string_lossy();
    let (children, children_notify) = match tree.get(folder_name.as_ref()) {
        Some(child) => match &child.item {
            SvnItem::Folder { folder, notify } => {
                debug!("Removing from folder {folder_name}");
                (folder.clone(), notify.clone())
            }
            SvnItem::File { metadata } => {
                return Err(RemoveFileError::ExpectedFolder(metadata.name.clone()));
            }
        },
        None => {
            return Err(RemoveFileError::ParentNotFound(folder_name.into()));
        }
    };
    let mut new_tree = (*tree).clone();
    let children = remove_file_rec(children, relative_path)?;
    new_tree.insert(
        folder_name.into(),
        Arc::new(SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Opened,
            },
            item: SvnItem::Folder {
                folder: children,
                notify: children_notify,
            },
        }),
    );
    Ok(Arc::new(new_tree))
}
