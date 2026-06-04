use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::SideViewList;
use super::SideViewNode;
use super::SvnStatus;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;

struct DummyManager;

impl SideViewNotify for DummyManager {
    fn watch_side_view_folder(&self, _path: &FilePath<Arc<Path>>) -> OpaqueNotifyRegistration {
        Default::default()
    }
}

fn make_test_path(path: &str) -> FilePath<Arc<Path>> {
    FilePath {
        base: "/path/from/root",
        file: path,
    }
    .map(PathBuf::from)
    .map(Arc::from)
}

#[test]
fn add_file() {
    let tree = Arc::<SideViewList>::default();
    let make_file = |name: &str| SideViewNode {
        properties: SvnProperties {
            status: SvnStatus::Active,
        },
        item: SvnItem::File {
            metadata: Arc::new(FileMetadata {
                name: Arc::from(name),
                size: Some(12),
                is_dir: false,
                created: None,
                accessed: None,
                modified: None,
                mode: None,
                user: None,
                group: None,
            }),
        },
    };
    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1/c1.txt"),
        make_file("c1.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1/c2.txt"),
        make_file("c2.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b2/c3.txt"),
        make_file("c2.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    // Folder --> File
    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1"),
        make_file("b1.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": File(
                "b1.txt",
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    // File --> Folder
    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1/c1.txt"),
        make_file("c1.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
            "b2": Folder(
                {
                    "c3.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );
}

#[test]
fn remove_file() {
    let tree = Arc::<SideViewList>::default();
    let make_file = |name: &str| SideViewNode {
        properties: SvnProperties {
            status: SvnStatus::Active,
        },
        item: SvnItem::File {
            metadata: Arc::new(FileMetadata {
                name: Arc::from(name),
                size: Some(12),
                is_dir: false,
                created: None,
                accessed: None,
                modified: None,
                mode: None,
                user: None,
                group: None,
            }),
        },
    };
    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1/c1.txt"),
        make_file("c1.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    let tree = super::add_node(
        &DummyManager,
        tree,
        &make_test_path("/a1/b1/c2.txt"),
        make_file("c2.txt"),
    );
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                    "c2.txt": File(
                        "c2.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    // Remove file: ExpectedFolder
    let error =
        super::remove_node(tree.clone(), "/a1/b1/c2.txt/not_found.txt".as_ref()).unwrap_err();
    assert_eq!(
        "[ExpectedFolder] File can't be a child of file c2.txt",
        format!("{error}")
    );

    // Remove file: ParentNotFound
    let error =
        super::remove_node(tree.clone(), "/a1/b1/c3.txt/not_found.txt".as_ref()).unwrap_err();
    assert_eq!(
        "[ParentNotFound] Parent folder does not exist: c3.txt",
        format!("{error}")
    );

    // Remove file: FileNotFound
    let error = super::remove_node(tree.clone(), "/a1/b1/c3.txt".as_ref()).unwrap_err();
    assert_eq!("[FileNotFound] The file was not found", format!("{error}"));

    // Remove file
    let tree = super::remove_node(tree, "/a1/b1/c2.txt".as_ref()).unwrap();
    assert_eq!(
        r#"
{
    "a1": Folder(
        {
            "b1": Folder(
                {
                    "c1.txt": File(
                        "c1.txt",
                    ),
                },
            ),
        },
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );

    // Remove folder
    let tree = super::remove_node(tree, "/a1/b1".as_ref()).unwrap();
    assert_eq!(
        r#"
{
    "a1": Folder(
        {},
    ),
}"#
        .trim(),
        format!("{tree:#?}")
    );
}
