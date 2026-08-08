use tonic::Status;

use crate::backend::client_service::remote_fn_service;
use crate::processes::get_processes;
use crate::terminal::api::SET_TILE_ID;
use crate::terminal_id::TerminalId;
use crate::tiles::id::TileId;

pub async fn set_tile_id(terminal_id: TerminalId, tile_id: TileId) -> Result<(), Status> {
    let Some(mut entry) = get_processes().get_mut(&terminal_id) else {
        return Err(Status::not_found(format!(
            "Terminal '{terminal_id}' not found"
        )));
    };
    entry.0.tile = tile_id;
    Ok(())
}

remote_fn_service::unary::declare_remote_fn!(
    SET_TILE_ID_FN,
    SET_TILE_ID,
    (TerminalId, TileId),
    (),
    |_server, (terminal_id, tile_id)| set_tile_id(terminal_id, tile_id)
);
