use std::sync::Arc;

use super::SideViewList;
use super::SideViewNode;
use super::SvnStatus;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;

#[test]
fn add_file() {
    let tree = Arc::<SideViewList>::default();
    let make_file = |name: &str| SideViewNode {
        properties: SvnProperties {
            status: SvnStatus::Opened,
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
    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
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

    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
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

    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b2"), Arc::from("c3.txt")],
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
    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1")],
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
    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
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
            status: SvnStatus::Opened,
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
    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c1.txt")],
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

    let tree = super::add_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
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
    let error = super::remove_file(
        tree.clone(),
        &[
            Arc::from("a1"),
            Arc::from("b1"),
            Arc::from("c2.txt"),
            Arc::from("not_found.txt"),
        ],
    )
    .unwrap_err();
    assert_eq!(
        "[ExpectedFolder] File can't be a child of file c2.txt",
        format!("{error}")
    );

    // Remove file: ParentNotFound
    let error = super::remove_file(
        tree.clone(),
        &[
            Arc::from("a1"),
            Arc::from("b1"),
            Arc::from("c3.txt"),
            Arc::from("not_found.txt"),
        ],
    )
    .unwrap_err();
    assert_eq!(
        "[ParentNotFound] Parent folder does not exist: c3.txt",
        format!("{error}")
    );

    // Remove file: FileNotFound
    let error = super::remove_file(
        tree.clone(),
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c3.txt")],
    )
    .unwrap_err();
    assert_eq!("[FileNotFound] The file was not found", format!("{error}"));

    // Remove file
    let tree = super::remove_file(
        tree,
        &[Arc::from("a1"), Arc::from("b1"), Arc::from("c2.txt")],
    )
    .unwrap();
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
    let tree = super::remove_file(tree, &[Arc::from("a1"), Arc::from("b1")]).unwrap();
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
