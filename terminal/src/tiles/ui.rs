#![cfg(feature = "client")]

use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use crate::frontend::remotes::Remote;
use crate::tiles::tree::Direction;
use crate::tiles::tree::TileNode;
use crate::tiles::tree::TileTree;

use super::app::App;

terrazzo_css::import_style!(style, "ui.scss");

#[autoclone]
pub fn show_tiles() -> XElement {
    let tiles = XSignal::new("tiles", Arc::new(TileTree::default()));
    spawn_local(async move {
        autoclone!(tiles);
        tiles.force(super::tree::get().await.unwrap());
    });
    show_tiles_tree(tiles)
}

#[html]
#[template(tag = div)]
fn show_tiles_tree(#[signal] tiles: Arc<TileTree>) -> XElement {
    show_tiles_rec(&tiles)
}

#[html]
fn show_tiles_rec(tiles: &TileTree) -> XElement {
    match tiles {
        TileTree::Node(node) => show_tile(node),
        TileTree::Array { direction, nodes } => tag(
            class = match direction {
                Direction::Vertical => style::HORIZONTAL_TILE,
                Direction::Horizontal => todo!(),
            },
            nodes.iter().map(|n| n.as_ref()).map(show_tiles_rec)..,
        ),
    }
}

#[html]
fn show_tile(node: &TileNode) -> XElement {
    div(
        key = "app",
        div(
            class = style::APP_SHELL,
            show_app(node.app, node.remote.clone()),
            maybe_logs_panel(node.remote.clone()),
        ),
    )
}

#[html]
fn show_app(app: App, remote: XSignal<Remote>) -> XElement {
    div(
        class = style::APP_CONTENT,
        match app {
            #[cfg(feature = "terminal")]
            App::Terminal => div(move |t| crate::terminal::terminals(t, remote.clone())),

            #[cfg(feature = "text-editor")]
            App::TextEditor => div(move |t| crate::text_editor::ui::text_editor(t, remote.clone())),

            #[cfg(feature = "converter")]
            App::Converter => div(move |t| crate::converter::ui::converter(t, remote.clone())),

            #[cfg(feature = "port-forward")]
            App::PortForward => {
                div(move |t| crate::portforward::ui::port_forward(t, remote.clone()))
            }
        },
    )
}
