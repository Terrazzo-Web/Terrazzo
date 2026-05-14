#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use nameth::NamedEnumValues as _;
use nameth::nameth;

use super::id::TileId;
use crate::tiles::tree::Direction;
use crate::tiles::tree::Side;
use crate::tiles::tree::TileNode;
use crate::tiles::tree::TileTree;

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
        TileTree::Node(node) if node.id == next_to => {
            let new = Arc::new(TileTree::Node(TileNode {
                id: new_id
                    .take()
                    .ok_or_else(|| TilesStateError::DuplicateTileId(next_to))?,
                app: node.app,
                remote: node.remote.clone(),
            }));
            Arc::new(TileTree::Array {
                direction: with_direction,
                nodes: match side {
                    Side::Before => vec![new, tree],
                    Side::After => vec![tree, new],
                },
            })
        }
        TileTree::Array { direction, nodes } if new_id.is_some() => {
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
    if let TileTree::Array { direction, nodes } = &*tree
        && *direction == flatten_direction
    {
        Ok(nodes.clone())
    } else {
        Ok(vec![tree])
    }
}

// Remove node

pub fn remove_node(id: TileId) -> Result<Arc<TileTree>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let mut maybe_id = Some(id);
    let tree = remove_node_aux(tree, &mut maybe_id).unwrap_or_default();
    if let Some(id) = maybe_id {
        return Err(TilesStateError::TileIdNotFound(id));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn remove_node_aux(tree: Arc<TileTree>, maybe_id: &mut Option<TileId>) -> Option<Arc<TileTree>> {
    let Some(id) = maybe_id else {
        return Some(tree);
    };
    match &*tree {
        TileTree::Node(node) if node.id == *id => {
            *maybe_id = None;
            None
        }
        TileTree::Node { .. } => Some(tree),
        TileTree::Array { direction, nodes } => {
            let nodes = nodes
                .iter()
                .filter_map(|node| remove_node_aux(node.clone(), maybe_id))
                .collect::<Vec<_>>();
            if nodes.len() <= 1 {
                nodes.into_iter().next()
            } else {
                Some(Arc::new(TileTree::Array {
                    direction: *direction,
                    nodes,
                }))
            }
        }
    }
}

// Mutate node

pub fn mutate_node(
    id: TileId,
    mutation: impl FnOnce(&TileNode) -> TileNode,
) -> Result<Arc<TileTree>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut maybe_mutation = Some(mutation);
    let tree = mutate_node_aux(tree, id, &mut maybe_mutation)?;
    if maybe_mutation.is_some() {
        return Err(TilesStateError::TileIdNotFound(id));
    }
    *lock = tree.clone().into();
    return Ok(tree);
}

fn mutate_node_aux(
    tree: Arc<TileTree>,
    id: TileId,
    maybe_mutation: &mut Option<impl FnOnce(&TileNode) -> TileNode>,
) -> Result<Arc<TileTree>, TilesStateError> {
    if maybe_mutation.is_none() {
        return Ok(tree);
    }
    Ok(match &*tree {
        TileTree::Node(node) if node.id == id => {
            let Some(mutation) = maybe_mutation.take() else {
                return Err(TilesStateError::DuplicateTileId(id));
            };
            Arc::new(TileTree::Node(mutation(node)))
        }
        TileTree::Node { .. } => tree,
        TileTree::Array { direction, nodes } => {
            let mut nodes2 = Vec::with_capacity(nodes.len());
            for node in nodes {
                nodes2.push(mutate_node_aux(node.clone(), id, maybe_mutation)?);
            }
            Arc::new(TileTree::Array {
                direction: *direction,
                nodes: nodes2,
            })
        }
    })
}

impl Drop for TileNode {
    fn drop(&mut self) {
        fn aux(this: &mut TileNode) -> Option<()> {
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

    use crate::tiles::id::TileId;
    use crate::tiles::tree::Direction;
    use crate::tiles::tree::Side;
    use crate::tiles::tree::TileNode;
    use crate::tiles::tree::TileTree;

    // TODO: finish implementing these tests. Test add vert+horz, remove, mutations
    #[test]
    fn add_remove() {
        let tree = Arc::new(TileTree::Node(TileNode {
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
