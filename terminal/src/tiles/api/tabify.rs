#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn tabify_node(id_to_tabify: TileId) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut found = false;
    let tree = tabify_node_aux(tree, id_to_tabify, &mut found, false);
    if !found {
        return Err(TilesStateError::TileIdNotFound(id_to_tabify));
    }
    *lock = tree.clone().into();
    Ok(tree)
}

fn tabify_node_aux(
    tree: Arc<Tiles>,
    id_to_tabify: TileId,
    found: &mut bool,
    in_tabbed_array: bool,
) -> Arc<Tiles> {
    if *found {
        return tree;
    }
    match &*tree {
        Tiles::Tile(tile) if tile.id == id_to_tabify => {
            *found = true;
            if in_tabbed_array {
                tree
            } else {
                Arc::new(Tiles::Array {
                    id: TileId::new(),
                    direction: Direction::Tabbed,
                    selected: Some(tile.id),
                    nodes: vec![tree.clone()],
                })
            }
        }
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            selected: _,
            nodes: _,
        } if *id == id_to_tabify => {
            *found = true;
            tree
        }
        Tiles::Array {
            id,
            direction,
            selected,
            nodes,
        } => Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            selected: *selected,
            nodes: nodes
                .iter()
                .map(|node| {
                    tabify_node_aux(
                        node.clone(),
                        id_to_tabify,
                        found,
                        in_tabbed_array || *direction == Direction::Tabbed,
                    )
                })
                .collect(),
        }),
    }
}
