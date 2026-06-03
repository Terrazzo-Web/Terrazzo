#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::diagnostics;
use terrazzo::prelude::*;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::debug;
use self::diagnostics::warn;
use super::SideViewList;
use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use super::mutation;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::fsio::client::list_folder;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::server_fn::EventKind;
use crate::text_editor::notify::server_fn::FileEventKind;
use crate::text_editor::notify::server_fn::NotifyResponse;
use crate::text_editor::notify::ui::NotifyRegistration;
use crate::utils::more_path::MorePath as _;

#[autoclone]
pub fn watch_side_view_folder(
    manager: &Ptr<TextEditorManager>,
    path: &Arc<Path>,
) -> Ptr<NotifyRegistration> {
    let path_vec = Arc::new(super::ui::path_vec(path));
    let folder_path: Arc<str> = path.to_owned_string().into();
    let full_path = FilePath {
        base: manager.path.base.get_value_untracked(),
        file: folder_path,
    };
    manager
        .notify_service
        .watch_folder(&full_path, move |event| {
            autoclone!(manager, full_path, path_vec);
            process_side_view_folder_event(
                manager.clone(),
                full_path.clone(),
                path_vec.clone(),
                event,
            );
        })
}

fn process_side_view_folder_event(
    manager: Ptr<TextEditorManager>,
    folder_path: FilePath<Arc<str>>,
    path_vec: Arc<Vec<Arc<str>>>,
    event: &NotifyResponse,
) {
    debug!("Side view folder notification: {event:?}");
    let EventKind::File(kind) = event.kind else {
        return;
    };
    let event_is_folder = Path::new(&event.path) == folder_path.as_deref().full_path();
    match (event_is_folder, kind) {
        // A child entry changed, or the watched folder metadata changed.
        // Re-list the folder so displayed children match the filesystem.
        (false, FileEventKind::Create | FileEventKind::Modify | FileEventKind::Delete)
        | (true, FileEventKind::Modify) => {
            spawn_local(refresh_side_view_folder(manager, folder_path, path_vec));
        }
        // The expanded folder itself was deleted. Remove that folder node from
        // the side view instead of trying to refresh children that no longer exist.
        (true, FileEventKind::Delete) => {
            manager.side_view.update(|side_view| {
                mutation::remove_file(side_view.clone(), &path_vec)
                    .inspect_err(|error| warn!("Failed to remove side view folder: {error}"))
                    .ok()
            });
        }
        // Creating the watched folder is not useful after it is already being
        // watched; error notifications are logged upstream by the notify service.
        (true, FileEventKind::Create) | (true | false, FileEventKind::Error) => {}
    }
}

async fn refresh_side_view_folder(
    manager: Ptr<TextEditorManager>,
    folder_path: FilePath<Arc<str>>,
    path_vec: Arc<Vec<Arc<str>>>,
) {
    let content = list_folder(manager.remote.clone(), folder_path.clone())
        .await
        .inspect_err(|error| warn!("Failed to refresh side view folder {folder_path:?}: {error}"));
    let Ok(Some(content)) = content else {
        return;
    };
    manager.side_view.update(|side_view| {
        Some(synchronize_folder(
            side_view.clone(),
            &path_vec,
            content.as_ref(),
        ))
    });
}

fn synchronize_folder(
    tree: Arc<SideViewList>,
    relative_path: &[Arc<str>],
    folder_content: &[FileMetadata],
) -> Arc<SideViewList> {
    mutation::update_folder(tree, relative_path, &|children| {
        let mut new_children = SideViewList::default();
        for metadata in folder_content {
            let child = match children.get(&metadata.name) {
                Some(child) => synchronize_node(child, metadata),
                None => Arc::new(displayed_child(metadata)),
            };
            new_children.insert(metadata.name.clone(), child);
        }
        for (name, child) in children.iter() {
            if child.properties.status == SvnStatus::Opened {
                new_children.insert(name.clone(), child.clone());
            }
        }
        Arc::new(new_children)
    })
}

fn synchronize_node(child: &Arc<SideViewNode>, metadata: &FileMetadata) -> Arc<SideViewNode> {
    match (&child.item, metadata.is_dir) {
        (SvnItem::Folder(children), true) => Arc::new(SideViewNode {
            properties: child.properties.clone(),
            item: SvnItem::Folder(children.clone()),
        }),
        (SvnItem::File { .. }, false) => Arc::new(SideViewNode {
            properties: child.properties.clone(),
            item: SvnItem::File {
                metadata: Arc::new(metadata.clone()),
                notify_registration: Default::default(),
            },
        }),
        _ => Arc::new(displayed_child(metadata)),
    }
}

fn displayed_child(metadata: &FileMetadata) -> SideViewNode {
    if metadata.is_dir {
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
    }
}
