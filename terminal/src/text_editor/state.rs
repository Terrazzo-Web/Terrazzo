use std::path::Path;
use std::sync::Arc;

#[cfg(feature = "server")]
use super::fsio::ROOT_FILE_PATH;
use crate::text_editor::side::SideViewNode;
use crate::tiles::state::make_state;

make_state!(base_path, Arc<Path>, ROOT_FILE_PATH.clone());
make_state!(file_path, Arc<Path>, ROOT_FILE_PATH.clone());
make_state!(side_view, Option<Arc<SideViewNode<()>>>);
make_state!(search, Arc<str>);
