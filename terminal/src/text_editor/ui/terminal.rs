//! The editor uses the tile's regular terminal tabs as its only enabled state.
#![cfg(feature = "terminal")]

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::resize_event::ResizeEvent;
use terrazzo::widgets::tabs::TabsDescriptor as _;
use wasm_bindgen_futures::spawn_local;

use super::super::file_path::FilePath;
use super::super::fsio::ROOT_FILE_PATH;
use super::super::manager::TextEditorManager;
use crate::api::shared::terminal_schema::TabTitle;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::assets::icons;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::frontend::resize_bar::resize_bar_vert;
use crate::terminal::ui::TerminalTabs;
use crate::terminal::ui::TerminalsState;

terrazzo_css::import_style!(style, "terminal.scss");

impl TextEditorManager {
    #[html]
    pub(super) fn terminal_toggle(&self) -> XElement {
        let state = TerminalsState::new(self.tile.clone());
        let base = self.path.base.clone();
        let remote = self.remote.clone();
        img(
            class = style::TOGGLE_TERMINAL,
            #[cfg(not(feature = "client-prod"))]
            class = "toggle-terminal",
            class %= active(state.terminal_tabs.clone()),
            src = icons::terminal(),
            title = "Toggle terminal",
            click = move |_| {
                let tabs = state.terminal_tabs.get_value_untracked();
                if !tabs.tab_descriptors().is_empty() {
                    for tab in tabs.tab_descriptors() {
                        let address = tab.address.clone();
                        spawn_local(async move {
                            crate::terminal::client::close(&address, None).await;
                        });
                    }
                    return;
                }
                // Allocate locally so the tab signal changes in this click, before any await.
                // The regular stream path creates the PTY when the tab is rendered.
                let id = format!(
                    "E-{}-{}-{}",
                    state.tile.id,
                    web_sys::js_sys::Date::now(),
                    web_sys::js_sys::Math::random()
                );
                let path = FilePath {
                    base: base.get_value_untracked(),
                    file: ROOT_FILE_PATH.clone(),
                };
                let base_path = path.with_base_path(|path| path.to_string_lossy().into_owned());
                state.add_terminal(TerminalDef {
                    address: TerminalAddress {
                        id: id.into(),
                        via: remote.clone(),
                    },
                    title: TabTitle {
                        shell_title: "Terminal".into(),
                        override_title: None,
                    },
                    order: 0,
                    tile: state.tile.id,
                    base_path: Some(base_path),
                });
            },
        )
    }

    #[autoclone]
    #[html]
    pub(super) fn terminal_panel(&self) -> XElement {
        let tile = self.tile.clone();
        let resize = MousemoveManager::new2(|| ResizeEvent::signal().force(()));
        div(
            class = style::EDITOR_TERMINAL,
            #[cfg(not(feature = "client-prod"))]
            class = "editor-terminal",
            style::display %= display(tile.terminal_ui.terminal_tabs.clone()),
            style::flex %= height(resize.delta.clone()),
            resize_bar_vert(resize, Default::default()),
            div(
                class = style::EDITOR_TERMINAL_CONTENT,
                div(move |template| {
                    autoclone!(tile);
                    crate::terminal::ui::terminals(template, tile.clone())
                }),
            ),
        )
    }
}

#[template(wrap = true)]
fn active(#[signal] tabs: TerminalTabs) -> XAttributeValue {
    (!tabs.tab_descriptors().is_empty()).then_some(style::ACTIVE)
}

#[template(wrap = true)]
fn display(#[signal] tabs: TerminalTabs) -> XAttributeValue {
    if tabs.tab_descriptors().is_empty() {
        "none"
    } else {
        "flex"
    }
}

#[template(wrap = true)]
fn height(#[signal] position: Option<Position>) -> XAttributeValue {
    format!(
        "0 0 clamp(100px, calc(240px - {}px), 70%)",
        position.unwrap_or_default().y
    )
}
