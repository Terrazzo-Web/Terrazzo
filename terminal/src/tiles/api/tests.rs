#![cfg(test)]
#![cfg(feature = "server")]

use std::sync::Arc;

use super::add::tests::add_node_for_tests;
use super::*;

// TODO: finish implementing these tests. Test add vert+horz, remove, mutations
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
    assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

    let tree = add_node_for_tests(
        tree,
        Direction::Horizontal,
        TileId::for_test(1),
        Side::Before,
        &mut Some(TileId::for_test(3)),
    )
    .unwrap();
    assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

    let tree = add_node_for_tests(
        tree,
        Direction::Horizontal,
        TileId::for_test(1),
        Side::After,
        &mut Some(TileId::for_test(4)),
    )
    .unwrap();
    assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());
}
