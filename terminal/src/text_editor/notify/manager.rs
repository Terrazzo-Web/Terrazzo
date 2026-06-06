#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::Ptr;
use terrazzo::prelude::diagnostics;

use self::diagnostics::warn;
use super::server_fn::NotifyResponse;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;
use crate::utils::more_path::MorePathRef as _;

pub trait SideViewNotify {
    fn watch_side_view_folder(&self, path: &FilePath<Arc<Path>>) -> OpaqueNotifyRegistration;
}

impl SideViewNotify for Ptr<TextEditorManager> {
    #[autoclone]
    fn watch_side_view_folder(&self, path: &FilePath<Arc<Path>>) -> OpaqueNotifyRegistration {
        let manager = self;
        manager
            .notify_service
            .watch_folder(path, move |event| {
                autoclone!(manager, path);
                if *event.path != path.full_path() {
                    on_child_change(&manager, &path, event)
                } else {
                    on_folder_change(&manager, &path, event)
                }
            })
            .into()
    }
}

#[autoclone]
fn on_child_change(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    event: &NotifyResponse,
) {
    let base = if path.base.is_absolute() {
        path.base.as_ref().to_path_buf()
    } else {
        Path::new("/").join(path.base.as_ref())
    };
    let relative_file_path = match event.path.strip_prefix(&base) {
        Ok(relative_file_path) => relative_file_path.to_path_buf(),
        Err(error) => {
            warn!(
                "Notify event path {:?} is not under base {:?}: {error}",
                event.path, path.base
            );
            return;
        }
    };
    let changed_path = FilePath {
        base: path.base.clone(),
        file: Arc::from(relative_file_path.clone()),
    };
    wasm_bindgen_futures::spawn_local(async move {
        autoclone!(manager, path);
        let Ok(exists) = fsio::client::file_exists(manager.remote.clone(), changed_path.clone())
            .await
            .inspect_err(|error| warn!("Failed to check file existence: {error}"))
        else {
            return;
        };
        if !exists {
            manager.remove_from_side_view(relative_file_path);
            return;
        }

        if !folder_has_shown_children(&manager.side_view.get_value_untracked(), path.file.as_ref())
        {
            return;
        }

        let Some(data) =
            fsio::client::load_file_metadata(manager.remote.clone(), changed_path.clone())
                .await
                .inspect_err(|error| warn!("Failed to load file metadata: {error}"))
                .ok()
                .flatten()
        else {
            return;
        };

        match data {
            fsio::File::TextFile { metadata, .. } | fsio::File::PdfFile { metadata, .. } => {
                manager.add_to_side_view(SvnItem::File { metadata }, &changed_path);
            }
            fsio::File::Folder(_) => {
                manager.add_to_side_view(
                    SvnItem::Folder {
                        folder: Arc::default(),
                        notify: manager.watch_side_view_folder(&changed_path),
                    },
                    &changed_path,
                );
            }
            fsio::File::Error(error) => {
                warn!("Failed to load file metadata: {error}")
            }
        }
    });
}

#[autoclone]
fn on_folder_change(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    event: &NotifyResponse,
) {
    wasm_bindgen_futures::spawn_local(async move {
        autoclone!(manager, path);
        if !fsio::client::file_exists(manager.remote.clone(), path.clone())
            .await
            .unwrap_or(true)
        {
            manager.remove_from_side_view(path.file.as_ref());
        }
    });
}

fn folder_has_shown_children(mut tree: &SideViewList, folder_path: &Path) -> bool {
    let folder_path = folder_path.make_relative();
    let mut components = folder_path.iter().peekable();
    if components.peek().is_none() {
        return tree
            .values()
            .any(|child| child.properties.status == SvnStatus::Show);
    }
    for component in components {
        let Some(child) = tree.get(Path::new(component)) else {
            return false;
        };
        let SvnItem::Folder { folder, notify: _ } = &child.item else {
            return false;
        };
        tree = folder;
    }
    tree.values()
        .any(|child| child.properties.status == SvnStatus::Show)
}
