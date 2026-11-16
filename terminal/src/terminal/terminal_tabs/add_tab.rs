use terrazzo::autoclone;
use terrazzo::prelude::*;
use terrazzo::template;

use self::diagnostics::Instrument as _;
use self::diagnostics::warn;
use crate::api::client_address::ClientAddress;
use crate::frontend::remotes::Remotes;
use crate::terminal::client as terminal_api;
use crate::terminal::ui::TerminalsState;

#[template(wrap = true)]
pub fn active(#[signal] remotes: Remotes) -> XAttributeValue {
    if let Remotes::Some { .. } = remotes {
        Some(super::style::ACTIVE)
    } else {
        None
    }
}

#[autoclone]
pub fn create_terminal(state: TerminalsState, client_address: ClientAddress) {
    let task = async move {
        autoclone!(state, client_address);
        let terminal_def = match terminal_api::new_id(client_address.clone(), state.tile.id).await {
            Ok(id) => id,
            Err(error) => {
                warn!("Failed to allocate new ID: {error}");
                return;
            }
        };
        state.add_terminal(terminal_def);
    };
    wasm_bindgen_futures::spawn_local(task.in_current_span());
}
