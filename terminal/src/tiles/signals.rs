#![cfg(feature = "client")]

use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo::envelope;
use terrazzo::prelude::XSignal;

use super::api::Direction;
use super::api::Tile as TileDto;
use super::api::TileTree as TileTreeDto;
use super::app::App;
use super::id::TileId;
use super::visitor::DtoVisitor;
use super::visitor::TilesTreeVisitor;
use super::visitor::UiStateVisitor;
use crate::frontend::remotes::Remote;

pub enum TileTree {
    Tile(TilePtr),
    Array {
        id: TileId,
        direction: XSignal<Direction>,
        nodes: Vec<Rc<TileTree>>,
    },
}

#[envelope]
pub struct Tile {
    pub id: TileId,
    pub app: XSignal<App>,
    pub remote: XSignal<Remote>,
}

impl TileTree {
    pub fn update(&mut self, dto: TileTreeDto) {
        let mut ids = TileIds::default();
        ids.visit_node(&dto);
        let mut signals = TileSignals::default();
        signals.visit_node(self);
        transform(&mut signals, dto);
    }
}

fn transform(
    signals: &mut TileSignals,
    tile_tree_dto: &TileTreeDto,
) -> Result<TileTree, TransformError> {
    Ok(match tile_tree_dto {
        TileTreeDto::Tile(TileDto { id, app, remote }) => {
            let Some(ui_tile) = signals.tile_ids.remove(id) else {
                return Err(TransformError::TileIdNotFound(*id));
            };
            ui_tile.app.set(*app);
            ui_tile.remote.set(remote.clone());
            TileTree::Tile(ui_tile.into())
        }
        TileTreeDto::Array {
            id,
            direction,
            nodes,
        } => {
            let Some(ui_direction) = signals.directions.remove(id) else {
                return Err(TransformError::TileTreeIdNotFound(*id));
            };
            ui_direction.set(*direction);
            let mut ui_nodes = Vec::with_capacity(nodes.len());
            for node in nodes {
                ui_nodes.push(transform(signals, &node)?.into());
            }
            TileTree::Array {
                id: *id,
                direction: ui_direction,
                nodes: ui_nodes,
            }
        }
    })
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TransformError {
    #[error("[{n}] The tile {0:?} was not found", n = self.name())]
    TileIdNotFound(TileId),

    #[error("[{n}] The tile tree {0:?} was not found", n = self.name())]
    TileTreeIdNotFound(TileId),
}

#[derive(Default)]
struct TileIds {
    tree_ids: HashSet<TileId>,
    tile_ids: HashSet<TileId>,
}

#[derive(Default)]
struct TileSignals {
    directions: HashMap<TileId, XSignal<Direction>>,
    tile_ids: HashMap<TileId, TilePtr>,
}

impl<'l> TilesTreeVisitor<DtoVisitor<'l>> for TileIds {
    fn visit_tree(&mut self, id: TileId, _: Direction) {
        self.tree_ids.insert(id);
    }

    fn visit_tile(&mut self, tile: &TileDto) {
        self.tile_ids.insert(tile.id);
    }
}

impl<'l> TilesTreeVisitor<UiStateVisitor<'l>> for TileSignals {
    fn visit_tree(&mut self, id: TileId, direction: &XSignal<Direction>) {
        self.directions.insert(id, direction.clone());
    }
    fn visit_tile(&mut self, tile: &TilePtr) {
        self.tile_ids.insert(tile.id, tile.clone());
    }
}
