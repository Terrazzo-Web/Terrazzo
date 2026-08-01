#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::id::TileId;

pub fn remove_node(id_to_remove: TileId) -> Result<Arc<Tiles>, TilesStateError> {
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
    tree: Arc<Tiles>,
    maybe_id_to_remove: &mut Option<TileId>,
) -> Option<Arc<Tiles>> {
    let Some(id_to_remove) = maybe_id_to_remove else {
        return Some(tree);
    };
    match &*tree {
        Tiles::Tile(node) if node.id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        Tiles::Array { id, .. } if *id == *id_to_remove => {
            *maybe_id_to_remove = None;
            None
        }
        Tiles::Tile { .. } => Some(tree),
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } => {
            let mut nodes = nodes
                .iter()
                .filter_map(|node| remove_node_aux(node.clone(), maybe_id_to_remove))
                .collect::<Vec<_>>();
            let floating_nodes = floating_nodes
                .iter()
                .filter_map(|floating| {
                    let tile = remove_node_aux(floating.tile.clone(), maybe_id_to_remove)?;
                    Some(Arc::new(floating.update(|_| tile)))
                })
                .collect::<Vec<_>>();
            if nodes.is_empty() && !floating_nodes.is_empty() {
                nodes.push(super::float::default_tile());
            }
            if nodes.len() <= 1 && floating_nodes.is_empty() {
                nodes.into_iter().next()
            } else {
                Some(Arc::new(Tiles::Array {
                    id: *id,
                    direction: *direction,
                    title: title.clone(),
                    selected: selected
                        .filter(|selected| nodes.iter().any(|node| node.id() == *selected)),
                    nodes,
                    floating_nodes,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Direction;
    use super::super::FloatingTile;
    use super::super::Tile;
    use super::*;
    use crate::tiles::app::App;

    #[test]
    fn removing_last_regular_node_with_floating_nodes_adds_default_tile() {
        let regular_id = TileId::for_test(1);
        let tree = Arc::new(Tiles::Array {
            id: TileId::for_test(10),
            direction: Direction::Tabbed,
            title: "Tabs".into(),
            selected: Some(regular_id),
            nodes: vec![tile(regular_id)],
            floating_nodes: vec![Arc::new(FloatingTile {
                x: 10,
                y: 10,
                width: 800,
                height: 600,
                z_index: 1,
                tile: tile(TileId::for_test(2)),
            })],
        });

        let tree = remove_node_aux(tree, &mut Some(regular_id)).unwrap();
        let Tiles::Array {
            nodes,
            floating_nodes,
            ..
        } = &*tree
        else {
            panic!("expected array");
        };
        assert_eq!(1, nodes.len());
        assert_eq!(1, floating_nodes.len());
        let Tiles::Tile(tile) = &*nodes[0] else {
            panic!("expected default tile");
        };
        assert_eq!(App::Default, tile.app);
    }

    fn tile(id: TileId) -> Arc<Tiles> {
        Arc::new(Tiles::Tile(Tile {
            id,
            app: App::Default,
            remote: Default::default(),
            title: format!("Tile {id}").into(),
        }))
    }
}
