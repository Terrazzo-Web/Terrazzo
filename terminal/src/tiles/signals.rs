#![cfg(feature = "client")]

use std::rc::Rc;

use terrazzo::prelude::XSignal;

use super::api::Direction;
use super::api::Tile as TileDto;
use super::api::TileTree as TileTreeDto;
use super::app::App;
use super::id::TileId;
use crate::frontend::remotes::Remote;

pub enum TileTree {
    Node(Tile),
    Array {
        direction: XSignal<Direction>,
        nodes: Vec<Rc<TileTree>>,
    },
}

pub struct Tile {
    pub id: TileId,
    pub app: XSignal<App>,
    pub remote: XSignal<Remote>,
}
