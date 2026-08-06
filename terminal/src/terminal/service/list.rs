use std::sync::Arc;

use server_fn::ServerFnError;
use tonic::Status;
use tracing::debug;
use trz_gateway_common::id::ClientName;

use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::backend::Server;
use crate::backend::client_service::remote_fn_service;
use crate::processes;

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
    debug!("Found list {response:?}");
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
