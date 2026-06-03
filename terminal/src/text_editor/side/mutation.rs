#![cfg(feature = "client")]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use self::diagnostics::warn;
use super::SideViewList;
use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use crate::frontend::remotes::Remote;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::fsio::client::file_exists;
use crate::utils::more_path::MorePath as _;

pub fn add_file(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
    node: SideViewNode,
) -> Arc<SideViewList> {
    match relative_path {
        [] => add_file(tree, &["/".into()], node),
        [child_name] => {
            #[cfg(debug_assertions)]
            #[cfg(debug_assertions)]
            match tree.get(child_name) {
                Some(child) => match &child.item {
                    SvnItem::Folder(..) => warn!("Replace folder {child_name}"),
                    SvnItem::File { .. } => debug!("Replace file {child_name}"),
                },
                None => debug!("Add new file {child_name}"),
            }
            let mut new_tree = (*tree).clone();
            new_tree.insert((*child_name).clone(), Arc::new(node));
            Arc::new(new_tree)
        }
        [folder_name, rest @ ..] => {
            let children = match tree.get(folder_name) {
                Some(child) => match &child.item {
                    SvnItem::Folder(children) => {
                        debug!("Adding to folder {folder_name}");
                        children.clone()
                    }
                    SvnItem::File { .. } => {
                        warn!("Replace file {folder_name}");
                        Arc::default()
                    }
                },
                None => {
                    debug!("Add new folder {folder_name}");
                    Arc::default()
                }
            };
            let mut new_tree = (*tree).clone();
            let rec = add_file(children, rest, node);
            new_tree.insert(
                (*folder_name).clone(),
                Arc::new({
                    SideViewNode {
                        properties: SvnProperties {
                            status: SvnStatus::Opened,
                        },
                        item: SvnItem::Folder(rec),
                    }
                }),
            );
            Arc::new(new_tree)
        }
    }
}

pub fn remove_file(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
) -> Result<Arc<SideViewList>, RemoveFileError> {
    match relative_path {
        [] => remove_file(tree, &["/".into()]),
        [child_name] => remove_aux_file(&tree, child_name),
        [folder_name, rest @ ..] => remove_aux_folder(tree, folder_name, rest),
    }
}

fn remove_aux_file(
    tree: &Arc<BTreeMap<Arc<str>, Arc<SideViewNode>>>,
    child_name: &Arc<str>,
) -> Result<Arc<BTreeMap<Arc<str>, Arc<SideViewNode>>>, RemoveFileError> {
    #[cfg(debug_assertions)]
    match tree.get(child_name) {
        Some(child) => match &child.item {
            SvnItem::Folder(..) => debug!("Remove folder {child_name}"),
            SvnItem::File { .. } => debug!("Remove file {child_name}"),
        },
        None => {
            debug!("The file wasn't here {child_name}");
            return Err(RemoveFileError::FileNotFound);
        }
    }
    let mut new_tree = (**tree).clone();
    new_tree.remove(child_name);
    Ok(Arc::new(new_tree))
}

fn remove_aux_folder(
    tree: Arc<BTreeMap<Arc<str>, Arc<SideViewNode>>>,
    folder_name: &Arc<str>,
    rest: &[Arc<str>],
) -> Result<Arc<BTreeMap<Arc<str>, Arc<SideViewNode>>>, RemoveFileError> {
    let children = match tree.get(folder_name) {
        Some(child) => match &child.item {
            SvnItem::Folder(children) => {
                debug!("Removing from folder {folder_name}");
                children.clone()
            }
            SvnItem::File {
                metadata: expected_folder,
                ..
            } => {
                return Err(RemoveFileError::ExpectedFolder(
                    expected_folder.name.clone(),
                ));
            }
        },
        None => {
            return Err(RemoveFileError::ParentNotFound(folder_name.clone()));
        }
    };
    let mut new_tree = (*tree).clone();
    let new_children = remove_file(children, rest)?;
    new_tree.insert(
        folder_name.clone(),
        Arc::new(SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Opened,
            },
            item: SvnItem::Folder(new_children),
        }),
    );
    Ok(Arc::new(new_tree))
}

pub fn add_displayed_folder_content(
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
                        status: SvnStatus::Displayed,
                    },
                    item: SvnItem::Folder(Arc::default()),
                }
            } else {
                SideViewNode {
                    properties: SvnProperties {
                        status: SvnStatus::Displayed,
                    },
                    item: SvnItem::File {
                        metadata: Arc::new(metadata.clone()),
                        notify_registration: Default::default(),
                    },
                }
            };
            new_children.insert(metadata.name.clone(), Arc::new(node));
        }
        Arc::new(new_children)
    })
}

pub fn collapse_displayed_children(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
) -> Arc<SideViewList> {
    update_folder(tree, relative_path, &|children| {
        let mut new_children = SideViewList::default();
        for (name, child) in children.iter() {
            if child.properties.status == SvnStatus::Displayed {
                continue;
            }
            let child = match &child.item {
                SvnItem::Folder(grandchildren) => Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder(remove_displayed(grandchildren.clone())),
                }),
                SvnItem::File { .. } => child.clone(),
            };
            new_children.insert(name.clone(), child);
        }
        Arc::new(new_children)
    })
}

pub async fn prune_side_view(
    remote: Remote,
    base: Arc<str>,
    tree: Arc<SideViewList>,
) -> Result<Option<Arc<SideViewList>>, ServerFnError> {
    let (tree, changed) = prune_side_view_rec(remote, base, vec![], tree).await?;
    Ok(changed.then_some(tree))
}

