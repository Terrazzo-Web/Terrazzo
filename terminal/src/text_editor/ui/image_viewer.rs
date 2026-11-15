use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::SeqCst;

use terrazzo::html;
use terrazzo::prelude::*;

use super::drag::encode_query;
use super::drag::encode_query_path;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::TextEditorManager;

terrazzo_css::import_style!(pub(super) style, "image_viewer.scss");

#[html]
pub(super) fn image_viewer(manager: &TextEditorManager, path: &FilePath<Arc<Path>>) -> XElement {
    // A new URL also refreshes the image after a file watcher notification.
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let remote = serde_json::to_string(&manager.remote).expect("Serialize remote address");
    let src = format!(
        "/api/text_editor/fsio/image?base={}&file={}&remote={}&version={}",
        encode_query_path(&path.base),
        encode_query_path(&path.file),
        encode_query(&remote),
        NEXT.fetch_add(1, SeqCst),
    );
    div(
        class = style::IMAGE_SCROLL,
        img(
            class = style::IMAGE,
            src = src,
            alt = path.file.to_string_lossy().into_owned(),
            #[cfg(not(feature = "client-prod"))]
            data_viewer = "image",
        ),
    )
}
