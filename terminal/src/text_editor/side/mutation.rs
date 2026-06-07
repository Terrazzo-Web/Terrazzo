#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::diagnostics;

use self::diagnostics::debug_span;
use self::diagnostics::info;
use self::diagnostics::warn;
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

fn add_node(
    manager: &impl SideViewNotify,
    node: Option<&SideViewNode>,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
) -> Option<SideViewNode> {
    let _span = debug_span!("Add node", ?path).entered();
    let relative_path = make_relative_path_iterator(&path.file);
    self::add::add_node_rec(manager, path, make, relative_path, node)
}

fn remove_node(node: Option<&SideViewNode>, path: &Path) -> Option<SideViewNode> {
    let _span = debug_span!("Remove node", ?path).entered();
    let relative_path = make_relative_path_iterator(path);
    match self::remove::remove_node_rec(relative_path, node) {
        Ok(node) => node,
        Err(error) => {
            warn!("Failed to remove node: {error}");
            None
        }
    }
}

pub fn show_folder_content(
    manager: &impl SideViewNotify,
    node: Option<&SideViewNode>,
    path: &FilePath<Arc<Path>>,
    folder_content: &[FileMetadata],
) -> Option<SideViewNode> {
    info!(?path, len = folder_content.len(), "Showing folder content");
    add_node(manager, node, path, |old| {
        info!("Setting folder content: {}", old.is_some());
        let (properties, mut new_folder, notify) = if let Some(SideViewNode {
            properties,
            item: SvnItem::Folder { folder, notify },
        }) = old
        {
            (properties, (**folder).clone(), notify)
        } else {
            warn!("Old folder not found");
            return None;
        };
        for metadata in folder_content {
            let name = Path::new(metadata.name.as_ref());
            if new_folder.contains_key(name) {
                continue;
            }
            let item = if metadata.is_dir {
                SvnItem::Folder {
                    folder: Arc::default(),
                    notify: manager.watch_side_view_folder(&FilePath {
                        base: path.base.clone(),
                        file: path.file.join(name).into(),
                    }),
                }
            } else {
                SvnItem::File {
                    metadata: Arc::new(metadata.clone()),
                }
            };
            let child = SideViewNode {
                properties: SvnProperties {
                    status: SvnStatus::Show,
                },
                item,
            };
            new_folder.insert(name.into(), child.into());
        }
        Some(SideViewNode {
            properties: properties.clone(),
            item: SvnItem::Folder {
                folder: new_folder.into(),
                notify: notify.clone(),
            },
        })
    })
}

pub fn filter_active_folder_content(
    manager: &impl SideViewNotify,
    node: Option<&SideViewNode>,
    path: &FilePath<Arc<Path>>,
) -> Option<SideViewNode> {
    info!(?path, "Filter folder content");
    add_node(manager, node, path, |old| {
        info!("Setting folder content: {}", old.is_some());
        let (properties, folder, notify) = if let Some(SideViewNode {
            properties,
            item: SvnItem::Folder { folder, notify },
        }) = old
        {
            (properties, (**folder).clone(), notify)
        } else {
            warn!("Old folder not found");
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
            properties: properties.clone(),
            item: SvnItem::Folder {
                folder: new_folder.into(),
                notify: notify.clone(),
            },
        })
    })
}

fn remove_displayed(folder: Arc<SideViewList>) -> Arc<SideViewList> {
    let mut new_folder = SideViewList::default();
    for (name, child) in folder.iter() {
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
        new_folder.insert(name.clone(), child);
    }
    Arc::new(new_folder)
}

fn make_relative_path_iterator(path: &Path) -> impl Iterator<Item = &Path> {
    path.make_relative().iter().map(|c| c.as_ref())
}