async fn prune_side_view_rec(
    remote: Remote,
    base: Arc<str>,
    parent_path: Vec<Arc<str>>,
    tree: Arc<SideViewList>,
) -> Result<(Arc<SideViewList>, bool), ServerFnError> {
    let mut changed = false;
    let mut new_tree = SideViewList::default();
    for (name, child) in tree.iter() {
        let mut path = parent_path.clone();
        path.push(name.clone());
        let file_path = FilePath {
            base: base.clone(),
            file: side_view_path(&path),
        };
        if !file_exists(remote.clone(), file_path).await? {
            changed = true;
            continue;
        }
        let child = match &child.item {
            SvnItem::Folder(children) => {
                // recursion in an async fn requires boxing
                let (children, children_changed) = Box::pin(prune_side_view_rec(
                    remote.clone(),
                    base.clone(),
                    path,
                    children.clone(),
                ))
                .await?;
                changed |= children_changed;
                Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder(children),
                })
            }
            SvnItem::File {
                metadata,
                notify_registration,
            } => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::File {
                    metadata: metadata.clone(),
                    notify_registration: notify_registration.clone(),
                },
            }),
        };
        new_tree.insert(name.clone(), child);
    }
    Ok((Arc::new(new_tree), changed))
}

fn side_view_path(path: &[Arc<str>]) -> Arc<str> {
    path.iter()
        .fold(std::path::PathBuf::new(), |path, name| {
            path.join(Path::new(name.as_ref()))
        })
        .to_owned_string()
        .into()
}

fn remove_displayed(tree: Arc<SideViewList>) -> Arc<SideViewList> {
    let mut new_tree = SideViewList::default();
    for (name, child) in tree.iter() {
        if child.properties.status == SvnStatus::Displayed {
            continue;
        }
        let child = match &child.item {
            SvnItem::Folder(children) => Arc::new(SideViewNode {
                properties: child.properties.clone(),
                item: SvnItem::Folder(remove_displayed(children.clone())),
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
            let SvnItem::Folder(children) = &child.item else {
                return tree;
            };
            let updated_children = update_folder(children.clone(), rest, update);
            let mut new_tree = (*tree).clone();
            new_tree.insert(
                folder_name.clone(),
                Arc::new(SideViewNode {
                    properties: child.properties.clone(),
                    item: SvnItem::Folder(updated_children),
                }),
            );
            Arc::new(new_tree)
        }
    }
}

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::SideViewList;
    use super::SideViewNode;
    use super::SvnStatus;
    use crate::text_editor::fsio::FileMetadata;
    use crate::text_editor::side::SvnItem;
    use crate::text_editor::side::SvnProperties;

    #[test]
    fn add_file() {
        let tree = Arc::<SideViewList>::default();
        let make_file = |name: &str| SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Opened,
            },
            item: SvnItem::File {
                metadata: Arc::new(FileMetadata {
                    name: Arc::from(name),
                    size: Some(12),
                    is_dir: false,
                    created: None,
                    accessed: None,
                    modified: None,
                    mode: None,
                    user: None,
                    group: None,
                }),
                notify_registration: Default::default(),
            },
        };
        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
            make_file("c1.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
            make_file("c2.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b2"), Arc::from("c3.txt")],
            make_file("c2.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        // Folder --> File
        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1")],
            make_file("b1.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": File(
                "b1.txt",
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        // File --> Folder
        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
            make_file("c1.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );
    }

    #[test]
    fn remove_file() {
        let tree = Arc::<SideViewList>::default();
        let make_file = |name: &str| SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Opened,
            },
            item: SvnItem::File {
                metadata: Arc::new(FileMetadata {
                    name: Arc::from(name),
                    size: Some(12),
                    is_dir: false,
                    created: None,
                    accessed: None,
                    modified: None,
                    mode: None,
                    user: None,
                    group: None,
                }),
                notify_registration: Default::default(),
            },
        };
        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
            make_file("c1.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        let tree = super::add_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
            make_file("c2.txt"),
        );
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        // Remove file: ExpectedFolder
        let error = super::remove_file(
            tree.clone(),
            &[
                Arc::from("a1"),
                Arc::from("b1"),
                Arc::from("c2.txt"),
                Arc::from("not_found.txt"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            "[ExpectedFolder] File can't be a child of file c2.txt",
            format!("{error}")
        );

        // Remove file: ParentNotFound
        let error = super::remove_file(
            tree.clone(),
            &[
                Arc::from("a1"),
                Arc::from("b1"),
                Arc::from("c3.txt"),
                Arc::from("not_found.txt"),
            ],
        )
        .unwrap_err();
        assert_eq!(
            "[ParentNotFound] Parent folder does not exist: c3.txt",
            format!("{error}")
        );

        // Remove file: FileNotFound
        let error = super::remove_file(
            tree.clone(),
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c3.txt")],
        )
        .unwrap_err();
        assert_eq!("[FileNotFound] The file was not found", format!("{error}"));

        // Remove file
        let tree = super::remove_file(
            tree,
            &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
        )
        .unwrap();
        assert_eq!(
            r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );

        // Remove folder
        let tree = super::remove_file(tree, &[Arc::from("a1"), Arc::from("b1")]).unwrap();
        assert_eq!(
            r#"
{
    "a1": Folder(
        {},
    ),
}"#
            .trim(),
            format!("{tree:#?}")
        );
    }
}
