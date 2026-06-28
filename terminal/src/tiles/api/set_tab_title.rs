#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn set_tab_title(array_id: TileId, title: String) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut done = false;
    let tree = set_tab_title_aux(tree, array_id, &title, &mut done)?;
    if !done {
        return Err(TilesStateError::TileIdNotFound(array_id));
    }
    *lock = tree.clone().into();
    Ok(tree)
}

fn set_tab_title_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    new_title: &str,
    done: &mut bool,
) -> Result<Arc<Tiles>, TilesStateError> {
    if *done {
        return Ok(tree);
    }
    Ok(match &*tree {
        Tiles::Tile { .. } => tree,
        Tiles::Array {
            id,
            direction,
            title: _old_title,
            selected,
            nodes,
            floating_nodes,
        } if *id == array_id => {
            *done = true;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: new_title.into(),
                selected: *selected,
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
                .map(|node| set_tab_title_aux(node.clone(), array_id, new_title, done))
                .collect::<Result<_, _>>()?;
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    let tile = set_tab_title_aux(floating.tile.clone(), array_id, new_title, done)?;
                    Ok(Arc::new(floating.update(|_| tile)))
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
