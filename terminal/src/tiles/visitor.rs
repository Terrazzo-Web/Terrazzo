#![cfg(feature = "client")]

use std::marker::PhantomData;

use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;

use super::api::Direction;
use super::id::TileId;
use super::signals::TilePtr as UiTilePtr;
use super::signals::Tiles as UiTileTree;

pub trait TilesVisitorKind: Sized {
    type Node;
    type Tile;
    type Direction;
    type Selected;
    type Title;

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
    fn visit_selected(
        &mut self,
        #[expect(unused)] id: TileId,
        #[expect(unused)] selected: K::Selected,
    ) {
    }
    fn visit_title(&mut self, #[expect(unused)] id: TileId, #[expect(unused)] title: K::Title) {}
}

pub struct UiStateVisitor<'l> {
    _phantom: PhantomData<&'l ()>,
}

impl<'l> TilesVisitorKind for UiStateVisitor<'l> {
    type Node = &'l UiTileTree;
    type Tile = &'l UiTilePtr;
    type Direction = &'l XSignal<Direction>;
    type Selected = &'l XSignal<Option<TileId>>;
    type Title = &'l XSignal<XString>;

    fn do_visit_node<V: TilesTreeVisitor<Self> + ?Sized>(visitor: &mut V, tree: Self::Node) {
        match tree {
            UiTileTree::Tile(tile) => visitor.visit_tile(tile),
            UiTileTree::Array {
                id,
                direction,
                title,
                selected,
                nodes,
            } => {
                visitor.visit_tree(*id, direction);
                visitor.visit_selected(*id, selected);
                visitor.visit_title(*id, title);
                for node in nodes {
                    visitor.visit_node(node);
                }
            }
        }
    }
}
