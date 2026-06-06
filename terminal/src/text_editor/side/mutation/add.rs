use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;

pub fn add_node_rec(
    manager: &impl SideViewNotify,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    mut relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    node: Option<&SideViewNode>,
) -> Option<Arc<SideViewNode>> {
    debug!(?path, next = ?relative_path.peek(), ?node, "Add node rec");
    let next = match relative_path.next() {
        None => return make(node).map(Arc::new),
        Some(item) => item,
    };
    let (properties, folder, notify) = parse_folder(manager, path, node);
    Some(Arc::new(SideViewNode {
        properties,
        item: SvnItem::Folder {
            folder: Arc::new(add_node_rec_folder(
                manager,
                path,
                make,
                relative_path,
                &folder,
                next.as_ref(),
            )?),
            notify,
        },
    }))
}

fn add_node_rec_folder(
    manager: &impl SideViewNotify,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    relative_path: std::iter::Peekable<std::path::Iter<'_>>,
    folder: &SideViewList,
    folder_name: &Path,
) -> Option<SideViewList> {
    let child = add_node_rec(
        manager,
        path,
        make,
        relative_path,
        folder.get(folder_name).map(Arc::as_ref),
    )?;
    let mut folder = folder.clone();
    folder.insert(folder_name.into(), child);
    folder.into()
}

fn parse_folder(
    manager: &impl SideViewNotify,
    path: &FilePath<Arc<Path>>,
    node: Option<&SideViewNode>,
) -> (SvnProperties, Arc<SideViewList>, OpaqueNotifyRegistration) {
    match node {
        Some(node) => {
            let (folder, notify) = match &node.item {
                SvnItem::Folder { folder, notify } => (folder.clone(), notify.clone()),
                SvnItem::File { metadata: _ } => {
                    (Default::default(), manager.watch_side_view_folder(path))
                }
            };
            (node.properties.clone(), folder, notify)
        }
        None => (
            SvnProperties {
                status: SvnStatus::Show,
            },
            Default::default(),
            manager.watch_side_view_folder(path),
        ),
    }
}
