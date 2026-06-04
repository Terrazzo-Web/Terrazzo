#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::Ptr;

use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;

use super::server_fn::NotifyResponse;

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
                    // TODO: add/delete the affected file
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
