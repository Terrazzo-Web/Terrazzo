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

fn assert_tree(tree: &Tiles, expected_direction: Direction, expected_tile_ids: &[i64]) {
    let Tiles::Array {
        direction, nodes, ..
    } = tree
    else {
        panic!("expected array tree, got {tree:?}");
    };
    assert_eq!(expected_direction, *direction);
    let tile_ids = nodes
        .iter()
        .map(|node| match &**node {
            Tiles::Tile(tile) => tile.id,
            Tiles::Array { .. } => panic!("expected flattened tile, got {node:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected_tile_ids
            .iter()
            .copied()
            .map(TileId::for_test)
            .collect::<Vec<_>>(),
        tile_ids
    );
}
