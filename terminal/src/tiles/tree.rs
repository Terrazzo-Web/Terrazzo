use std::sync::Arc;

use server_fn::ServerFnError;
use terrazzo::server;

use super::app::App;
use super::id::TileId;
use crate::api::client_address::ClientAddress;

#[server]
pub async fn get() -> Result<Arc<TileTree>, ServerFnError> {
    Ok(super::state::TREE.lock()?.clone().unwrap_or_default())
}

#[server]
pub async fn add(
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> Result<Arc<TileTree>, ServerFnError> {
    Ok(super::state::add_node(with_direction, next_to, side))
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
pub enum TileTree {
    Node(TileNode),
    Array {
        direction: Direction,
        nodes: Vec<Arc<TileTree>>,
    },
}

// Basic
#[derive(Debug, PartialEq, Eq)]
// Serde
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TileNode {
    pub id: TileId,
    pub app: App,
    pub remote: Option<ClientAddress>,
}

impl Default for TileTree {
    fn default() -> Self {
        Self::Node(TileNode {
            id: TileId::first_tile_id(),
            app: Default::default(),
            remote: Default::default(),
        })
    }
}
