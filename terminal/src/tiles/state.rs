#![cfg(feature = "server")]

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

use super::id::TileId;
use crate::tiles::tree::Direction;
use crate::tiles::tree::Side;
use crate::tiles::tree::TileNode;
use crate::tiles::tree::TileTree;

pub static STATE: OnceLock<HashMap<TileId, Box<dyn Fn(TileId) + Send + Sync>>> = OnceLock::new();

pub static TREE: Mutex<TileTree> = Mutex::new(TileTree::Node(TileNode {
    id: TileId::first_tile_id(),
}));

// Add node

pub fn add_node(
    tree: &mut TileTree,
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> TileTree {
    let mut new_id = Some(TileId::new());
    let tree = add_node_aux(tree.take(), with_direction, next_to, side, &mut new_id);
    assert!(new_id.is_none());
    tree
}

fn add_node_aux(
    tree: TileTree,
    with_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> TileTree {
    match tree {
        TileTree::Node(node) if node.id == next_to => TileTree::Array {
            direction: with_direction,
            nodes: match side {
                Side::Before => vec![
                    TileTree::Node(TileNode {
                        id: new_id.take().expect("new id"),
                    }),
                    TileTree::Node(node),
                ],
                Side::After => vec![
                    TileTree::Node(node),
                    TileTree::Node(TileNode {
                        id: new_id.take().expect("new id"),
                    }),
                ],
            },
        },
        TileTree::Array { direction, nodes } if new_id.is_some() => {
            let nodes = nodes
                .into_iter()
                .flat_map(|tree| {
                    add_node_flatten(tree, with_direction, direction, next_to, side, new_id)
                })
                .collect::<Vec<_>>();
            TileTree::Array { direction, nodes }
        }
        tree => tree,
    }
}

fn add_node_flatten(
    tree: TileTree,
    with_direction: Direction,
    flatten_direction: Direction,
    next_to: TileId,
    side: Side,
    new_id: &mut Option<TileId>,
) -> Vec<TileTree> {
    if new_id.is_none() {
        return vec![tree];
    }
    let tree = add_node_aux(tree, with_direction, next_to, side, new_id);
    match tree {
        TileTree::Array { direction, nodes } if direction == flatten_direction => nodes,
        tree => vec![tree],
    }
}

// Remove node

pub fn remove_node(tree: &mut TileTree, id: TileId) -> Option<TileTree> {
    remove_node_aux(tree.take(), id)
}

fn remove_node_aux(tree: TileTree, id: TileId) -> Option<TileTree> {
    match tree {
        TileTree::Node(node) if node.id == id => None,
        tree @ TileTree::Node { .. } => Some(tree),
        TileTree::Array { direction, nodes } => {
            let nodes = nodes
                .into_iter()
                .filter_map(|node| remove_node_aux(node, id))
                .collect::<Vec<_>>();
            if nodes.len() <= 1 {
                nodes.into_iter().next()
            } else {
                Some(TileTree::Array { direction, nodes })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tiles::id::TileId;
    use crate::tiles::tree::Direction;
    use crate::tiles::tree::Side;
    use crate::tiles::tree::TileNode;
    use crate::tiles::tree::TileTree;

    // TODO: finish implementing these tests
    #[test]
    fn add_remove() {
        let tree = TileTree::Node(TileNode {
            id: TileId::for_test(1),
        });

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::After,
            &mut Some(TileId::for_test(2)),
        );
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::Before,
            &mut Some(TileId::for_test(3)),
        );
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());

        let tree = super::add_node_aux(
            tree,
            Direction::Horizontal,
            TileId::for_test(1),
            Side::After,
            &mut Some(TileId::for_test(4)),
        );
        assert_eq!("", serde_json::to_string_pretty(&tree).unwrap());
    }
}
