use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt as _;
use futures::TryStreamExt as _;
use futures::stream::BoxStream;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use terrazzo_pty::lease::LeaseItem;
use tonic::Status;
use trz_gateway_common::id::ClientName;
use uuid::Uuid;

use super::api::*;
use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::*;
use crate::backend::Server;
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
    (TerminalId, TileId),
    (),
    |_server, (terminal_id, tile_id)| set_tile_id(terminal_id, tile_id)
);

pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    let mut terminals = LIST_FN.call(ClientAddress::default(), vec![]).await?;
    terminals.sort_by_key(|terminal| terminal.order);
    Ok(terminals)
}

type ListRequest = Vec<ClientName>;

async fn list_impl(
    server: &Arc<Server>,
    mut request: ListRequest,
) -> Result<Vec<TerminalDef>, Status> {
    let mut response = processes::list::list();
    for client_name in server.connections().clients() {
        if request.iter().any(|name| name == &client_name) {
            continue;
        }
        let mut visited = request.clone();
        visited.push(client_name.clone());
        let address = ClientAddress::from(client_name.clone());
        let Ok(mut terminals) = LIST_FN.call(address, visited).await else {
            continue;
        };
        for terminal in &mut terminals {
            let mut via = terminal.address.via.to_vec();
            via.push(client_name.clone());
            terminal.address.via = via.into();
        }
        response.append(&mut terminals);
    }
    request.clear();
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

pub async fn write(terminal: TerminalAddress, data: String) -> Result<(), ServerFnError> {
    Ok(WRITE_FN
        .call(terminal.via.clone(), (terminal, data))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    WRITE_FN,
    "terminal.write",
    (TerminalAddress, String),
    (),
    |_server, (terminal, data)| async move {
        processes::write::write(&terminal.id, data.as_bytes())
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
    let mut groups: HashMap<ClientAddress, Vec<(TerminalId, i32)>> = HashMap::new();
    for (order, terminal) in terminals.into_iter().enumerate() {
        let entry = groups.entry(terminal.via);
        let entry = entry.or_default();
        entry.push((terminal.id, order as i32));
    }
    for (remote, entries) in groups {
        SET_ORDER_FN.call(remote, entries).await?;
    }
    Ok(())
}
remote_fn_service::unary::declare_remote_fn!(
    SET_ORDER_FN,
    "terminal.set_order",
    Vec<(TerminalId, i32)>,
    (),
    |_server, entries: Vec<(TerminalId, i32)>| async move {
        for (terminal_id, order) in entries {
            if let Some(mut process) = get_processes().get_mut(&terminal_id) {
                process.0.order = order;
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

pub async fn ack(terminal: TerminalAddress, ack: usize) -> Result<(), ServerFnError> {
    Ok(ACK_FN
        .call(terminal.via.clone(), (terminal.id, ack))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    ACK_FN,
    "terminal.ack",
    (TerminalId, usize),
    (),
    |_server, (terminal_id, ack)| async move {
        crate::backend::throttling_stream::ack(&terminal_id, ack);
        Ok::<_, Status>(())
    }
);

pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<TextStream<ServerFnError>, ServerFnError> {
    let remote = terminal_def.address.via.clone();
    let stream = STREAM_FN.call(remote, (mode, terminal_def)).await?;
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
    (RegisterTerminalMode, TerminalDef),
    LeaseMessage,
    |server, (mode, terminal_def)| {
        let terminal_id = terminal_def.address.id.clone();
        let create = mode == RegisterTerminalMode::Create;
        let server = server.clone();
        futures::stream::once(async move {
            let open_server = server.clone();
            let stream =
                processes::stream::open_stream(&server, terminal_def, create, |_| async move {
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
