#![cfg(feature = "client")]

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::prelude::Ptr;

use super::SideViewList;
use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::utils::more_path::MorePathRef as _;

mod add;
mod remove;

#[cfg(test)]
mod tests;

pub use self::remove::RemoveFileError;

pub fn add_node<N: Into<Option<SideViewNode>>>(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    node: impl FnOnce(Option<&Arc<SideViewNode>>) -> N,
) -> Arc<SideViewList> {
    let relative_path = path.file.as_ref().make_relative().iter().peekable();
    self::add::add_node_rec(manager, &tree, path, relative_path, |old| node(old).into())
        .unwrap_or(tree)
}

pub fn remove_node(
    tree: Arc<SideViewList>,
    path: &Path,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    let relative_path = path.make_relative().iter().peekable();
    self::remove::remove_node_rec(&tree, relative_path)
}

pub fn show_folder_content(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    folder_content: &[FileMetadata],
) -> Arc<SideViewList> {
    add_node(manager, tree, path, |old| {
        let old = old?;
        let (mut new_folder, notify) = if let SideViewNode {
            properties: SvnProperties { status: _ },
            item: SvnItem::Folder { folder, notify },
        } = old.as_ref()
        {
            ((**folder).clone(), notify.clone())
        } else {
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
        Some(SideViewNode {
            properties: old.properties.clone(),
            item: SvnItem::Folder {
                folder: new_folder.into(),
                notify,
            },
        })
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

pub fn live_side_view(
    manager: &Ptr<TextEditorManager>,
    base: &Arc<Path>,
    side_view: Arc<SideViewList<()>>,
) -> Arc<SideViewList> {
    live_side_view_rec(manager, base, PathBuf::new(), side_view)
}

fn live_side_view_rec(
    manager: &Ptr<TextEditorManager>,
    base: &Arc<Path>,
    parent_path: PathBuf,
    side_view: Arc<SideViewList<()>>,
) -> Arc<SideViewList> {
    let mut active = SideViewList::default();
    for (name, child) in side_view.iter() {
        let child = match &child.item {
            SvnItem::Folder {
                folder: children,
                notify: (),
            } => {
                let path = parent_path.join(Path::new(name.as_ref()));
                Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder {
                        folder: live_side_view_rec(manager, base, path.clone(), children.clone()),
                        notify: manager
                            .watch_side_view_folder(&FilePath {
                                base: base.clone(),
                                file: path.into(),
                            }),
                    },
                })
            }
            SvnItem::File { metadata } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::File {
                    metadata: metadata.clone(),
                },
            }),
        };
        active.insert(name.clone(), child);
    }
    Arc::new(active)
}

pub fn stored_side_view(side_view: Arc<SideViewList>) -> Arc<SideViewList<()>> {
    let mut persisted = SideViewList::default();
    for (name, child) in side_view.iter() {
        let child = match &child.item {
            SvnItem::Folder { folder, notify: _ } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::Folder {
                    folder: stored_side_view(folder.clone()),
                    notify: (),
                },
            }),
            SvnItem::File { metadata } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::File {
                    metadata: metadata.clone(),
                },
            }),
        };
        persisted.insert(name.clone(), child);
    }
    Arc::new(persisted)
}
