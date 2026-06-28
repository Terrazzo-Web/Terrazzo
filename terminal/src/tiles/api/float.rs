#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::FloatingTile;
use super::Tile;
use super::Tiles;
use super::state::TREE;
use super::state::TilesStateError;
use crate::tiles::app::App;
use crate::tiles::id::TileId;

pub fn float_node(tile_id: TileId) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let host_id = find_nearest_tabbed(&tree, tile_id, None)
        .ok_or(TilesStateError::TileIdNotFound(tile_id))?;
    let tree = match host_id {
        Some(host_id) => float_in_host(tree, host_id, tile_id)?,
        None => wrap_root(tree, tile_id)?,
    };
    *lock = Some(tree.clone());
    Ok(tree)
}

pub fn raise_floating(
    array_id: TileId,
    floating_id: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.take().unwrap_or_default();
    let mut raised = false;
    let tree = raise_floating_aux(tree, array_id, floating_id, &mut raised);
    if !raised {
        return Err(TilesStateError::TileIdNotFound(floating_id));
    }
    *lock = Some(tree.clone());
    Ok(tree)
}

fn find_nearest_tabbed(
    tree: &Tiles,
    tile_id: TileId,
    nearest: Option<TileId>,
) -> Option<Option<TileId>> {
    match tree {
        Tiles::Tile(tile) => (tile.id == tile_id).then_some(nearest),
        Tiles::Array {
            id,
            direction,
            nodes,
            floating_nodes,
            ..
        } => {
            let nearest = (*direction == Direction::Tabbed).then_some(*id).or(nearest);
            let nodes = nodes.iter().chain(floating_nodes.iter().map(|t| &t.tile));
            for node in nodes {
                if let Some(nearest) = find_nearest_tabbed(node, tile_id, nearest) {
                    return Some(nearest);
                }
            }
            None
        }
    }
}

fn float_in_host(
    tree: Arc<Tiles>,
    host_id: TileId,
    tile_id: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    match &*tree {
        Tiles::Tile(_) => Ok(tree),
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } if *id == host_id => {
            let (mut nodes, mut extracted) = extract_from_nodes(nodes, tile_id);
            let mut floating_nodes = floating_nodes.clone();
            if extracted.is_none() {
                let result = extract_from_floating(&floating_nodes, tile_id);
                floating_nodes = result.0;
                extracted = result.1;
            }
            let extracted = extracted.ok_or(TilesStateError::TileIdNotFound(tile_id))?;
            if nodes.is_empty() {
                nodes.push(default_tile());
            }
            floating_nodes.push(new_floating(extracted, &floating_nodes));
            let selected = selected
                .filter(|selected| nodes.iter().any(|node| node.id() == *selected))
                .or_else(|| nodes.first().map(|node| node.id()));
            Ok(Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected,
                nodes,
                floating_nodes,
            }))
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
                .map(|node| float_in_host(node.clone(), host_id, tile_id))
                .collect::<Result<_, _>>()?;
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    let tile = float_in_host(floating.tile.clone(), host_id, tile_id)?;
                    Ok(Arc::new(floating.update(|_| tile)))
                })
                .collect::<Result<_, TilesStateError>>()?;
            Ok(Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes,
                floating_nodes,
            }))
        }
    }
}

fn wrap_root(tree: Arc<Tiles>, tile_id: TileId) -> Result<Arc<Tiles>, TilesStateError> {
    let (remaining, extracted) = extract(tree, tile_id);
    let extracted = extracted.ok_or(TilesStateError::TileIdNotFound(tile_id))?;
    let nodes = vec![remaining.unwrap_or_else(default_tile)];
    let selected = nodes.first().map(|node| node.id());
    Ok(Arc::new(Tiles::Array {
        id: TileId::new(),
        direction: Direction::Tabbed,
        title: "Floating tiles".into(),
        selected,
        floating_nodes: vec![new_floating(extracted, &[])],
        nodes,
    }))
}

