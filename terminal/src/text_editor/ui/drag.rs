use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::DragEvent;

use self::diagnostics::debug;
use self::diagnostics::error;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::fsio::ROOT_BASE_PATH;
use crate::text_editor::manager::TextEditorManager;

const MOVE_FILE_KEY: &str = "text-editor-move-file";

#[autoclone]
pub fn on_move_dragstart(path: &FilePath<Arc<Path>>) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(path);
        let Some(data_transfer) = event.data_transfer() else {
            return;
        };
        debug!("Dragging source file:{path:?}");
        event.stop_propagation();
        let _ = data_transfer.set_data(MOVE_FILE_KEY, &path.full_path().to_string_lossy());
        data_transfer.set_effect_allowed("move");
    }
}

pub fn on_move_dragover(event: DragEvent) -> bool {
    let Some(data_transfer) = event.data_transfer() else {
        return false;
    };
    if !has_side_view_drag_key(&data_transfer) {
        return false;
    }
    event.prevent_default();
    data_transfer.set_drop_effect("move");
    return true;
}

fn has_side_view_drag_key(data_transfer: &web_sys::DataTransfer) -> bool {
    let types = data_transfer.types();
    for i in 0..types.length() {
        if types.get(i).as_string().as_deref() == Some(MOVE_FILE_KEY) {
            return true;
        }
    }
    false
}

#[autoclone]
pub(in super::super) fn on_move_drop(
    manager: &Ptr<TextEditorManager>,
    destination_folder: &FilePath<Arc<Path>>,
) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(manager, destination_folder);
        debug!("Processing drop event to {destination_folder:?}");
        event.prevent_default();
        event.stop_propagation();
        let Some(data_transfer) = event.data_transfer() else {
            debug!("No DataTransfer object found");
            return;
        };
        let Ok(source_file) = data_transfer.get_data(MOVE_FILE_KEY) else {
            debug!("Data not found for key {MOVE_FILE_KEY}");
            return;
        };
        if source_file.is_empty() {
            debug!("Source file is empty!");
            return;
        }
        debug!("Source file:'{source_file}' to Destination folder:'{destination_folder:?}'");

        let source_path = Path::new(&source_file);
        let destination_path = destination_folder.full_path();
        if source_path.parent() == Some(&destination_path)
            || source_path == destination_path
            || destination_path.starts_with(source_path)
        {
            debug!("No-op move!");
            return;
        }

        spawn_local(move_side_view_node(
            manager.clone(),
            FilePath {
                base: ROOT_BASE_PATH.clone(),
                file: source_path.into(),
            },
            destination_folder.clone(),
        ));
    }
}

async fn move_side_view_node(
    manager: Ptr<TextEditorManager>,
    source: FilePath<Arc<Path>>,
    destination_folder: FilePath<Arc<Path>>,
) {
    let result = fsio::client::move_file(manager.remote.clone(), source, destination_folder).await;
    if let Err(error) = result {
        error!("Failed to move side-view entry: {error}");
        return;
    }
}
