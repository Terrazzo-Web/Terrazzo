use std::sync::Arc;

use server_fn::ServerFnError;
use terrazzo::server;

use super::app::App;
use super::id::TileId;
use crate::api::client_address::ClientAddress;

mod add;
mod drop;
mod mutate;
mod remove;
mod state;
mod tests;

#[server]
pub async fn get() -> Result<Arc<Tiles>, ServerFnError> {
    Ok(state::TREE.lock()?.clone().unwrap_or_default())
}

#[server]
pub async fn add(
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(add::add_node(with_direction, next_to, side)?)
}

#[server]
pub async fn remove(id: TileId) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(remove::remove_node(id)?)
}

#[server]
pub async fn set_app(id: TileId, app: App) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(mutate::mutate_node(id, |tile| Tile {
        id: tile.id,
        app,
        remote: tile.remote.clone(),
    })?)
}

#[server]
pub async fn set_remote(
    id: TileId,
    remote: Option<ClientAddress>,
) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(mutate::mutate_node(id, |tile| Tile {
        id: tile.id,
        app: tile.app,
        remote,
    })?)
}

// Basic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Direction {
    Vertical,
    Horizontal,
}

// Basic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Side {
    Before,
    After,
}

// Basic
#[derive(Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Tiles {
    Tile(Tile),
    Array {
        id: TileId,
        direction: Direction,
        nodes: Vec<Arc<Tiles>>,
    },
}

// Basic
#[derive(Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Tile {
    pub id: TileId,
    pub app: App,
    pub remote: Option<ClientAddress>,
}

impl Default for Tiles {
    fn default() -> Self {
        Tile {
            id: TileId::first_tile_id(),
            app: Default::default(),
            remote: Default::default(),
        }
        .into()
    }
}

impl From<Tile> for Tiles {
    fn from(tile: Tile) -> Self {
        Self::Tile(tile)
    }
}
