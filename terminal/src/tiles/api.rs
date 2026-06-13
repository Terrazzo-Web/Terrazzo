use std::sync::Arc;

use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use terrazzo::server;

use super::app::App;
use super::id::TileId;
use crate::api::client_address::ClientAddress;

mod add;
mod add_tab;
mod drop;
mod move_child;
mod mutate;
mod remove;
mod select_child;
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
pub async fn add_tab(
    array_id: TileId,
    after_child: Option<TileId>,
) -> Result<(Arc<Tiles>, TileId), ServerFnError> {
    Ok(add_tab::add_tab(array_id, after_child)?)
}

#[server]
pub async fn move_child(
    array_id: TileId,
    after_child: Option<TileId>,
    moved_child: TileId,
) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(move_child::move_child(array_id, after_child, moved_child)?)
}

#[server]
pub async fn select_child(
    array_id: TileId,
    selected: Option<TileId>,
) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(select_child::select_child(array_id, selected)?)
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
        title: tile.title.clone(),
    })?)
}

#[server]
pub async fn set_title(id: TileId, title: String) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(mutate::mutate_node(id, |tile| Tile {
        id: tile.id,
        app: tile.app,
        remote: tile.remote.clone(),
        title: title.clone(),
    })?)
}

#[server(protocol = Http<Json, Json>)]
pub async fn set_remote(id: TileId, remote: ClientAddress) -> Result<Arc<Tiles>, ServerFnError> {
    Ok(mutate::mutate_node(id, |tile| Tile {
        id: tile.id,
        app: tile.app,
        remote,
        title: tile.title.clone(),
    })?)
}

// Basic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Direction {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "H"))]
    Horizontal,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "V"))]
    Vertical,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "T"))]
    Tabbed,
}

// Basic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(not(feature = "server"), allow(dead_code))]
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
        #[serde(default)]
        selected: Option<TileId>,
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
    #[serde(default)]
    pub remote: ClientAddress,
    #[serde(default)]
    pub title: String,
}

impl Default for Tiles {
    fn default() -> Self {
        Tile {
            id: TileId::first_tile_id(),
            app: Default::default(),
            remote: Default::default(),
            title: "New tile".into(),
        }
        .into()
    }
}

impl From<Tile> for Tiles {
    fn from(tile: Tile) -> Self {
        Self::Tile(tile)
    }
}
