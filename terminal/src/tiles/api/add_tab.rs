#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::app::App;
use crate::tiles::id::TileId;

pub fn add_tab(
    array_id: TileId,
    after_child: Option<TileId>,
) -> Result<(Arc<Tiles>, TileId), TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let new_id = TileId::new();
    let tree = try_transform_first(tree, &mut |tree| {
        let Tiles::Array {
            id,
            direction,
            title,
            nodes,
            floating_nodes,
            ..
        } = tree
        else {
            return Ok::<_, TilesStateError>(None);
        };
        if *id != array_id || *direction != Direction::Tabbed {
            return Ok(None);
        }
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
                    .position(|node| node.id() == after_child)
                    .map(|index| index + 1)
            })
            .unwrap_or(nodes.len());
        nodes.insert(insert_at, new);
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: Some(new_id),
            nodes,
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))?;
    *lock = tree.clone().into();
    Ok((tree, new_id))
}
