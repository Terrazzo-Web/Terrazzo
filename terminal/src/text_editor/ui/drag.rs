use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::DragEvent;
use web_sys::File;

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::error;
use crate::api::client::request;
use crate::api::client::request::Method;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::fsio::ROOT_BASE_PATH;
use crate::text_editor::manager::TextEditorManager;

const MOVE_FILE_KEY: &str = "text-editor-move-file";

#[autoclone]
pub fn on_move_dragstart(path: &FilePath<Arc<Path>>) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(path);
        let _span = debug_span!("Drag start").entered();
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
    let _span = debug_span!("Drag over").entered();
    let Some(data_transfer) = event.data_transfer() else {
        debug!("No DataTransfer object found");
        return false;
    };
    if has_side_view_drag_key(&data_transfer) {
        debug!("Accepted draggable");
        event.prevent_default();
        data_transfer.set_drop_effect("move");
        return true;
    }
    if has_external_files(&data_transfer) {
        debug!("Accepted external files");
        event.prevent_default();
        data_transfer.set_drop_effect("copy");
        return true;
    }
    debug!("Not expected draggable");
    return false;
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

fn has_external_files(data_transfer: &web_sys::DataTransfer) -> bool {
    let types = data_transfer.types();
    for i in 0..types.length() {
        if types.get(i).as_string().as_deref() == Some("Files") {
            return true;
        }
    }
    false
}

#[autoclone]
pub(in crate::text_editor) fn on_move_drop(
    manager: &Ptr<TextEditorManager>,
    destination_folder: &FilePath<Arc<Path>>,
) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(manager, destination_folder);
        let _span = debug_span!("Drop").entered();
        debug!("Processing drop event to {destination_folder:?}");
        event.prevent_default();
        event.stop_propagation();
        let Some(data_transfer) = event.data_transfer() else {
            debug!("No DataTransfer object found");
            return;
        };
        if has_side_view_drag_key(&data_transfer) {
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

            spawn_local(
                move_side_view_node(
                    manager.clone(),
                    FilePath {
                        base: ROOT_BASE_PATH.clone(),
                        file: source_path.into(),
                    },
                    destination_folder.clone(),
                )
                .in_current_span(),
            );
        } else {
            let Some(files) = data_transfer.files() else {
                debug!("No FileList found");
                return;
            };
            if files.length() == 0 {
                debug!("No dropped files found");
                return;
            }
            debug!("Uploading {} dropped file(s)", files.length());
            for i in 0..files.length() {
                let Some(file) = files.get(i) else {
                    continue;
                };
                spawn_local(
                    upload_dropped_file(manager.clone(), destination_folder.clone(), file)
                        .in_current_span(),
                );
            }
        }
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
    debug!("Moved!")
}

async fn upload_dropped_file(
    manager: Ptr<TextEditorManager>,
    destination_folder: FilePath<Arc<Path>>,
    file: File,
) {
    let file_name = file.name();
    if file_name.is_empty() {
        debug!("Dropped file has no name; skipping upload");
        return;
    }
    let destination_file = FilePath {
        base: destination_folder.base.clone(),
        file: Arc::from(destination_folder.file.as_ref().join(&file_name)),
    };
    debug!("Uploading dropped file to {destination_file:?}");

    let url = upload_url(&destination_file);
    let result = request::send_request(Method::POST, url, move |request| {
        request.set_body(JsValue::from(file).as_ref());
    })
    .await;
    if let Err(error) = result {
        error!("Failed to upload dropped file: {error}");
        return;
    }
    debug!("Uploaded dropped file");
    if manager.path.file.get_value_untracked() == destination_folder.file {
        manager.path.file.force(destination_folder.file);
    }
}

fn upload_url(file_path: &FilePath<Arc<Path>>) -> String {
    format!(
        "/api/text_editor/fsio/upload?base={}&file={}",
        encode_query_path(file_path.base.as_ref()),
        encode_query_path(file_path.file.as_ref())
    )
}

pub fn encode_query_path(path: &Path) -> String {
    path.to_string_lossy()
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
