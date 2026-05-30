use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use terrazzo::server;

use crate::api::client_address::ClientAddress;
use crate::terminal_id::TerminalId;
use crate::tiles::id::TileId;

#[server(protocol = Http<Json, Json>)]
#[cfg_attr(feature = "server", nameth::nameth)]
pub async fn set_tile_id(
    remote: ClientAddress,
    terminal_id: TerminalId,
    tile_id: TileId,
) -> Result<(), ServerFnError> {
    Ok(super::service::SET_TILE_ID_FN
        .call(
            remote,
            SetTileIdRequest {
                terminal_id,
                tile_id,
            },
        )
        .await?)
}

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct SetTileIdRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "t"))]
    pub terminal_id: TerminalId,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "i"))]
    pub tile_id: TileId,
}
