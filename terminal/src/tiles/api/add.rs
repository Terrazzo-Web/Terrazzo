#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Side;
use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::app::App;
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
            let new_id = new_id
                .take()
                .ok_or(TilesStateError::DuplicateTileId(next_to))?;
            let new = Arc::new(Tiles::Tile(Tile {
                id: new_id,
                app: App::Default,
                remote: node.remote.clone(),
                title: format!("New tile {new_id}").into(),
            }));
            Arc::new(Tiles::Array {
                id: TileId::new(),
                direction: with_direction,
                title: node.title.clone(),
                selected: (with_direction == Direction::Tabbed).then_some(node.id),
                floating_nodes: vec![],
                nodes: match side {
                    Side::Before => vec![new, tree],
                    Side::After => vec![tree, new],
                },
            })
        }
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } if new_id.is_some() => {
            let mut nodes2 = Vec::with_capacity(nodes.len());
            let mut new_selected = *selected;
            for node in nodes {
                let list = add_node_flatten(
                    node.clone(),
                    with_direction,
                    *direction,
                    next_to,
                    side,
                    new_id,
                )?;
                {
                    let id = node.id();
                    if selected == &Some(id) {
                        new_selected = match side {
                            Side::Before => list.first().map(|first| first.id()),
                            Side::After => list.last().map(|last| last.id()),
                        };
                    }
                }
                nodes2.extend(list)
            }
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    let tile =
                        add_node_aux(floating.tile.clone(), with_direction, next_to, side, new_id)?;
                    Ok(Arc::new(floating.update(|_| tile)))
                })
                .collect::<Result<_, TilesStateError>>()?;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: new_selected,
                nodes: nodes2,
                floating_nodes,
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
        title: _,
        selected: _,
        nodes,
        floating_nodes,
    } = &*tree
        && *direction == flatten_direction
        && floating_nodes.is_empty()
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
