use std::collections::HashMap;

use futures::StreamExt as _;
use futures::TryStreamExt as _;
use futures::stream::BoxStream;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use terrazzo_pty::lease::LeaseItem;
use tonic::Status;
use uuid::Uuid;

use super::api::*;
use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::*;
use crate::backend::client_service::remote_fn_service;
use crate::backend::throttling_stream::ThrottleProcessOutput;
use crate::processes;
use crate::processes::get_processes;
use crate::terminal_id::TerminalId;
use crate::tiles::id::TileId;
use crate::utils::ndjson_utils::serialize_line;

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
    super::api::SET_TILE_ID,
    SetTileIdRequest,
    (),
    |_server, arg| set_tile_id(arg.terminal_id, arg.tile_id)
);

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ListRequest {
    visited: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SetOrderEntry {
    terminal_id: TerminalId,
    order: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NewIdResult {
    next: i32,
    client_name: Option<String>,
}

pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    let mut terminals = LIST_FN
        .call(ClientAddress::default(), ListRequest { visited: vec![] })
        .await?;
    terminals.sort_by_key(|terminal| terminal.order);
    Ok(terminals)
}

async fn list_impl(
    server: &std::sync::Arc<crate::backend::Server>,
    mut request: ListRequest,
) -> Result<Vec<TerminalDef>, Status> {
    let mut response = processes::list::list();
    for client_name in server.connections().clients() {
        if request
            .visited
            .iter()
            .any(|name| name == client_name.as_ref())
        {
            continue;
        }
        let mut visited = request.visited.clone();
        visited.push(client_name.to_string());
        let address = ClientAddress::from(client_name.clone());
        let Ok(mut terminals) = LIST_FN.call(address, ListRequest { visited }).await else {
            continue;
        };
        for terminal in &mut terminals {
            let mut via = terminal.address.via.to_vec();
            via.push(client_name.clone());
            terminal.address.via = via.into();
        }
        response.append(&mut terminals);
    }
    request.visited.clear();
    Ok(response)
}

remote_fn_service::unary::declare_remote_fn!(
    LIST_FN,
    "terminal.list",
    ListRequest,
    Vec<TerminalDef>,
    |server, request| {
        let server = server.clone();
        async move { list_impl(&server, request).await }
    }
);

pub async fn new_id(remote: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    let result = NEW_ID_FN.call(remote.clone(), ()).await?;
    let next = result.next;
    let local_client_name = result.client_name.as_deref();
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

remote_fn_service::unary::declare_remote_fn!(
    NEW_ID_FN,
    "terminal.new_id",
    (),
    NewIdResult,
    |server, ()| {
        let client_name = server
            .config()
            .mesh
            .with(|mesh| Some(mesh.as_ref()?.client_name.as_str().to_owned()));
        async move {
            Ok::<_, Status>(NewIdResult {
                next: processes::next_terminal_id(),
                client_name,
            })
        }
    }
);

pub async fn write(request: WriteRequest) -> Result<(), ServerFnError> {
    Ok(WRITE_FN.call(request.terminal.via.clone(), request).await?)
}
remote_fn_service::unary::declare_remote_fn!(
    WRITE_FN,
    "terminal.write",
    WriteRequest,
    (),
    |_server, request: WriteRequest| async move {
        processes::write::write(&request.terminal.id, request.data.as_bytes())
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }
);

pub async fn resize(request: ResizeRequest) -> Result<(), ServerFnError> {
    Ok(RESIZE_FN
        .call(request.terminal.via.clone(), request)
        .await?)
}
remote_fn_service::unary::declare_remote_fn!(
    RESIZE_FN,
    "terminal.resize",
    ResizeRequest,
    (),
    |_server, request: ResizeRequest| async move {
        processes::resize::resize(
            &request.terminal.id,
            request.size.rows,
            request.size.cols,
            request.force,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))
    }
);

pub async fn set_title(request: SetTitleRequest) -> Result<(), ServerFnError> {
    Ok(SET_TITLE_FN
        .call(request.terminal.via.clone(), request)
        .await?)
}
remote_fn_service::unary::declare_remote_fn!(
    SET_TITLE_FN,
    "terminal.set_title",
    SetTitleRequest,
    (),
    |_server, request: SetTitleRequest| async move {
        processes::set_title::set_title(&request.terminal.id, request.title)
            .map_err(|e| Status::not_found(e.to_string()))
    }
);

pub async fn set_order(terminals: Vec<TerminalAddress>) -> Result<(), ServerFnError> {
    let mut groups: HashMap<ClientAddress, Vec<SetOrderEntry>> = HashMap::new();
    for (order, terminal) in terminals.into_iter().enumerate() {
        groups.entry(terminal.via).or_default().push(SetOrderEntry {
            terminal_id: terminal.id,
            order: order as i32,
        });
    }
    for (remote, entries) in groups {
        SET_ORDER_FN.call(remote, entries).await?;
    }
    Ok(())
}
remote_fn_service::unary::declare_remote_fn!(
    SET_ORDER_FN,
    "terminal.set_order",
    Vec<SetOrderEntry>,
    (),
    |_server, entries: Vec<SetOrderEntry>| async move {
        for entry in entries {
            if let Some(mut process) = get_processes().get_mut(&entry.terminal_id) {
                process.0.order = entry.order;
            }
        }
        Ok::<_, Status>(())
    }
);

pub async fn close(terminal: TerminalAddress) -> Result<(), ServerFnError> {
    Ok(CLOSE_FN.call(terminal.via, terminal.id).await?)
}
remote_fn_service::unary::declare_remote_fn!(
    CLOSE_FN,
    "terminal.close",
    TerminalId,
    (),
    |_server, terminal_id: TerminalId| async move {
        processes::close::close(&terminal_id).map_err(|e| Status::not_found(e.to_string()))
    }
);

pub async fn ack(request: AckRequest) -> Result<(), ServerFnError> {
    Ok(ACK_FN.call(request.terminal.via.clone(), request).await?)
}
remote_fn_service::unary::declare_remote_fn!(
    ACK_FN,
    "terminal.ack",
    AckRequest,
    (),
    |_server, request: AckRequest| async move {
        crate::backend::throttling_stream::ack(&request.terminal.id, request.ack);
        Ok::<_, Status>(())
    }
);

pub async fn stream(
    request: RegisterTerminalRequest,
) -> Result<TextStream<ServerFnError>, ServerFnError> {
    let remote = request.def.address.via.clone();
    let stream = STREAM_FN.call(remote, request).await?;
    let stream = stream.map_ok(|item| {
        serialize_line(&item).unwrap_or_else(|error| {
            serialize_line(&LeaseMessage::Error(error.to_string()))
                .expect("serializing a string cannot fail")
        })
    });
    Ok(TextStream::new(stream.map_err(Into::into)))
}

remote_fn_service::streaming::declare_remote_fn!(
    STREAM_FN,
    "terminal.stream",
    RegisterTerminalRequest,
    LeaseMessage,
    |server, request: RegisterTerminalRequest| {
        let terminal_id = request.def.address.id.clone();
        let create = request.mode == RegisterTerminalMode::Create;
        let server = server.clone();
        futures::stream::once(async move {
            let open_server = server.clone();
            let stream =
                processes::stream::open_stream(&server, request.def, create, |_| async move {
                    if !create {
                        return Err(OpenProcessError::NotFound);
                    }
                    let shell = open_server
                        .config()
                        .server
                        .with(|config| config.terminal_shell.clone());
                    ProcessIO::open(None::<String>, STREAMING_WINDOW_SIZE, shell).await
                })
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            Ok::<_, Status>(ThrottleProcessOutput::new(terminal_id, stream))
        })
        .flat_map(
            |result| -> BoxStream<'static, Result<LeaseMessage, Status>> {
                match result {
                    Ok(stream) => Box::pin(
                        futures::stream::once(async { Ok(LeaseMessage::Init) })
                            .chain(stream.map(|item| Ok(LeaseMessage::from(item)))),
                    ),
                    Err(error) => Box::pin(futures::stream::once(async move { Err(error) })),
                }
            },
        )
    }
);

impl From<LeaseItem> for LeaseMessage {
    fn from(item: LeaseItem) -> Self {
        match item {
            LeaseItem::Data(data) => Self::Data(data.to_vec()),
            LeaseItem::EOS => Self::Eos,
            LeaseItem::Error(error) => Self::Error(error.to_string()),
        }
    }
}
