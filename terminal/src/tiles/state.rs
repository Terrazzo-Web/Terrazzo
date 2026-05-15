#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use nameth::NamedEnumValues as _;
use nameth::nameth;

use super::id::TileId;
use crate::tiles::api::Direction;
use crate::tiles::api::Side;
use crate::tiles::api::Tile;
use crate::tiles::api::TileTree;

static STATE: Mutex<Option<HashMap<TileId, Vec<Box<dyn Fn(TileId) + Send + Sync>>>>> =
    Mutex::new(None);

pub static TREE: Mutex<Option<Arc<TileTree>>> = Mutex::new(None);

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TilesStateError {
    #[error("[{n}] The mutex was poisoned", n = self.name())]
    PoisonError,

    #[error("[{n}] The tile {0:?} was not found", n = self.name())]
    TileIdNotFound(TileId),

    #[error("[{n}] The tile {0:?} was found twice", n = self.name())]
    DuplicateTileId(TileId),
}

// Add node

pub fn add_node(
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> Result<Arc<TileTree>, TilesStateError> {
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
    tree: Arc<TileTree>,
    with_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> Result<Arc<TileTree>, TilesStateError> {
    Ok(match &*tree {
        TileTree::Tile(node) if node.id == next_to => {
            let new = Arc::new(TileTree::Tile(Tile {
                id: new_id
                    .take()
                    .ok_or_else(|| TilesStateError::DuplicateTileId(next_to))?,
                app: node.app,
                remote: node.remote.clone(),
            }));
            Arc::new(TileTree::Array {
                id: TileId::new(),
                direction: with_direction,
                nodes: match side {
                    Side::Before => vec![new, tree],
                    Side::After => vec![tree, new],
                },
            })
        }
        TileTree::Array {
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
            Arc::new(TileTree::Array {
                id: *id,
                direction: *direction,
                nodes: nodes2,
            })
        }
        _ => tree.clone(),
    })
}

fn add_node_flatten(
    tree: Arc<TileTree>,
    with_direction: Direction,
    flatten_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> Result<Vec<Arc<TileTree>>, TilesStateError> {
    if new_id.is_none() {
        return Ok(vec![tree]);
    }
    let tree = add_node_aux(tree, with_direction, next_to, side, new_id)?;
    if let TileTree::Array {
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

// Remove node

pub fn remove_node(id_to_remove: TileId) -> Result<Arc<TileTree>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let mut maybe_id_to_remove = Some(id_to_remove);
    let tree = remove_node_aux(tree, &mut maybe_id_to_remove).unwrap_or_default();
    if let Some(id_to_remove) = maybe_id_to_remove {
        return Err(TilesStateError::TileIdNotFound(id_to_remove));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn remove_node_aux(
    tree: Arc<TileTree>,
    maybe_id_to_remove: &mut Option<TileId>,
) -> Option<Arc<TileTree>> {
    let Some(id_to_remove) = maybe_id_to_remove else {
        return Some(tree);
    };
    match &*tree {
        TileTree::Tile(node) if node.id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        TileTree::Array { id, .. } if *id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        TileTree::Tile { .. } => Some(tree),
        TileTree::Array {
            id,
            direction,
            nodes,
        } => {
            let nodes = nodes
                .iter()
                .filter_map(|node| remove_node_aux(node.clone(), maybe_id_to_remove))
                .collect::<Vec<_>>();
            if nodes.len() <= 1 {
                nodes.into_iter().next()
            } else {
                Some(Arc::new(TileTree::Array {
                    id: *id,
                    direction: *direction,
                    nodes,
                }))
            }
        }
    }
}

// Mutate node

pub fn mutate_node(
    id_to_mutate: TileId,
    mutation: impl FnOnce(&Tile) -> Tile,
) -> Result<Arc<TileTree>, TilesStateError> {
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
    tree: Arc<TileTree>,
    id_to_mutate: TileId,
    maybe_mutation: &mut Option<impl FnOnce(&Tile) -> Tile>,
) -> Result<Arc<TileTree>, TilesStateError> {
    if maybe_mutation.is_none() {
        return Ok(tree);
    }
    Ok(match &*tree {
        TileTree::Tile(node) if node.id == id_to_mutate => {
            let Some(mutation) = maybe_mutation.take() else {
                return Err(TilesStateError::DuplicateTileId(id_to_mutate));
            };
            Arc::new(TileTree::Tile(mutation(node)))
        }
        TileTree::Tile { .. } => tree,
        TileTree::Array {
            id,
            direction,
            nodes,
        } => {
            let mut nodes2 = Vec::with_capacity(nodes.len());
            for node in nodes {
                nodes2.push(mutate_node_aux(node.clone(), id_to_mutate, maybe_mutation)?);
            }
            Arc::new(TileTree::Array {
                id: *id,
                direction: *direction,
                nodes: nodes2,
            })
        }
    })
}

impl Drop for Tile {
    fn drop(&mut self) {
        fn aux(this: &mut Tile) -> Option<()> {
            let mut state = STATE.lock().unwrap();
            let state = state.as_mut()?;
            let drop_fns = state.remove(&this.id)?;
            for drop_fn in drop_fns {
                drop_fn(this.id)
            }
            Some(())
        }
        let _ = aux(self);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::tiles::api::Direction;
    use crate::tiles::api::Side;
    use crate::tiles::api::Tile;
    use crate::tiles::api::TileTree;
    use crate::tiles::id::TileId;

    // TODO: finish implementing these tests. Test add vert+horz, remove, mutations
    #[test]
    fn add_remove() {
        let tree = Arc::new(TileTree::Tile(Tile {
            id: TileId::for_test(1),
            app: Default::default(),
            remote: Default::default(),
        }));

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::After,
            &mut Some(TileId::for_test(2)),
        )
        .unwrap();
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::Before,
            &mut Some(TileId::for_test(3)),
        )
        .unwrap();
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::After,
            &mut Some(TileId::for_test(4)),
        )
        .unwrap();
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());
    }
}
