#![cfg(feature = "server")]

use std::sync::Arc;

use super::Tiles;

pub(super) fn try_transform_first<E>(
    tree: Arc<Tiles>,
    transform: &mut impl FnMut(&Tiles) -> Result<Option<Arc<Tiles>>, E>,
) -> Result<Option<Arc<Tiles>>, E> {
    if let Some(tree) = transform(&tree)? {
        return Ok(Some(tree));
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
        return Ok(None);
    };
    for (index, node) in nodes.iter().enumerate() {
        if let Some(node) = try_transform_first(node.clone(), transform)? {
            let mut nodes = nodes.clone();
            nodes[index] = node;
            return Ok(Some(Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes,
                floating_nodes: floating_nodes.clone(),
            })));
        }
    }
    for (index, floating) in floating_nodes.iter().enumerate() {
        if let Some(tile) = try_transform_first(floating.tile.clone(), transform)? {
            let mut floating_nodes = floating_nodes.clone();
            floating_nodes[index] = Arc::new(floating.update(|_| tile));
            return Ok(Some(Arc::new(Tiles::Array {
                id: *id,
                direction: *direction,
                title: title.clone(),
                selected: *selected,
                nodes: nodes.clone(),
                floating_nodes,
            })));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::super::Direction;
    use super::super::FloatingTile;
    use super::super::Tile;
    use super::*;
    use crate::tiles::app::App;
    use crate::tiles::id::TileId;

    #[test]
    fn transforms_a_node_inside_a_floating_subtree() {
        let floating_tile = tile(2);
        let tree = Arc::new(Tiles::Array {
            id: TileId::for_test(10),
            direction: Direction::Horizontal,
            title: "Root".into(),
            selected: None,
            nodes: vec![tile(1)],
            floating_nodes: vec![Arc::new(FloatingTile {
                x: 10,
                y: 20,
                width: 800,
                height: 600,
                z_index: 1,
                tile: floating_tile,
            })],
        });

        let transformed = try_transform_first(tree, &mut |tree| {
            let Tiles::Tile(tile) = tree else {
                return Ok::<_, ()>(None);
            };
            Ok((tile.id == TileId::for_test(2)).then(|| {
                Arc::new(Tiles::Tile(Tile {
                    id: tile.id,
                    app: tile.app,
                    remote: tile.remote.clone(),
                    title: "Changed".into(),
                }))
            }))
        })
        .unwrap()
        .unwrap();

        let Tiles::Array {
            nodes,
            floating_nodes,
            ..
        } = &*transformed
        else {
            panic!("expected array");
        };
        assert_eq!("", title(&nodes[0]));
        assert_eq!("Changed", title(&floating_nodes[0].tile));
    }

    #[test]
    fn returns_none_without_rebuilding_when_no_node_matches() {
        let tree = tile(1);
        let transformed =
            try_transform_first(tree, &mut |_| Ok::<_, ()>(None)).expect("infallible transform");
        assert!(transformed.is_none());
    }

    fn tile(id: i64) -> Arc<Tiles> {
        Arc::new(Tiles::Tile(Tile {
            id: TileId::for_test(id),
            app: App::Default,
            remote: Default::default(),
            title: Default::default(),
        }))
    }

    fn title(tree: &Tiles) -> &str {
        let Tiles::Tile(tile) = tree else {
            panic!("expected tile");
        };
        &tile.title
    }
}
