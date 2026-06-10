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
fn add_tabbed_flattens_tabbed_arrays() {
    let tree = Arc::new(Tiles::Tile(tile(1)));

    let tree = add_node_for_tests(
        tree,
        Direction::Tabbed,
        TileId::for_test(1),
        Side::After,
        &mut Some(TileId::for_test(2)),
    )
    .unwrap();
    assert_tree(&tree, Direction::Tabbed, &[1, 2]);

    let tree = add_node_for_tests(
        tree,
        Direction::Tabbed,
        TileId::for_test(1),
        Side::Before,
        &mut Some(TileId::for_test(3)),
    )
    .unwrap();
    assert_tree(&tree, Direction::Tabbed, &[3, 1, 2]);
}

#[test]
fn tabify_add_and_reorder_tabs() {
    {
        let mut tree = state::TREE.lock().unwrap();
        *tree = Some(Arc::new(Tiles::Tile(tile(1))));
    }

    let tree = tabify::tabify_node(TileId::for_test(1)).unwrap();
    assert_tree(&tree, Direction::Tabbed, &[1]);
    assert_selected(&tree, Some(1));

    let (tree, new_id) = add_tab::add_tab(root_array_id(&tree), Some(TileId::for_test(1))).unwrap();
    assert_tree(&tree, Direction::Tabbed, &[1, i64::from(new_id)]);
    assert_selected_id(&tree, Some(new_id));

    let tree = move_child::move_child(root_array_id(&tree), None, new_id).unwrap();
    assert_tree(&tree, Direction::Tabbed, &[i64::from(new_id), 1]);

    let tree = select_child::select_child(root_array_id(&tree), Some(TileId::for_test(1))).unwrap();
    assert_selected(&tree, Some(1));

    let mut tree = state::TREE.lock().unwrap();
    *tree = None;
}

#[test]
fn missing_remote_defaults_to_local() {
    let tile = serde_json::from_value::<Tile>(serde_json::json!({
        "id": 1,
        "app": App::default(),
    }))
    .unwrap();

    assert!(tile.remote.is_empty());
    assert_eq!(None, tile.title);
}

fn tile(id: i64) -> Tile {
    Tile {
        id: TileId::for_test(id),
        app: Default::default(),
        remote: Default::default(),
        title: Default::default(),
    }
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

fn assert_selected(tree: &Tiles, expected_selected: Option<i64>) {
    assert_selected_id(tree, expected_selected.map(TileId::for_test));
}

fn assert_selected_id(tree: &Tiles, expected_selected: Option<TileId>) {
    let Tiles::Array { selected, .. } = tree else {
        panic!("expected array tree, got {tree:?}");
    };
    assert_eq!(expected_selected, *selected);
}

fn root_array_id(tree: &Tiles) -> TileId {
    let Tiles::Array { id, .. } = tree else {
        panic!("expected array tree, got {tree:?}");
    };
    *id
}
