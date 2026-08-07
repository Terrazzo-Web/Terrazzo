use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;

use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::ResizeRequest;
use crate::api::shared::terminal_schema::SetTitleRequest;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
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
    Ok(super::service::tile_id::SET_TILE_ID_FN
        .call(remote, (terminal_id, tile_id))
        .await?)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum LeaseMessage {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "I"))]
    Init,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "B"))]
    Base64(String),
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "U"))]
    Utf8(String),
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    Eos,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    Error(String),
}

#[server(protocol = Http<Json, Json>)]
pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    super::service::list::list().await
}

#[server(protocol = Http<Json, Json>)]
pub async fn new_id(remote: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    super::service::new_id::new_id(remote, tile).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn write(terminal: TerminalAddress, data: String) -> Result<(), ServerFnError> {
    super::service::write::write(terminal, data).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn resize(request: ResizeRequest) -> Result<(), ServerFnError> {
    super::service::resize::resize(request).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn set_title(request: SetTitleRequest) -> Result<(), ServerFnError> {
    super::service::title::set_title(request).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn set_order(terminals: Vec<TerminalAddress>) -> Result<(), ServerFnError> {
    super::service::order::set_order(terminals).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn close(terminal: TerminalAddress) -> Result<(), ServerFnError> {
    super::service::close::close(terminal).await
}

#[server(protocol = Http<Json, Json>)]
pub async fn ack(terminal: TerminalAddress, ack: usize) -> Result<(), ServerFnError> {
    super::service::ack::ack(terminal, ack).await
}

#[cfg(feature = "client")]
pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<impl futures::Stream<Item = Result<String, ServerFnError>>, ServerFnError> {
    Ok(super::streams::client::stream(mode, terminal_def).await?)
}

#[server(protocol = Http<Json, StreamingText>)]
pub async fn pipe() -> Result<TextStream, ServerFnError> {
    Ok(crate::terminal::streams::server::pipe().await)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PipeMessage {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "t"))]
    pub terminal_id: TerminalId,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "c"))]
    pub chunk: Result<String, ServerFnError>,
}

#[server(protocol = Http<Json, Json>)]
pub async fn add_stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<(), ServerFnError> {
    Ok(crate::terminal::streams::server::add_stream(mode, terminal_def).await?)
}
