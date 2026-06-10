use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;

use self::diagnostics::debug;
use crate::frontend::remotes::Remote;
use crate::frontend::remotes::RemotesState;

terrazzo_css::import_style!(style, "remotes_ui.scss");

#[html]
#[template(tag = div)]
pub fn show_remote(#[signal] mut cur_remote: Remote) -> XElement {
    let remotes_state = RemotesState::new();

    let cur_remote_name;
    let cur_remote_name = if cur_remote.is_empty() {
        "Local"
    } else {
        cur_remote_name = cur_remote.to_string();
        &cur_remote_name
    };
    tag(
        class = style::REMOTES,
        #[cfg(not(feature = "client-prod"))]
        class = "show-remote",
        div(
            "{cur_remote_name}",
            class = style::SHOW_CURRENT,
            mouseenter = remotes_state.mouseenter(),
        ),
        mouseleave = remotes_state.mouseleave(),
        remotes_state.show_remotes_dropdown(
            move |remote| {
                let remote_name = if remote.is_empty() {
                    "Local".into()
                } else {
                    format!("{remote} ⏎")
                };
                let remote_class = (cur_remote == *remote).then_some(style::CURRENT);
                (remote_name, remote_class)
            },
            move |_, new_remote| {
                debug!("Set text editor remote to {new_remote:?}");
                cur_remote_mut.set(new_remote)
            },
        ),
    )
}
