#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn select_child(
    array_id: TileId,
    selected_child: Option<TileId>,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut selected = false;
    let tree = select_child_aux(tree, array_id, selected_child, &mut selected)?;
    if !selected {
        return Err(TilesStateError::TileIdNotFound(array_id));
    }
    *lock = tree.clone().into();
    Ok(tree)
}

fn select_child_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    selected_child: Option<TileId>,
    selected: &mut bool,
) -> Result<Arc<Tiles>, TilesStateError> {
    if *selected {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            title,
            selected: old_selected,
            nodes,
            floating_nodes,
        } if *id == array_id => {
            if let Some(selected_child) = selected_child
                && !nodes.iter().any(|node| child_id(node) == selected_child)
            {
                return Err(TilesStateError::TileIdNotFound(selected_child));
            }
            *selected = true;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: selected_child.or(*old_selected),
                nodes: nodes.clone(),
                floating_nodes: floating_nodes.clone(),
            })
        }
        Tiles::Array {
            id,
            direction,
            title,
            selected: old_selected,
            nodes,
            floating_nodes,
        } => {
            let nodes = nodes
                .iter()
                .map(|node| select_child_aux(node.clone(), array_id, selected_child, selected))
                .collect::<Result<_, _>>()?;
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    Ok(Arc::new(super::FloatingTile {
                        tile: (*select_child_aux(
                            Arc::new(floating.tile.clone()),
                            array_id,
                            selected_child,
                            selected,
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
                selected: *old_selected,
                nodes,
                floating_nodes,
            })
        }
    })
}

fn child_id(node: &Tiles) -> TileId {
    match node {
        Tiles::Tile(tile) => tile.id,
        Tiles::Array { id, .. } => *id,
    }
}
