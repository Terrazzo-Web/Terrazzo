#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::app::App;
use crate::tiles::id::TileId;

pub fn add_tab(
    array_id: TileId,
    after_child: Option<TileId>,
) -> Result<(Arc<Tiles>, TileId), TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let new_id = TileId::new();
    let mut inserted = false;
    let tree = add_tab_aux(tree, array_id, after_child, new_id, &mut inserted)?;
    if !inserted {
        return Err(TilesStateError::TileIdNotFound(array_id));
    }
    *lock = tree.clone().into();
    Ok((tree, new_id))
}

fn add_tab_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    new_id: TileId,
    inserted: &mut bool,
) -> Result<Arc<Tiles>, TilesStateError> {
    if *inserted {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } if *id == array_id && *direction == Direction::Tabbed => {
            let new = Arc::new(Tiles::Tile(Tile {
                id: new_id,
                app: App::Default,
                remote: Default::default(),
                title: format!("New tab {new_id}").into(),
            }));
            let mut nodes = nodes.clone();
            let insert_at = after_child
                .and_then(|after_child| {
                    nodes
                        .iter()
                        .position(|node| node_id(node) == after_child)
                        .map(|index| index + 1)
                })
                .unwrap_or(nodes.len());
            nodes.insert(insert_at, new);
            *inserted = true;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: Some(new_id),
                nodes,
                floating_nodes: floating_nodes.clone(),
            })
        }
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } => {
            let nodes = nodes
                .iter()
                .map(|node| add_tab_aux(node.clone(), array_id, after_child, new_id, inserted))
                .collect::<Result<_, _>>()?;
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    Ok(Arc::new(super::FloatingTile {
                        tile: (*add_tab_aux(
                            Arc::new(floating.tile.clone()),
                            array_id,
                            after_child,
                            new_id,
                            inserted,
                        )?)
                        .clone(),
                        ..(**floating).clone()
                    }))
                })
                .collect::<Result<_, TilesStateError>>()?;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes,
                floating_nodes,
            })
        }
    })
}

pub fn node_id(node: &Tiles) -> TileId {
    match node {
        Tiles::Tile(tile) => tile.id,
        Tiles::Array { id, .. } => *id,
    }
}
