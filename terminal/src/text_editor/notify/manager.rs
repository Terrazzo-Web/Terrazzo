#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::Ptr;

use super::server_fn::NotifyResponse;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;

pub trait SideViewNotify {
    fn watch_side_view_folder(&self, path: &FilePath<Arc<Path>>) -> OpaqueNotifyRegistration;
}

impl SideViewNotify for Ptr<TextEditorManager> {
    #[autoclone]
    fn watch_side_view_folder(&self, path: &FilePath<Arc<Path>>) -> OpaqueNotifyRegistration {
        let manager = self;
        manager
            .notify_service
            .watch_folder(path, move |event: &NotifyResponse| {
                autoclone!(manager, path);

                if *event.path != path.full_path() {
                    /*
                    TODO: add/delete the changed file.
                    In this case it means one of the files in the current folder being watched as changed.
                    The folder path is `path` adn the changed file path is `event.path`.
                    The file relative path is relative_file_path, which is event.path without the path.base prefix / path.base should be a prefix, warn and return if not.

                    Process:
                    1. if the changed file exists (as in fsio::client::file_exists)
                       1.1. if the current folder has any child.properties.status == SvnStatus::Show items
                            // This means all files in the folder are showed, not just the active ones
                            1.1.1. add it to the list using manager.add_to_side_view
                       1.2. else
                            1.2.1. no-op
                    2. else
                       2.1. remove it from the list using manager.remove_from_side_view(relative_file_path);

                    For case 1.1.1., you need to add a new method load_file_metadata.
                    It has the same signature as:

                        pub async fn load_file(
                            remote: Remote,
                            path: FilePath<Arc<Path>>,
                        ) -> Result<Option<super::File>, ServerFnError> {
                            super::load_file(remote, path).await
                        }

                    but it does not load the file contents (so just return empty file contents and skip reading from disk)
                    */
                }

                // Remove from side view on deletion notification.
                wasm_bindgen_futures::spawn_local(async move {
                    autoclone!(manager, path);
                    if !fsio::client::file_exists(manager.remote.clone(), path.clone())
                        .await
                        .unwrap_or(true)
                    {
                        manager.remove_from_side_view(path.file.as_ref());
                    }
                });
            })
            .into()
    }
}
