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
use crate::utils::more_path::MorePath as _;
use crate::utils::more_path::MorePathRef as _;

mod add;
mod remove;

#[cfg(test)]
mod tests;

pub use self::remove::RemoveFileError;

pub fn add_node(
    manager: &impl SideViewNotify,
    tree: Arc<SideViewList>,
    path: &FilePath<Arc<Path>>,
    node: SideViewNode,
) -> Arc<SideViewList> {
    let relative_path = path.file.as_ref().make_relative().iter().peekable();
    self::add::add_node_rec(manager, tree, path, relative_path, node)
}

pub fn remove_node(
    tree: Arc<SideViewList>,
    path: &Path,
) -> Result<Arc<SideViewList>, RemoveFileError> {
    let relative_path = path.make_relative().iter().peekable();
    self::remove::remove_node_rec(tree, relative_path)
}

pub fn expand_folder_content(
    manager: &Ptr<TextEditorManager>,
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
    folder_content: &[FileMetadata],
) -> Arc<SideViewList> {
    update_folder(tree, relative_path, &|children| {
        let mut new_children = (*children).clone();
        for metadata in folder_content {
            if new_children.contains_key(&metadata.name) {
                continue;
            }
            let node = if metadata.is_dir {
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Show,
                    },
                    item: SvnItem::Folder {
                        folder: Arc::default(),
                        notify: manager
                            .watch_side_view_file(&FilePath {
                                base: manager.path.base.get_value_untracked(),
                                file: child_path(relative_path, &metadata.name),
                            })
                            .into(),
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
            new_children.insert(metadata.name.clone(), Arc::new(node));
        }
        Arc::new(new_children)
    })
}

fn child_path(parent: &[Arc<str>], name: &Arc<str>) -> Arc<str> {
    parent
        .iter()
        .chain(std::iter::once(name))
        .fold(PathBuf::new(), |path, name| {
            path.join(Path::new(name.as_ref()))
        })
        .to_owned_string()
        .into()
}

pub fn collapse_displayed_children(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
) -> Arc<SideViewList> {
    update_folder(tree, relative_path, &|children| {
        let mut new_children = SideViewList::default();
        for (name, child) in children.iter() {
            if child.properties.status == SvnStatus::Show {
                continue;
            }
            let child = match &child.item {
                SvnItem::Folder {
                    folder: grandchildren,
                    notify: notify_registration,
                } => Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder {
                        folder: remove_displayed(grandchildren.clone()),
                        notify: notify_registration.clone(),
                    },
                }),
                SvnItem::File { .. } => child.clone(),
            };
            new_children.insert(name.clone(), child);
        }
        Arc::new(new_children)
    })
}

pub fn side_view_notify_registrations(
    manager: &Ptr<TextEditorManager>,
    base: Arc<str>,
    side_view: Arc<SideViewList>,
) -> Arc<SideViewList> {
    side_view_notify_registrations_rec(manager, &base, PathBuf::new(), side_view)
}

fn side_view_notify_registrations_rec(
    manager: &Ptr<TextEditorManager>,
    base: &Arc<str>,
    parent_path: PathBuf,
    side_view: Arc<SideViewList>,
) -> Arc<SideViewList> {
    let mut recovered = SideViewList::default();
    for (name, child) in side_view.iter() {
        let path = parent_path.join(Path::new(name.as_ref()));
        let child = match &child.item {
            SvnItem::Folder {
                folder: children,
                notify: _,
            } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::Folder {
                    folder: side_view_notify_registrations_rec(
                        manager,
                        base,
                        path.clone(),
                        children.clone(),
                    ),
                    notify: manager
                        .watch_side_view_file(&FilePath {
                            base: base.clone(),
                            file: path.to_owned_string().into(),
                        })
                        .into(),
                },
            }),
            SvnItem::File { metadata, .. } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::File {
                    metadata: metadata.clone(),
                },
            }),
        };
        recovered.insert(name.clone(), child);
    }
    Arc::new(recovered)
}

fn remove_displayed(tree: Arc<SideViewList>) -> Arc<SideViewList> {
    let mut new_tree = SideViewList::default();
    for (name, child) in tree.iter() {
        if child.properties.status == SvnStatus::Show {
            continue;
        }
        let child = match &child.item {
            SvnItem::Folder {
                folder: children,
                notify: notify_registration,
            } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::Folder {
                    folder: remove_displayed(children.clone()),
                    notify: notify_registration.clone(),
                },
            }),
            SvnItem::File { .. } => child.clone(),
        };
        new_tree.insert(name.clone(), child);
    }
    Arc::new(new_tree)
}

pub fn update_folder(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
    update: &impl Fn(Arc<SideViewList>) -> Arc<SideViewList>,
) -> Arc<SideViewList> {
    match relative_path {
        [] => update(tree),
        [folder_name, rest @ ..] => {
            let Some(child) = tree.get(folder_name) else {
                return tree;
            };
            let SvnItem::Folder {
                folder: children,
                notify: notify_registration,
            } = &child.item
            else {
                return tree;
            };
            let updated_children = update_folder(children.clone(), rest, update);
            let mut new_tree = (*tree).clone();
            new_tree.insert(
                folder_name.clone(),
                Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder {
                        folder: updated_children,
                        notify: notify_registration.clone(),
                    },
                }),
            );
            Arc::new(new_tree)
        }
    }
}
