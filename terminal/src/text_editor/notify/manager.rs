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
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;
use crate::utils::more_path::MorePathRef as _;

pub trait SideViewNotify {
    fn watch_side_view_folder(&self, folder_path: &FilePath<Arc<Path>>)
    -> OpaqueNotifyRegistration;
}

impl SideViewNotify for Ptr<TextEditorManager> {
    #[autoclone]
    fn watch_side_view_folder(
        &self,
        folder_path: &FilePath<Arc<Path>>,
    ) -> OpaqueNotifyRegistration {
        let manager = self;
        manager
            .notify_service
            .watch_folder(folder_path, move |event| {
                autoclone!(manager, folder_path);
                if *event.path != folder_path.full_path() {
                    on_child_change(&manager, &folder_path, event)
                } else {
                    on_folder_change(&manager, &folder_path)
                }
            })
            .into()
    }
}

#[autoclone]
fn on_child_change(
    manager: &Ptr<TextEditorManager>,
    folder_path: &FilePath<Arc<Path>>,
    event: &NotifyResponse,
) {
    let relative_file_path = match folder_path
        .with_base_path(|base| event.path.strip_prefix(base).map(Path::to_owned))
    {
        Ok(relative_file_path) => relative_file_path,
        Err(error) => {
            warn!(
                "Notify event path {:?} is not under base {:?}: {error}",
                event.path, folder_path.base
            );
            return;
        }
    };
    let changed_path = FilePath {
        base: folder_path.base.clone(),
        file: Arc::from(relative_file_path),
    };

    wasm_bindgen_futures::spawn_local(async move {
        autoclone!(manager, folder_path);
        let Ok(exists) = fsio::client::file_exists(manager.remote.clone(), changed_path.clone())
            .await
            .inspect_err(|error| warn!("Failed to check file existence: {error}"))
        else {
            return;
        };
        if !exists {
            manager.remove_from_side_view(&changed_path.file);
            return;
        }

        if !folder_has_shown_children(
            manager.side_view.get_value_untracked().as_deref(),
            &folder_path.file,
        ) {
            return;
        }

        let Some(data) =
            fsio::client::load_file_metadata(manager.remote.clone(), changed_path.clone())
                .await
                .inspect_err(|error| warn!("Failed to load file metadata: {error}"))
                .ok()
                .flatten()
        else {
            // Note: expect the file to exist since we check above.
            return;
        };

        let item = match data {
            fsio::File::TextFile { metadata, .. } | fsio::File::PdfFile { metadata, .. } => {
                SvnItem::File { metadata }
            }
            fsio::File::Folder(_) => SvnItem::Folder {
                folder: Arc::default(),
                notify: manager.watch_side_view_folder(&changed_path),
            },
            fsio::File::Error(error) => {
                warn!("Failed to load file metadata: {error}");
                return;
            }
        };
        manager.add_to_side_view(&changed_path, |old_node| {
            Some(SideViewNode {
                properties: SvnProperties {
                    status: old_node
                        .map(|old_node| old_node.properties.status)
                        .unwrap_or(SvnStatus::Show),
                },
                item,
            })
        });
    })
}

#[autoclone]
fn on_folder_change(manager: &Ptr<TextEditorManager>, folder_path: &FilePath<Arc<Path>>) {
    wasm_bindgen_futures::spawn_local(async move {
        autoclone!(manager, folder_path);
        // TODO: Consider reloading the entire folder.
        if !fsio::client::file_exists(manager.remote.clone(), folder_path.clone())
            .await
            .unwrap_or(true)
        {
            manager.remove_from_side_view(folder_path.file.as_ref());
        }
    });
}

fn folder_has_shown_children(node: Option<&SideViewNode>, folder_path: &Path) -> bool {
    let Some(node) = node else {
        return false;
    };
    let mut tree = match &node.item {
        SvnItem::Folder { folder, notify: _ } => folder,
        SvnItem::File { .. } => return false,
    };
    for component in folder_path.make_relative() {
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