fn extract_from_nodes(
    nodes: &[Arc<Tiles>],
    tile_id: TileId,
) -> (Vec<Arc<Tiles>>, Option<Arc<Tiles>>) {
    let mut extracted = None;
    let nodes = nodes
        .iter()
        .filter_map(|node| {
            if extracted.is_some() {
                return Some(node.clone());
            }
            let (node, found) = extract(node.clone(), tile_id);
            extracted = found;
            node
        })
        .collect();
    (nodes, extracted)
}

fn extract_from_floating(
    floating_nodes: &[Arc<FloatingTile>],
    tile_id: TileId,
) -> (Vec<Arc<FloatingTile>>, Option<Arc<Tiles>>) {
    let mut extracted = None;
    let floating_nodes = floating_nodes
        .iter()
        .filter_map(|floating| {
            if extracted.is_some() {
                return Some(floating.clone());
            }
            let (tile, found) = extract(floating.tile.clone(), tile_id);
            extracted = found;
            tile.map(|tile| Arc::new(floating.update(|_| tile)))
        })
        .collect();
    (floating_nodes, extracted)
}

fn extract(tree: Arc<Tiles>, tile_id: TileId) -> (Option<Arc<Tiles>>, Option<Arc<Tiles>>) {
    match &*tree {
        Tiles::Tile(tile) if tile.id == tile_id => (None, Some(tree)),
        Tiles::Tile(_) => (Some(tree), None),
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } => {
            let (nodes, extracted) = extract_from_nodes(nodes, tile_id);
            if extracted.is_none() {
                return (Some(tree), None);
            }
            if nodes.len() == 1 && floating_nodes.is_empty() {
                return (nodes.into_iter().next(), extracted);
            }
            let selected = selected
                .filter(|selected| nodes.iter().any(|node| node.id() == *selected))
                .or_else(|| nodes.first().map(|node| node.id()));
            (
                (!nodes.is_empty() || !floating_nodes.is_empty()).then(|| {
                    Arc::new(Tiles::Array {
                        id: *id,
                        direction: *direction,
                        title: title.clone(),
                        selected,
                        nodes,
                        floating_nodes: floating_nodes.clone(),
                    })
                }),
                extracted,
            )
        }
    }
}

fn raise_floating_aux(
    tree: Arc<Tiles>,
    array_id: TileId,
    floating_id: TileId,
    raised: &mut bool,
) -> Arc<Tiles> {
    match &*tree {
        Tiles::Tile(_) => tree,
        Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } if *id == array_id => {
            let Some(index) = floating_nodes
                .iter()
                .position(|floating| floating.tile.id() == floating_id)
            else {
                return tree;
            };
            let z_index = floating_nodes
                .iter()
                .map(|floating| floating.z_index)
                .max()
                .unwrap_or_default()
                + 1;
            let mut floating_nodes = floating_nodes.clone();
            // FloatingTile::update only replaces the tile and preserves z_index, but raising a
            // floating tile specifically requires changing its z_index.
            floating_nodes[index] = Arc::new(FloatingTile {
                x1: floating_nodes[index].x1,
                y1: floating_nodes[index].y1,
                x2: floating_nodes[index].x2,
                y2: floating_nodes[index].y2,
                z_index,
                tile: floating_nodes[index].tile.clone(),
            });
            *raised = true;
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes: nodes.clone(),
                floating_nodes,
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
                .map(|node| raise_floating_aux(node.clone(), array_id, floating_id, raised))
                .collect();
            let floating_nodes = floating_nodes
                .iter()
                .map(|floating| {
                    let tile =
                        raise_floating_aux(floating.tile.clone(), array_id, floating_id, raised);
                    Arc::new(floating.update(|_| tile))
                })
                .collect();
            Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes,
                floating_nodes,
            })
        }
    }
}

