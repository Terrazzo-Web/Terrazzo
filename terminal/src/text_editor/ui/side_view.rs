use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::prelude::*;

use super::super::manager::TextEditorManager;
use super::super::notify::manager::SideViewNotify as _;
use super::super::side::SideViewList;
use super::super::side::SideViewNode;
use super::super::side::SvnItem;
use super::super::side::SvnProperties;
use super::super::side::SvnStatus;
use super::super::side::opaque::OpaqueNotifyRegistration;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;

#[derive(Default)]
struct SideViewTreeNode {
    children: HashMap<Arc<Path>, SideViewTreeNode>,
    metadata: Option<Arc<FileMetadata>>,
}

pub fn side_view(
    manager: &Ptr<TextEditorManager>,
    base: &Arc<Path>,
    files: &[FileMetadata],
) -> Arc<SideViewNode> {
    let mut root = SideViewTreeNode::default();
    for metadata in files {
        let path = Path::new(metadata.name.as_ref());
        let mut node = &mut root;
        for component in path.iter() {
            node = node
                .children
                .entry(Arc::from(Path::new(component)))
                .or_default();
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        node.metadata = Some(Arc::new(FileMetadata {
            name: name.to_string_lossy().into_owned().into(),
            ..metadata.clone()
        }));
    }

    let root_path = FilePath {
        base: base.clone(),
        file: Arc::from(PathBuf::new()),
    };
    let notify = manager.watch_side_view_folder(&root_path);
    Arc::new(side_view_tree_node(root, notify))
}

fn side_view_tree_node(node: SideViewTreeNode, notify: OpaqueNotifyRegistration) -> SideViewNode {
    if let Some(metadata) = node.metadata
        && node.children.is_empty()
    {
        return SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Active,
            },
            item: SvnItem::File { metadata },
        };
    }
    SideViewNode {
        properties: SvnProperties {
            status: SvnStatus::Active,
        },
        item: SvnItem::Folder {
            folder: Arc::new(
                node.children
                    .into_iter()
                    .map(|(name, child)| {
                        (name, Arc::new(side_view_tree_node(child, notify.clone())))
                    })
                    .collect::<SideViewList>(),
            ),
            notify,
        },
    }
}
