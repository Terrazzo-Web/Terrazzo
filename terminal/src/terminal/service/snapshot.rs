use server_fn::ServerFnError;
use terrazzo_pty::OutputSnapshot;
use tonic::Status;

use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalSnapshot;
use crate::backend::client_service::remote_fn_service;

pub async fn put(
    terminal: TerminalAddress,
    snapshot: TerminalSnapshot,
) -> Result<(), ServerFnError> {
    Ok(SNAPSHOT_FN
        .call(terminal.via.clone(), (terminal, snapshot))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    SNAPSHOT_FN,
    "terminal.snapshot",
    (TerminalAddress, TerminalSnapshot),
    (),
    |_server, (terminal, snapshot): (TerminalAddress, TerminalSnapshot)| async move {
        let entry = crate::processes::get_processes()
            .get(&terminal.id)
            .ok_or_else(|| Status::not_found("ProcessGone"))?
            .1
            .clone();
        if entry.process_id != snapshot.process {
            return Err(Status::failed_precondition("ProcessChanged"));
        }
        entry
            .output_buffer
            .save_snapshot(OutputSnapshot {
                position: snapshot.position,
                data: snapshot.data.into_bytes().into(),
            })
            .map_err(Status::invalid_argument)
    }
);
