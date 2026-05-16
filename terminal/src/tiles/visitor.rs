use std::marker::PhantomData;

use terrazzo::prelude::XSignal;

use super::api::Direction;
use super::api::Tile as TileDto;
use super::api::TileTree as TileTreeDto;
use super::id::TileId;
use super::signals::TilePtr as UiTilePtr;
use super::signals::TileTree as UiTileTree;

pub trait TilesVisitorKind: Sized {
    type Node;
    type Tile;
    type Direction;

    fn do_visit_node<V: TilesTreeVisitor<Self> + ?Sized>(visitor: &mut V, tree: Self::Node);
}

pub trait TilesTreeVisitor<K: TilesVisitorKind> {
    fn visit_node(&mut self, node: K::Node) {
        K::do_visit_node(self, node);
    }
    fn visit_tree(
        &mut self,
        #[expect(unused)] id: TileId,
        #[expect(unused)] direction: K::Direction,
    ) {
    }
    fn visit_tile(&mut self, #[expect(unused)] tile: K::Tile) {}
}

pub struct DtoVisitor<'l> {
    _phantom: PhantomData<&'l ()>,
}

impl<'l> TilesVisitorKind for DtoVisitor<'l> {
    type Node = &'l TileTreeDto;
    type Tile = &'l TileDto;
    type Direction = Direction;

    fn do_visit_node<V: TilesTreeVisitor<Self> + ?Sized>(visitor: &mut V, tree: Self::Node) {
        match tree {
            TileTreeDto::Tile(tile) => visitor.visit_tile(tile),
            TileTreeDto::Array {
                id,
                direction,
                nodes,
            } => {
                visitor.visit_tree(*id, *direction);
                for node in nodes {
                    visitor.visit_node(node);
                }
            }
        }
    }
}

pub struct UiStateVisitor<'l> {
    _phantom: PhantomData<&'l ()>,
}

impl<'l> TilesVisitorKind for UiStateVisitor<'l> {
    type Node = &'l UiTileTree;
    type Tile = &'l UiTilePtr;
    type Direction = &'l XSignal<Direction>;

    fn do_visit_node<V: TilesTreeVisitor<Self> + ?Sized>(visitor: &mut V, tree: Self::Node) {
        match tree {
            UiTileTree::Tile(tile) => visitor.visit_tile(tile),
            UiTileTree::Array {
                id,
                direction,
                nodes,
            } => {
                visitor.visit_tree(*id, direction);
                for node in nodes {
                    visitor.visit_node(node);
                }
            }
        }
    }
}
