use std::collections::HashMap;

use server_fn::ServerFnError;
use tonic::Status;

use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::backend::client_service::remote_fn_service;
use crate::processes::get_processes;
use crate::terminal_id::TerminalId;

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