fn new_floating(tile: Arc<Tiles>, floating_nodes: &[Arc<FloatingTile>]) -> Arc<FloatingTile> {
    Arc::new(FloatingTile {
        x1: 10,
        y1: 10,
        x2: 90,
        y2: 90,
        z_index: floating_nodes
            .iter()
            .map(|floating| floating.z_index)
            .max()
            .unwrap_or_default()
            + 1,
        tile,
    })
}

pub(super) fn default_tile() -> Arc<Tiles> {
    let id = TileId::new();
    Arc::new(Tiles::Tile(Tile {
        id,
        app: App::Default,
        remote: Default::default(),
        title: format!("New tile {id}").into(),
    }))
}

impl FloatingTile {
    pub fn update(&self, f: impl FnOnce(&Self) -> Arc<Tiles>) -> Self {
        FloatingTile {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
            z_index: self.z_index,
            tile: f(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_the_only_root_tile_wraps_it_and_adds_a_default_tab() {
        let tree = tile(1);
        let tree = wrap_root(tree, TileId::for_test(1)).unwrap();
        let Tiles::Array {
            direction,
            nodes,
            floating_nodes,
            ..
        } = &*tree
        else {
            panic!("expected tabbed root");
        };
        assert_eq!(Direction::Tabbed, *direction);
        assert_eq!(1, nodes.len());
        assert_eq!(App::Default, app(&nodes[0]));
        assert_eq!(1, floating_nodes.len());
        assert_eq!(TileId::for_test(1), floating_nodes[0].tile.id());
    }

    #[test]
    fn floating_uses_the_nearest_tabbed_parent() {
        let nested_id = TileId::for_test(20);
        let tree = Arc::new(Tiles::Array {
            id: TileId::for_test(10),
            direction: Direction::Tabbed,
            title: "outer".into(),
            selected: Some(nested_id),
            nodes: vec![Arc::new(Tiles::Array {
                id: nested_id,
                direction: Direction::Tabbed,
                title: "inner".into(),
                selected: Some(TileId::for_test(1)),
                nodes: vec![tile(1), tile(2)],
                floating_nodes: vec![],
            })],
            floating_nodes: vec![],
        });

        assert_eq!(
            Some(Some(nested_id)),
            find_nearest_tabbed(&tree, TileId::for_test(1), None)
        );
        let tree = float_in_host(tree, nested_id, TileId::for_test(1)).unwrap();
        let Tiles::Array { nodes, .. } = &*tree else {
            panic!("expected outer array");
        };
        let Tiles::Array {
            nodes,
            floating_nodes,
            ..
        } = &*nodes[0]
        else {
            panic!("expected inner array");
        };
        assert_eq!(1, nodes.len());
        assert_eq!(TileId::for_test(2), nodes[0].id());
        assert_eq!(TileId::for_test(1), floating_nodes[0].tile.id());
    }

    #[test]
    fn floating_the_last_regular_node_adds_a_default_tile() {
        let host_id = TileId::for_test(10);
        let tree = Arc::new(Tiles::Array {
            id: host_id,
            direction: Direction::Tabbed,
            title: "tabs".into(),
            selected: Some(TileId::for_test(1)),
            nodes: vec![tile(1)],
            floating_nodes: vec![],
        });
        let tree = float_in_host(tree, host_id, TileId::for_test(1)).unwrap();
        let Tiles::Array {
            nodes,
            floating_nodes,
            ..
        } = &*tree
        else {
            panic!("expected array");
        };
        assert_eq!(1, nodes.len());
        assert_eq!(App::Default, app(&nodes[0]));
        assert_eq!(TileId::for_test(1), floating_nodes[0].tile.id());
    }

    fn tile(id: i64) -> Arc<Tiles> {
        Arc::new(Tiles::Tile(Tile {
            id: TileId::for_test(id),
            app: App::Default,
            remote: Default::default(),
            title: format!("Tile {id}").into(),
        }))
    }

    fn app(tree: &Tiles) -> App {
        match tree {
            Tiles::Tile(tile) => tile.app,
            Tiles::Array { .. } => panic!("expected tile"),
        }
    }
}
