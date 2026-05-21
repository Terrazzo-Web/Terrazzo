#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Side;
use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn add_node(
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let mut new_id = Some(TileId::new());
    let tree = add_node_aux(tree, with_direction, next_to, side, &mut new_id)?;
    if new_id.is_some() {
        return Err(TilesStateError::TileIdNotFound(next_to));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn add_node_aux(
    tree: Arc<Tiles>,
    with_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> Result<Arc<Tiles>, TilesStateError> {
    Ok(match &*tree {
        Tiles::Tile(node) if node.id == next_to => {
            let new = Arc::new(Tiles::Tile(Tile {
                id: new_id
                    .take()
                    .ok_or(TilesStateError::DuplicateTileId(next_to))?,
                app: node.app,
                remote: node.remote.clone(),
            }));
            Arc::new(Tiles::Array {
                id: TileId::new(),
                direction: with_direction,
                nodes: match side {
                    Side::Before => vec![new, tree],
                    Side::After => vec![tree, new],
                },
            })
        }
        Tiles::Array {
            id,
            direction,
            nodes,
        } if new_id.is_some() => {
            let mut nodes2 = Vec::with_capacity(nodes.len());
            for node in nodes {
                nodes2.extend(add_node_flatten(
                    node.clone(),
                    with_direction,
                    *direction,
                    next_to,
                    side,
                    new_id,
                )?)
            }
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                nodes: nodes2,
            })
        }
        _ => tree.clone(),
    })
}

fn add_node_flatten(
    tree: Arc<Tiles>,
    with_direction: Direction,
    flatten_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> Result<Vec<Arc<Tiles>>, TilesStateError> {
    if new_id.is_none() {
        return Ok(vec![tree]);
    }
    let tree = add_node_aux(tree, with_direction, next_to, side, new_id)?;
    if let Tiles::Array {
        id: _,
        direction,
        nodes,
    } = &*tree
        && *direction == flatten_direction
    {
        Ok(nodes.clone())
    } else {
        Ok(vec![tree])
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn add_node_for_tests(
        tree: Arc<Tiles>,
        with_direction: Direction,
        next_to: TileId,
        side: Side,
        new_id: &mut Option<TileId>,
    ) -> Result<Arc<Tiles>, TilesStateError> {
        super::add_node_aux(tree, with_direction, next_to, side, new_id)
    }
}
