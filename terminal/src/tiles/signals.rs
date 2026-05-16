#![cfg(feature = "client")]

use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use terrazzo::envelope;
use terrazzo::prelude::XSignal;

use super::api::Direction;
use super::api::Tile as TileDto;
use super::api::Tiles as TilesDto;
use super::app::App;
use super::id::TileId;
use super::visitor::TilesTreeVisitor;
use super::visitor::UiStateVisitor;
use crate::frontend::remotes::Remote;

pub enum Tiles {
    Tile(TilePtr),
    Array {
        id: TileId,
        direction: XSignal<Direction>,
        nodes: Vec<Rc<Tiles>>,
    },
}

#[envelope]
pub struct Tile {
    pub id: TileId,
    pub app: XSignal<App>,
    pub remote: XSignal<Remote>,
}

impl Tiles {
    pub fn update(&self, tiles: &TilesDto) -> Self {
        let mut signals = TileSignals::default();
        signals.visit_node(self);
        transform(&mut signals, tiles)
    }
}

fn transform(signals: &mut TileSignals, tile_tree_dto: &TilesDto) -> Tiles {
    match tile_tree_dto {
        TilesDto::Tile(TileDto { id, app, remote }) => {
            let ui_tile = if let Some(ui_tile) = signals.tile_ids.remove(id) {
                ui_tile.app.set(*app);
                ui_tile.remote.set(remote.clone());
                ui_tile
            } else {
                Tile {
                    id: *id,
                    app: XSignal::new(format!("app-{id}"), *app),
                    remote: XSignal::new(format!("remote-{id}"), remote.clone()),
                }
                .into()
            };
            Tiles::Tile(ui_tile)
        }
        TilesDto::Array {
            id,
            direction,
            nodes,
        } => {
            let ui_direction = if let Some(ui_direction) = signals.directions.remove(id) {
                ui_direction.set(*direction);
                ui_direction
            } else {
                XSignal::new(format!("direction-{id}"), *direction)
            };
            let mut ui_nodes = Vec::with_capacity(nodes.len());
            for node in nodes {
                ui_nodes.push(transform(signals, &node).into());
            }
            Tiles::Array {
                id: *id,
                direction: ui_direction,
                nodes: ui_nodes,
            }
        }
    }
}

impl Default for Tiles {
    fn default() -> Self {
        let id = TileId::first_tile_id();
        Tile {
            id,
            app: XSignal::new(format!("app-{id}"), App::default()),
            remote: XSignal::new(format!("remote-{id}"), Remote::default()),
        }
        .into()
    }
}

impl From<Tile> for Tiles {
    fn from(tile: Tile) -> Self {
        Self::Tile(tile.into())
    }
}

#[derive(Default)]
struct TileSignals {
    directions: HashMap<TileId, XSignal<Direction>>,
    tile_ids: HashMap<TileId, TilePtr>,
}

impl<'l> TilesTreeVisitor<UiStateVisitor<'l>> for TileSignals {
    fn visit_tree(&mut self, id: TileId, direction: &XSignal<Direction>) {
        self.directions.insert(id, direction.clone());
    }
    fn visit_tile(&mut self, tile: &TilePtr) {
        self.tile_ids.insert(tile.id, tile.clone());
    }
}

#[derive(Clone)]
pub struct TilesCmp<T>(T);

impl<T> TilesCmp<T> {
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: AsRef<Tiles>> PartialEq for TilesCmp<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Tiles::Tile(a), Tiles::Tile(b)) => a.id == b.id,
            (
                Tiles::Array {
                    id: a_id,
                    nodes: a_nodes,
                    ..
                },
                Tiles::Array {
                    id: b_id,
                    nodes: b_nodes,
                    ..
                },
            ) => {
                a_id == b_id
                    && a_nodes.len() == b_nodes.len()
                    && Iterator::zip(a_nodes.iter(), b_nodes.iter())
                        .all(|(a, b)| TilesCmp(a) == TilesCmp(b))
            }
            _ => false,
        }
    }
}

impl<T: AsRef<Tiles>> Eq for TilesCmp<T> {}

impl<T: AsRef<Tiles>> AsRef<Tiles> for TilesCmp<T> {
    fn as_ref(&self) -> &Tiles {
        self.0.as_ref()
    }
}

impl<T> Deref for TilesCmp<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::fmt::Debug for TilesCmp<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tiles(...)")
    }
}
