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

#[nameth]
#[derive(thiserror::Error, Debug, serde::Serialize, serde::Deserialize)]
pub enum RemoveFileError {
    #[error("[{n}] File can't be a child of file {0}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    ExpectedFolder(Arc<str>),

    #[error("[{n}] Parent folder does not exist: {0}", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "P"))]
    ParentNotFound(PathBuf),

    #[error("[{n}] The file was not found", n = self.name())]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    FileNotFound,
}

pub fn remove_node_rec<'l>(
    mut relative_path: impl Iterator<Item = &'l Path>,
    node: Option<&SideViewNode>,
) -> Result<Option<SideViewNode>, RemoveFileError> {
    let next = relative_path.next();
    debug!(?next, ?node, "Remove node rec");
    let Some(next) = next else {
        return Ok(None);
    };
    let Some(node) = node else {
        return Err(RemoveFileError::ParentNotFound(next.into()));
    };
    let (folder, notify) = match &node.item {
        SvnItem::Folder { folder, notify } => (folder.clone(), notify.clone()),
        SvnItem::File { metadata } => {
            return Err(RemoveFileError::ExpectedFolder(metadata.name.clone()));
        }
    };
    Ok(Some(SideViewNode {
        properties: node.properties.clone(),
        item: SvnItem::Folder {
            folder: remove_node_rec_folder(relative_path, &folder, next)?.into(),
            notify,
        },
    }))
}

fn remove_node_rec_folder<'l>(
    relative_path: impl Iterator<Item = &'l Path>,
    folder: &SideViewList,
    folder_name: &Path,
) -> Result<SideViewList, RemoveFileError> {
    let child = remove_node_rec(relative_path, folder.get(folder_name).map(Arc::as_ref))?;
    if let Some(child) = child {
        let mut folder = folder.clone();
        folder.insert(folder_name.into(), child.into());
        Ok(folder)
    } else if folder.contains_key(folder_name) {
        let mut folder = folder.clone();
        folder.remove(folder_name);
        Ok(folder)
    } else {
        Err(RemoveFileError::FileNotFound)
    }
}
