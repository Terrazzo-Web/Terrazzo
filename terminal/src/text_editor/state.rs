use std::path::Path;
use std::sync::Arc;

use crate::text_editor::side::SideViewNode;
use crate::tiles::state::make_state;

make_state!(
    base_path,
    Arc<Path>,
    std::sync::Arc::from(std::path::Path::new(""))
);
make_state!(
    file_path,
    Arc<Path>,
    std::sync::Arc::from(std::path::Path::new(""))
);
make_state!(side_view, Option<Arc<SideViewNode<()>>>);
make_state!(search, Arc<str>);
