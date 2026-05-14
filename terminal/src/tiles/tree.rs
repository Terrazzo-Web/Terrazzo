use server_fn::ServerFnError;
use terrazzo::server;

use super::state::TREE;
use crate::tiles::id::TileId;

#[server]
pub async fn get() -> Result<TileTree, ServerFnError> {
    Ok(TREE.lock()?.clone())
}

#[server]
pub async fn add(
    with_direction: Direction,
    next_to: TileId,
    side: Side,
) -> Result<TileTree, ServerFnError> {
    Ok(super::state::add_node(
        &mut *TREE.lock()?,
        with_direction,
        next_to,
        side,
    ))
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum TileTree {
    Node(TileNode),
    Array {
        direction: Direction,
        nodes: Vec<TileTree>,
    },
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TileNode {
    pub id: TileId,
}

#[cfg(feature = "server")]
impl TileTree {
    pub fn zero() -> Self {
        TileTree::Array {
            direction: Direction::Horizontal,
            nodes: Default::default(),
        }
    }

    pub fn take(&mut self) -> Self {
        std::mem::replace(self, Self::zero())
    }
}
