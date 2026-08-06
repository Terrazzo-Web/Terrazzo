use server_fn::ServerFnError;
use tonic::Status;
use trz_gateway_common::id::ClientName;
use uuid::Uuid;

use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::TabTitle;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::backend::client_service::remote_fn_service;
use crate::processes;
use crate::tiles::id::TileId;

pub async fn new_id(remote: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    let (next, client_name) = NEW_ID_FN.call(remote.clone(), ()).await?;
    let local_client_name = client_name.as_deref();
    let client_name = remote
        .last()
        .map(|name| name.as_ref())
        .or(local_client_name);
    let title = client_name.map_or_else(
        || format!("Terminal {next}"),
        |name| format!("Terminal {name}:{next}"),
    );
    let id = if cfg!(feature = "concise-traces") {
        Uuid::new_v4().to_string()
    } else if let Some(client_name) = client_name {
        format!("T-{client_name}-{next}")
    } else {
        format!("T-{next}")
    };
    Ok(TerminalDef {
        address: TerminalAddress {
            id: id.into(),
            via: remote,
        },
        title: TabTitle {
            shell_title: title,
            override_title: None,
        },
        order: next,
        tile,
    })
}

type NewIdResult = (i32, Option<ClientName>);

remote_fn_service::unary::declare_remote_fn!(
    NEW_ID_FN,
    "terminal.new_id",
    (),
    NewIdResult,
    |server, ()| {
        let client_name = server
            .config()
            .mesh
            .with(|mesh| Some(mesh.as_ref()?.client_name.as_str().into()));
        async move { Ok::<_, Status>((processes::next_terminal_id(), client_name)) }
    }
);
