//! Conversion utils for the Notify service.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::backend::protos::terrazzo::notify::FilePath as FilePathProto;
use crate::text_editor::file_path::FilePath;
use crate::utils::more_path::MorePath as _;

impl From<FilePathProto> for FilePath<Arc<Path>> {
    fn from(proto: FilePathProto) -> Self {
        Self {
            base: PathBuf::from(proto.base).into(),
            file: PathBuf::from(proto.file).into(),
        }
    }
}

impl From<FilePath<Arc<Path>>> for FilePathProto {
    fn from(proto: FilePath<Arc<Path>>) -> Self {
        Self {
            base: proto.base.as_ref().to_owned_string(),
            file: proto.file.as_ref().to_owned_string(),
        }
    }
}
