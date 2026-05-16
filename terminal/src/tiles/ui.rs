#![cfg(feature = "client")]

use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use super::app::App;
use crate::tiles::api::Direction;
use crate::tiles::api::Tile;
use crate::tiles::api::TileTree;

terrazzo_css::import_style!(style, "ui.scss");

#[autoclone]
pub fn show_tiles() -> XElement {
    let tiles = XSignal::new("tiles", Arc::new(TileTree::default()));
    spawn_local(async move {
        autoclone!(tiles);
        let mut batch = Batch::use_batch("load tiles");
        let tree = super::api::get().await.unwrap();
        tiles.update(|tiles| {
            if **tiles == TileTree::default() {
                Some(tree)
            } else {
                batch.forget();
                None
            }
        });
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
        TileTree::Tile(node) => div(
            key = node.id.to_string(),
            class = style::APP_TILE,
            show_app(node),
            #[cfg(feature = "logs-panel")]
            crate::logs::panel(node.remote.clone()),
        ),
        TileTree::Array {
            id: _,
            direction,
            nodes,
        } => div(
            class = match direction {
                Direction::Vertical => style::HORIZONTAL_TILE,
                Direction::Horizontal => style::VERTICAL_TILE,
            },
            nodes.iter().map(|n| n.as_ref()).map(show_tiles_rec)..,
        ),
    }
}

#[html]
fn show_app(node: &Tile) -> XElement {
    // TODO how to get a signal for the remote of the tile?
    // let remote: XSignal<Remote> = XSignal::new("remote", Remote::default());
    div(
        class = style::APP_CONTENT,
        match node.app {
            #[cfg(feature = "terminal")]
            App::Terminal => div(move |t| crate::terminal::terminals(t)),

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
