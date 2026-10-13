#![cfg(test)]
#![cfg(feature = "server")]

use std::sync::Arc;

use super::add::tests::add_node_for_tests;
use super::*;

#[test]
fn add_remove() {
    let tree = Arc::new(Tiles::Tile(Tile {
        id: TileId::for_test(1),
        app: Default::default(),
        remote: Default::default(),
        title: Default::default(),
    }));

    let tree = add_node_for_tests(
        tree,
        Direction::Horizontal,
        TileId::for_test(1),
        Side::After,
        &mut Some(TileId::for_test(2)),
    )
    .unwrap();
    assert_tree(&tree, Direction::Horizontal, &[1, 2]);

    let tree = add_node_for_tests(
        tree,
        Direction::Horizontal,
        TileId::for_test(1),
        Side::Before,
        &mut Some(TileId::for_test(3)),
    )
    .unwrap();
    assert_tree(&tree, Direction::Horizontal, &[3, 1, 2]);

    let tree = add_node_for_tests(
        tree,
        Direction::Horizontal,
        TileId::for_test(1),
        Side::After,
        &mut Some(TileId::for_test(4)),
    )
    .unwrap();
    assert_tree(&tree, Direction::Horizontal, &[3, 1, 4, 2]);
}

#[test]
fn split_preserves_app() {
    let apps = [
        App::Default,
        #[cfg(feature = "terminal")]
        App::Terminal,
        #[cfg(feature = "text-editor")]
        App::TextEditor,
        #[cfg(feature = "converter")]
        App::Converter,
        #[cfg(feature = "port-forward")]
        App::PortForward,
    ];
    for app in apps {
        for direction in [
            Direction::Horizontal,
            Direction::Vertical,
            Direction::Tabbed,
        ] {
            for side in [Side::Before, Side::After] {
                let tree = Arc::new(Tiles::Tile(Tile {
                    id: TileId::for_test(1),
                    app,
                    remote: Default::default(),
                    title: "Original".into(),
                }));
                let tree = add_node_for_tests(
                    tree,
                    direction,
                    TileId::for_test(1),
                    side,
                    &mut Some(TileId::for_test(2)),
                )
                .unwrap();
                let Tiles::Array { nodes, .. } = &*tree else {
                    panic!("expected split tree, got {tree:?}");
                };
                for node in nodes {
                    let Tiles::Tile(tile) = &**node else {
                        panic!("expected tile, got {node:?}");
                    };
                    let expected_app =
                        if tile.id == TileId::for_test(2) && direction == Direction::Tabbed {
                            App::Default
                        } else {
                            app
                        };
                    assert_eq!(tile.app, expected_app);
                }
            }
        }
    }
}

#[test]
fn missing_remote_defaults_to_local() {
    let tile = serde_json::from_value::<Tile>(serde_json::json!({
        "id": 1,
        "app": App::default(),
    }))
    .unwrap();

    assert!(tile.remote.is_empty());
    assert_eq!("", tile.title.as_ref());
}

fn assert_tree(tree: &Tiles, expected_direction: Direction, expected_tile_ids: &[i64]) {
    let Tiles::Array {
        direction, nodes, ..
    } = tree
    else {
        panic!("expected array tree, got {tree:?}");
    };
    assert_eq!(expected_direction, *direction);
    let tile_ids = nodes.iter().map(|node| node.id()).collect::<Vec<_>>();
    assert_eq!(
        expected_tile_ids
            .iter()
            .copied()
            .map(TileId::for_test)
            .collect::<Vec<_>>(),
        tile_ids
    );
}
