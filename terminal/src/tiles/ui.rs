#![cfg(feature = "client")]

use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;

use terrazzo::html;
use terrazzo::prelude::diagnostics::warn;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use super::api::Direction;
use super::api::Tiles as TilesDto;
use super::api::set_app;
use super::api::set_remote;
use super::app::App;
use super::signals::TilePtr;
use super::signals::Tiles;
use super::signals::TilesCmp;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::frontend::resize_bar::resize_bar;

terrazzo_css::import_style!(style, "ui.scss");

pub struct RootTree(XSignal<TilesCmp<Rc<Tiles>>>);

pub static ROOT_TREE: LazyLock<RootTree> = LazyLock::new(|| {
    RootTree(XSignal::new(
        "tiles",
        TilesCmp::new(Rc::new(Tiles::default())),
    ))
});

unsafe impl Sync for RootTree {}
unsafe impl Send for RootTree {}

impl RootTree {
    pub fn update(new: Result<Arc<TilesDto>, impl std::fmt::Display + std::fmt::Debug + 'static>) {
        match new {
            Ok(new) => {
                let _batch = Batch::use_batch("Update tiles");
                ROOT_TREE.update_ne(|old| Some(TilesCmp::new(Rc::new(old.update(&new)))));
            }
            Err(error) => warn!("Failed to update tiles: {error}"),
        }
    }
}

impl std::ops::Deref for RootTree {
    type Target = XSignal<TilesCmp<Rc<Tiles>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn show_tiles() -> XElement {
    spawn_local(async move { RootTree::update(super::api::get().await) });
    show_tiles_tree(ROOT_TREE.clone())
}

#[template(tag = div)]
fn show_tiles_tree(#[signal] tiles: TilesCmp<Rc<Tiles>>) -> XElement {
    show_tiles_rec(
        &tiles,
        1,
        MousemoveManager::new(),
        XSignal::new("direction0", Direction::Horizontal),
    )
}

#[html]
fn show_tiles_rec(
    tiles: &Tiles,
    siblings: usize,
    parent_resize_manager: MousemoveManager,
    parent_direction: XSignal<Direction>,
) -> XElement {
    match tiles {
        Tiles::Tile(tile) => {
            let tile_id = tile.id;
            let update_app = tile.app.add_subscriber(move |app| {
                spawn_local(async move { RootTree::update(set_app(tile_id, app).await) })
            });
            let update_remote = tile.remote.add_subscriber(move |remote| {
                spawn_local(async move { RootTree::update(set_remote(tile_id, remote).await) })
            });
            div(
                key = tile.id,
                before_render = move |_| {
                    let _ = &update_app;
                    let _ = &update_remote;
                },
                key = tile.id.to_string(),
                class = style::APP_TILE,
                #[cfg(not(feature = "client-prod"))]
                class = "app-tile",
                style::flex %= size(
                    parent_resize_manager.delta.clone(),
                    siblings,
                    parent_direction,
                ),
                show_app(tile.clone(), tile.app.clone()),
                #[cfg(feature = "logs-panel")]
                crate::logs::panel(tile.clone()),
            )
        }
        Tiles::Array {
            id: _,
            direction,
            nodes,
        } => {
            let count = nodes.len();
            let nodes = nodes.iter().enumerate().flat_map(|(i, node)| {
                let resize_manager = MousemoveManager::new();
                let node = show_tiles_rec(
                    node.as_ref(),
                    nodes.len(),
                    resize_manager.clone(),
                    direction.clone(),
                );
                if i == count - 1 {
                    return vec![node];
                }
                let sep = resize_bar(resize_manager);
                vec![node, sep]
            });
            div(
                class %= direction_class(direction.clone()),
                #[cfg(not(feature = "client-prod"))]
                class = "tile-array",
                style::flex %= size(
                    parent_resize_manager.delta.clone(),
                    siblings,
                    parent_direction,
                ),
                nodes..,
            )
        }
    }
}

#[template(wrap = true)]
pub fn direction_class(#[signal] direction: Direction) -> XAttributeValue {
    match direction {
        Direction::Horizontal => style::HORIZONTAL_TILE,
        Direction::Vertical => style::VERTICAL_TILE,
    }
}
#[html]
#[template(tag = div)]
fn show_app(tile: TilePtr, #[signal] app: App) -> XElement {
    tag(
        class = style::APP_CONTENT,
        match app {
            App::Default => div(crate::frontend::menu::menu(tile.clone())),

            #[cfg(feature = "terminal")]
            App::Terminal => div(move |t| crate::terminal::terminals(t, tile.clone())),

            #[cfg(feature = "text-editor")]
            App::TextEditor => crate::text_editor::ui::text_editor(tile),

            #[cfg(feature = "converter")]
            App::Converter => crate::converter::ui::converter(tile),

            #[cfg(feature = "port-forward")]
            App::PortForward => crate::portforward::ui::port_forward(tile),
        },
    )
}

#[template(wrap = true)]
fn size(
    #[signal] mut position: Option<Position>,
    siblings: usize,
    #[signal] direction: Direction,
) -> XAttributeValue {
    let base = 100 / siblings;
    let px = position.map(|p| p.get(direction)).unwrap_or_default();
    format!("0 0 calc({base}% + {px}px)")
}
