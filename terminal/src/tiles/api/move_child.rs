#![cfg(feature = "server")]

use std::sync::Arc;

use super::Direction;
use super::Tiles;
use super::float::default_tile;
use super::state::TREE;
use super::state::TilesStateError;
use super::transform::try_transform_first;
use crate::tiles::id::TileId;

pub fn move_child(
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    let mut lock = TREE.lock().map_err(|_| TilesStateError::PoisonError)?;
    let tree = lock.clone().unwrap_or_default();
    let tree = move_child_in_tree(tree, array_id, after_child, moved_child)?;
    *lock = Some(tree.clone());
    Ok(tree)
}

fn move_child_in_tree(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    if after_child == Some(moved_child) {
        return Ok(tree);
    }

    let source_array_id = find_tabbed_parent(&tree, moved_child)
        .ok_or(TilesStateError::TileIdNotFound(moved_child))?;
    let destination = find_node(&tree, array_id)
        .filter(|destination| {
            matches!(
                &**destination,
                Tiles::Array {
                    direction: Direction::Tabbed,
                    ..
                }
            )
        })
        .ok_or(TilesStateError::TileIdNotFound(array_id))?;
    let moved_node =
        find_node(&tree, moved_child).ok_or(TilesStateError::TileIdNotFound(moved_child))?;

    if contains_id(&moved_node, array_id) {
        return Err(TilesStateError::MoveIntoDescendant {
            moved: moved_child,
            destination: array_id,
        });
    }

    validate_after_child(&destination, after_child)?;

    if source_array_id == array_id {
        return reorder_child(tree, array_id, after_child, moved_child);
    }

    let (tree, detached) = detach_node(tree, moved_child);
    let tree = tree.ok_or(TilesStateError::TileIdNotFound(array_id))?;
    let detached = detached.ok_or(TilesStateError::TileIdNotFound(moved_child))?;
    insert_child(tree, array_id, after_child, detached)
}

fn reorder_child(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
) -> Result<Arc<Tiles>, TilesStateError> {
    try_transform_first(tree, &mut |tree| {
        let Tiles::Array {
            id,
            direction,
            title,
            selected,
            nodes,
            floating_nodes,
        } = tree
        else {
            return Ok(None);
        };
        if *id != array_id || *direction != Direction::Tabbed {
            return Ok(None);
        }
        let from = nodes
            .iter()
            .position(|node| node.id() == moved_child)
            .ok_or(TilesStateError::TileIdNotFound(moved_child))?;
        let moved_node = nodes[from].clone();
        let mut nodes = nodes.clone();
        nodes.remove(from);
        let insert_at = insertion_index(&nodes, after_child)?;
        nodes.insert(insert_at, moved_node);
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: selected.or(Some(moved_child)),
            nodes,
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))
}

fn insert_child(
    tree: Arc<Tiles>,
    array_id: TileId,
    after_child: Option<TileId>,
    moved_node: Arc<Tiles>,
) -> Result<Arc<Tiles>, TilesStateError> {
    let moved_child = moved_node.id();
    try_transform_first(tree, &mut |tree| {
        let Tiles::Array {
            id,
            direction,
            title,
            nodes,
            floating_nodes,
            ..
        } = tree
        else {
            return Ok(None);
        };
        if *id != array_id || *direction != Direction::Tabbed {
            return Ok(None);
        }
        let mut nodes = nodes.clone();
        let insert_at = insertion_index(&nodes, after_child)?;
        nodes.insert(insert_at, moved_node.clone());
        Ok(Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected: Some(moved_child),
            nodes,
            floating_nodes: floating_nodes.clone(),
        })))
    })?
    .ok_or(TilesStateError::TileIdNotFound(array_id))
}

fn insertion_index(
    nodes: &[Arc<Tiles>],
    after_child: Option<TileId>,
) -> Result<usize, TilesStateError> {
    let Some(after_child) = after_child else {
        return Ok(0);
    };
    nodes
        .iter()
        .position(|node| node.id() == after_child)
        .map(|index| index + 1)
        .ok_or(TilesStateError::TileIdNotFound(after_child))
}

fn validate_after_child(
    destination: &Tiles,
    after_child: Option<TileId>,
) -> Result<(), TilesStateError> {
    let Some(after_child) = after_child else {
        return Ok(());
    };
    let Tiles::Array { nodes, .. } = destination else {
        return Err(TilesStateError::TileIdNotFound(after_child));
    };
    nodes
        .iter()
        .any(|node| node.id() == after_child)
        .then_some(())
        .ok_or(TilesStateError::TileIdNotFound(after_child))
}

fn find_tabbed_parent(tree: &Tiles, child_id: TileId) -> Option<TileId> {
    let Tiles::Array {
        id,
        direction,
        nodes,
        floating_nodes,
        ..
    } = tree
    else {
        return None;
    };
    if *direction == Direction::Tabbed && nodes.iter().any(|node| node.id() == child_id) {
        return Some(*id);
    }
    nodes
        .iter()
        .find_map(|node| find_tabbed_parent(node, child_id))
        .or_else(|| {
            floating_nodes
                .iter()
                .find_map(|floating| find_tabbed_parent(&floating.tile, child_id))
        })
}

