use std::path::Path;
use std::sync::Arc;

use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;

pub fn add_node_rec<'l>(
    manager: &impl SideViewNotify,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    mut relative_path: impl Iterator<Item = &'l Path>,
    node: Option<&SideViewNode>,
) -> Option<SideViewNode> {
    let next = relative_path.next();
    debug!(?path, ?next, ?node, "Add node rec");
    let next = match next {
        None => return make(node),
        Some(item) => item,
    };
    let (mut properties, folder, notify) = parse_folder(manager, path, node);
    let (folder, status) = add_node_rec_folder(manager, path, make, relative_path, &folder, next)?;
    if status == SvnStatus::Active {
        properties.status = status;
    }
    Some(SideViewNode {
        properties,
        item: SvnItem::Folder {
            folder: folder.into(),
            notify,
        },
    })
}

fn add_node_rec_folder<'l>(
    manager: &impl SideViewNotify,
    path: &FilePath<Arc<Path>>,
    make: impl FnOnce(Option<&SideViewNode>) -> Option<SideViewNode>,
    relative_path: impl Iterator<Item = &'l Path>,
    folder: &SideViewList,
    folder_name: &Path,
) -> Option<(SideViewList, SvnStatus)> {
    let child = add_node_rec(
        manager,
        path,
        make,
        relative_path,
        folder.get(folder_name).map(Arc::as_ref),
    )?;
    let status = child.properties.status;
    let mut folder = folder.clone();
    folder.insert(folder_name.into(), child.into());
    (folder, status).into()
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
                status: SvnStatus::Active,
            },
            Default::default(),
            manager.watch_side_view_folder(path),
        ),
    }
}
