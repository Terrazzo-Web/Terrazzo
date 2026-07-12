use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;

use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::AckRequest;
use crate::api::shared::terminal_schema::RegisterTerminalRequest;
use crate::api::shared::terminal_schema::ResizeRequest;
use crate::api::shared::terminal_schema::SetTitleRequest;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::api::shared::terminal_schema::WriteRequest;
use crate::terminal_id::TerminalId;
use crate::tiles::id::TileId;
use crate::tiles::state::make_state;

make_state!(selected_tab, Option<TerminalId>);

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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum LeaseMessage {
    Init,
    Data(Vec<u8>),
    Eos,
    Error(String),
}

#[server(protocol = Http<Json, Json>)]
pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    super::service::list().await
}

#[server(protocol = Http<Json, Json>)]
pub async fn new_id(remote: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    super::service::new_id(remote, tile).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn write(request: WriteRequest) -> Result<(), ServerFnError> {
    super::service::write(request).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn resize(request: ResizeRequest) -> Result<(), ServerFnError> {
    super::service::resize(request).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn set_title(request: SetTitleRequest) -> Result<(), ServerFnError> {
    super::service::set_title(request).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn set_order(terminals: Vec<TerminalAddress>) -> Result<(), ServerFnError> {
    super::service::set_order(terminals).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn close(terminal: TerminalAddress) -> Result<(), ServerFnError> {
    super::service::close(terminal).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn ack(request: AckRequest) -> Result<(), ServerFnError> {
    super::service::ack(request).await
}

#[server(protocol = Http<Json, StreamingText>)]
pub async fn stream(
    request: RegisterTerminalRequest,
) -> Result<TextStream<ServerFnError>, ServerFnError> {
    super::service::stream(request).await
}
