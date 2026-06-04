use std::path::Path;
use std::sync::Arc;

use crate::text_editor::side::SideViewList;
use crate::tiles::state::make_state;

make_state!(base_path, Arc<Path>);
make_state!(file_path, Arc<Path>);
make_state!(side_view, Arc<SideViewList<()>>);
make_state!(search, Arc<str>);
