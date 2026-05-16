#![cfg(feature = "client")]

use std::rc::Rc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use super::app::App;
use super::signals::TilesCmp;
use crate::tiles::api::Direction;
use crate::tiles::signals::TilePtr;
use crate::tiles::signals::Tiles;

terrazzo_css::import_style!(style, "ui.scss");

#[autoclone]
pub fn show_tiles() -> XElement {
    let tiles = XSignal::new("tiles", TilesCmp::new(Rc::new(Tiles::default())));
    spawn_local(async move {
        autoclone!(tiles);
        let _batch = Batch::use_batch("load tiles");
        let tree = super::api::get().await.unwrap();
        tiles.update(|old| Some(TilesCmp::new(Rc::new(old.update(&tree)))));
    });
    show_tiles_tree(tiles)
}

#[template(tag = div)]
fn show_tiles_tree(#[signal] tiles: TilesCmp<Rc<Tiles>>) -> XElement {
    show_tiles_rec(&tiles)
}

#[html]
fn show_tiles_rec(tiles: &Tiles) -> XElement {
    match tiles {
        Tiles::Tile(tile) => div(
            key = tile.id.to_string(),
            class = style::APP_TILE,
            show_app(tile.clone(), tile.app),
            #[cfg(feature = "logs-panel")]
            crate::logs::panel(tile.remote.clone()),
        ),
        Tiles::Array {
            id: _,
            direction,
            nodes,
        } => div(
            class %= direction_class(direction.clone()),
            nodes.iter().map(|n| n.as_ref()).map(show_tiles_rec)..,
        ),
    }
}

#[template(wrap = true)]
pub fn direction_class(#[signal] direction: Direction) -> XAttributeValue {
    match direction {
        Direction::Vertical => style::HORIZONTAL_TILE,
        Direction::Horizontal => style::VERTICAL_TILE,
    }
}
#[html]
#[template(tag = div)]
fn show_app(tile: TilePtr, #[signal] app: App) -> XElement {
    tag(
        class = style::APP_CONTENT,
        match app {
            #[cfg(feature = "terminal")]
            App::Terminal => div(move |t| crate::terminal::terminals(t)),

            #[cfg(feature = "text-editor")]
            App::TextEditor => crate::text_editor::ui::text_editor(tile.remote.clone()),

            #[cfg(feature = "converter")]
            App::Converter => crate::converter::ui::converter(tile.remote.clone()),

            #[cfg(feature = "port-forward")]
            App::PortForward => crate::portforward::ui::port_forward(tile.remote.clone()),
        },
    )
}