fn find_node(tree: &Arc<Tiles>, id: TileId) -> Option<Arc<Tiles>> {
    if tree.id() == id {
        return Some(tree.clone());
    }
    let Tiles::Array {
        nodes,
        floating_nodes,
        ..
    } = &**tree
    else {
        return None;
    };
    nodes
        .iter()
        .find_map(|node| find_node(node, id))
        .or_else(|| {
            floating_nodes
                .iter()
                .find_map(|floating| find_node(&floating.tile, id))
        })
}

fn contains_id(tree: &Tiles, id: TileId) -> bool {
    if tree.id() == id {
        return true;
    }
    let Tiles::Array {
        nodes,
        floating_nodes,
        ..
    } = tree
    else {
        return false;
    };
    nodes.iter().any(|node| contains_id(node, id))
        || floating_nodes
            .iter()
            .any(|floating| contains_id(&floating.tile, id))
}

fn detach_node(tree: Arc<Tiles>, moved_child: TileId) -> (Option<Arc<Tiles>>, Option<Arc<Tiles>>) {
    if tree.id() == moved_child {
        return (None, Some(tree));
    }
    let Tiles::Array {
        id,
        direction,
        title,
        selected,
        nodes,
        floating_nodes,
    } = &*tree
    else {
        return (Some(tree), None);
    };

    let mut detached = None;
    let mut new_nodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        if detached.is_some() {
            new_nodes.push(node.clone());
            continue;
        }
        let (node, found) = detach_node(node.clone(), moved_child);
        detached = found;
        new_nodes.extend(node);
    }

    let mut new_floating_nodes = Vec::with_capacity(floating_nodes.len());
    for floating in floating_nodes {
        if detached.is_some() {
            new_floating_nodes.push(floating.clone());
            continue;
        }
        let (tile, found) = detach_node(floating.tile.clone(), moved_child);
        detached = found;
        if let Some(tile) = tile {
            new_floating_nodes.push(Arc::new(floating.update(|_| tile)));
        }
    }

    if detached.is_none() {
        return (Some(tree), None);
    }
    if new_nodes.is_empty() && !new_floating_nodes.is_empty() {
        new_nodes.push(default_tile());
    }
    if new_nodes.len() <= 1 && new_floating_nodes.is_empty() {
        return (new_nodes.into_iter().next(), detached);
    }
    let selected = selected.filter(|selected| new_nodes.iter().any(|node| node.id() == *selected));
    (
        Some(Arc::new(Tiles::Array {
            id: *id,
            direction: *direction,
            title: title.clone(),
            selected,
            nodes: new_nodes,
            floating_nodes: new_floating_nodes,
        })),
        detached,
    )
}

#[cfg(test)]
mod tests {
    use super::super::Tile;
    use super::*;
    use crate::tiles::app::App;

    #[test]
    fn moves_a_tab_between_arrays_and_collapses_the_source() {
        let tree = array(
            100,
            Direction::Horizontal,
            vec![
                array(10, Direction::Tabbed, vec![tile(1), tile(2)]),
                array(20, Direction::Tabbed, vec![tile(3), tile(4)]),
            ],
        );

        let tree = move_child_in_tree(
            tree,
            TileId::for_test(20),
            Some(TileId::for_test(3)),
            TileId::for_test(2),
        )
        .unwrap();

        let Tiles::Array { nodes, .. } = &*tree else {
            panic!("expected root array");
        };
        assert_eq!(TileId::for_test(1), nodes[0].id());
        assert_eq!(vec![3, 2, 4], child_ids(&nodes[1]));
    }

    #[test]
    fn rejects_moving_an_array_into_its_descendant() {
        let moved = array(
            10,
            Direction::Tabbed,
            vec![
                array(20, Direction::Tabbed, vec![tile(1), tile(2)]),
                tile(3),
            ],
        );
        let tree = array(100, Direction::Tabbed, vec![moved, tile(4)]);

        let error =
            move_child_in_tree(tree, TileId::for_test(20), None, TileId::for_test(10)).unwrap_err();

        assert!(matches!(error, TilesStateError::MoveIntoDescendant { .. }));
    }

    fn array(id: i64, direction: Direction, nodes: Vec<Arc<Tiles>>) -> Arc<Tiles> {
        Arc::new(Tiles::Array {
            id: TileId::for_test(id),
            direction,
            title: format!("Array {id}").into(),
            selected: nodes.first().map(|node| node.id()),
            nodes,
            floating_nodes: vec![],
        })
    }

    fn tile(id: i64) -> Arc<Tiles> {
        Arc::new(Tiles::Tile(Tile {
            id: TileId::for_test(id),
            app: App::Default,
            remote: Default::default(),
            title: format!("Tile {id}").into(),
        }))
    }

    fn child_ids(tree: &Tiles) -> Vec<i64> {
        let Tiles::Array { nodes, .. } = tree else {
            panic!("expected array");
        };
        nodes.iter().map(|node| i64::from(node.id())).collect()
    }
}
