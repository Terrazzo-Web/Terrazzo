#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::diagnostics;

use self::diagnostics::*;
use super::SideViewList;
use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::utils::more_path::MorePathRef as _;

mod add;
mod live;
mod manager;
mod remove;

#[cfg(test)]
mod tests;

pub use self::remove::RemoveFileError;

fn add_node(
    manager: &impl SideViewNotify,
    node: Arc<SideViewNode>,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
) -> Option<Arc<SideViewNode>> {
    let _span = debug_span!("Add node", ?path).entered();
    let relative_path = path.file.as_ref().make_relative().iter().peekable();
    self::add::add_node_rec(manager, path, make, relative_path, Some(&node))
}

fn remove_node(node: Arc<SideViewNode>, path: &Path) -> Result<Arc<SideViewList>, RemoveFileError> {
    let _span = debug_span!("Remove node", ?path).entered();
    let relative_path = path.make_relative().iter().peekable();
    self::remove::remove_node_rec(&node, relative_path)
}

pub fn show_folder_content(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    folder_content: &[FileMetadata],
) -> Arc<SideViewList> {
    let _span = debug_span!("Show folder content", ?path).entered();
    info!("Showing {} files", folder_content.len());
    add_node(manager, tree, path, |old| {
        info!("Setting folder content: {}", old.is_some());
        let old = old?;
        let (mut new_folder, notify) = if let SideViewNode {
            properties: SvnProperties { status: _ },
            item: SvnItem::Folder { folder, notify },
        } = old.as_ref()
        {
            ((**folder).clone(), notify.clone())
        } else {
            warn!("Old folder not found");
            return None;
        };
        for metadata in folder_content {
            let name = Path::new(metadata.name.as_ref());
            if new_folder.contains_key(name) {
                continue;
            }
            let node = if metadata.is_dir {
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Show,
                    },
                    item: SvnItem::Folder {
                        folder: Arc::default(),
                        notify: manager.watch_side_view_folder(&FilePath {
                            base: path.base.clone(),
                            file: path.file.join(name).into(),
                        }),
                    },
                }
            } else {
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Show,
                    },
                    item: SvnItem::File {
                        metadata: Arc::new(metadata.clone()),
                    },
                }
            };
            new_folder.insert(name.into(), node.into());
        }
        SideViewNode {
            properties: old.properties.clone(),
            item: SvnItem::Folder {
                folder: new_folder.into(),
                notify,
            },
        }
    })
}

pub fn filter_active_folder_content(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
) -> Arc<SideViewList> {
    add_node(manager, tree, path, |old| {
        let old = old?;
        let (folder, notify) = if let SideViewNode {
            properties: SvnProperties { status: _ },
            item: SvnItem::Folder { folder, notify },
        } = old.as_ref()
        {
            (folder, notify.clone())
        } else {
            return None;
        };
        let mut new_folder = SideViewList::default();
        for (name, child) in folder.iter() {
            if child.properties.status == SvnStatus::Show {
                continue;
            }
            let child = match &child.item {
                SvnItem::Folder {
                    folder: sub_folder,
                    notify,
                } => Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder {
                        folder: remove_displayed(sub_folder.clone()),
                        notify: notify.clone(),
                    },
                }),
                SvnItem::File { .. } => child.clone(),
            };
            new_folder.insert(name.clone(), child);
        }
        Some(SideViewNode {
            properties: old.properties.clone(),
            item: SvnItem::Folder {
                folder: new_folder.into(),
                notify,
            },
        })
    })
}

fn remove_displayed(tree: Arc<SideViewList>) -> Arc<SideViewList> {
    let mut new_tree = SideViewList::default();
    for (name, child) in tree.iter() {
        if child.properties.status == SvnStatus::Show {
            continue;
        }
        let child = match &child.item {
            SvnItem::Folder { folder, notify } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::Folder {
                    folder: remove_displayed(folder.clone()),
                    notify: notify.clone(),
                },
            }),
            SvnItem::File { .. } => child.clone(),
        };
        new_tree.insert(name.clone(), child);
    }
    Arc::new(new_tree)
}
