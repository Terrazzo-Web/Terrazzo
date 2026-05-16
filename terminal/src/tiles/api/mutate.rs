use std::sync::Arc;

use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn mutate_node(
    id_to_mutate: TileId,
    mutation: impl FnOnce(&Tile) -> Tile,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut maybe_mutation = Some(mutation);
    let tree = mutate_node_aux(tree, id_to_mutate, &mut maybe_mutation)?;
    if maybe_mutation.is_some() {
        return Err(TilesStateError::TileIdNotFound(id_to_mutate));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn mutate_node_aux(
    tree: Arc<Tiles>,
    id_to_mutate: TileId,
    maybe_mutation: &mut Option<impl FnOnce(&Tile) -> Tile>,
) -> Result<Arc<Tiles>, TilesStateError> {
    if maybe_mutation.is_none() {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile(node) if node.id == id_to_mutate => {
            let Some(mutation) = maybe_mutation.take() else {
                return Err(TilesStateError::DuplicateTileId(id_to_mutate));
            };
            Arc::new(Tiles::Tile(mutation(node)))
        }
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            nodes,
        } => {
            let mut nodes2 = Vec::with_capacity(nodes.len());
            for node in nodes {
                nodes2.push(mutate_node_aux(node.clone(), id_to_mutate, maybe_mutation)?);
            }
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                nodes: nodes2,
            })
        }
    })
}
