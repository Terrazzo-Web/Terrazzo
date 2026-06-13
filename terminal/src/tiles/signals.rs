#![cfg(feature = "client")]

use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;

use terrazzo::envelope;
use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;

use super::api::Direction;
use super::api::Side;
use super::api::Tile as TileDto;
use super::api::Tiles as TilesDto;
use super::app::App;
use super::id::TileId;
use super::ui::RootTree;
use super::visitor::TilesTreeVisitor;
use super::visitor::UiStateVisitor;
use crate::frontend::menu::MenuState;
use crate::frontend::remotes::Remote;

pub enum Tiles {
    Tile(TilePtr),
    Array {
        id: TileId,
        direction: XSignal<Direction>,
        selected: XSignal<Option<TileId>>,
        nodes: Vec<Rc<Tiles>>,
    },
}

#[envelope]
pub struct Tile {
    pub id: TileId,
    pub app: XSignal<App>,
    pub remote: XSignal<Remote>,
    pub title: XSignal<XString>,
    pub menu: MenuState,
}

impl Tiles {
    pub fn update(&self, tiles: &TilesDto) -> Self {
        let mut signals = TileSignals::default();
        signals.visit_node(self);
        transform(&mut signals, tiles)
    }
}

fn transform(signals: &mut TileSignals, tile_tree_dto: &TilesDto) -> Tiles {
    match tile_tree_dto {
        TilesDto::Tile(TileDto {
            id,
            app,
            remote,
            title,
        }) => {
            let ui_tile = if let Some(ui_tile) = signals.tile_ids.remove(id) {
                ui_tile.app.set(*app);
                ui_tile.remote.set(remote.clone());
                ui_tile.title.set(title.clone());
                ui_tile
            } else {
                Tile {
                    id: *id,
                    app: XSignal::new("app", *app),
                    remote: XSignal::new("remote", remote.clone()),
                    title: XSignal::new("title", title.clone().into()),
                    menu: MenuState::default(),
                }
                .into()
            };
            Tiles::Tile(ui_tile)
        }
        TilesDto::Array {
            id,
            direction,
            selected,
            nodes,
        } => {
            let ui_direction = if let Some(ui_direction) = signals.directions.remove(id) {
                ui_direction.set(*direction);
                ui_direction
            } else {
                XSignal::new("direction", *direction)
            };
            let ui_selected = if let Some(ui_selected) = signals.selected.remove(id) {
                ui_selected.set(*selected);
                ui_selected
            } else {
                XSignal::new("selected-tile-tab", *selected)
            };
            let mut ui_nodes = Vec::with_capacity(nodes.len());
            for node in nodes {
                ui_nodes.push(transform(signals, node).into());
            }
            Tiles::Array {
                id: *id,
                direction: ui_direction,
                selected: ui_selected,
                nodes: ui_nodes,
            }
        }
    }
}

impl Default for Tiles {
    fn default() -> Self {
        let id = TileId::first_tile_id();
        Tile {
            id,
            app: XSignal::new("app", App::default()),
            remote: XSignal::new("remote", Remote::default()),
            title: XSignal::new("title", format!("New tile {id}").into()),
            menu: MenuState::default(),
        }
        .into()
    }
}

impl From<Tile> for Tiles {
    fn from(tile: Tile) -> Self {
        Self::Tile(tile.into())
    }
}

#[derive(Default)]
struct TileSignals {
    directions: HashMap<TileId, XSignal<Direction>>,
    selected: HashMap<TileId, XSignal<Option<TileId>>>,
    tile_ids: HashMap<TileId, TilePtr>,
}

impl<'l> TilesTreeVisitor<UiStateVisitor<'l>> for TileSignals {
    fn visit_tree(&mut self, id: TileId, direction: &XSignal<Direction>) {
        self.directions.insert(id, direction.clone());
    }
    fn visit_selected(&mut self, id: TileId, selected: &XSignal<Option<TileId>>) {
        self.selected.insert(id, selected.clone());
    }
    fn visit_tile(&mut self, tile: &TilePtr) {
        self.tile_ids.insert(tile.id, tile.clone());
    }
}

#[derive(Clone)]
pub struct TilesCmp<T>(T);

impl<T> TilesCmp<T> {
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: AsRef<Tiles>> PartialEq for TilesCmp<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Tiles::Tile(a), Tiles::Tile(b)) => a.id == b.id,
            (
                Tiles::Array {
                    id: a_id,
                    nodes: a_nodes,
                    ..
                },
                Tiles::Array {
                    id: b_id,
                    nodes: b_nodes,
                    ..
                },
            ) => {
                a_id == b_id
                    && a_nodes.len() == b_nodes.len()
                    && Iterator::zip(a_nodes.iter(), b_nodes.iter())
                        .all(|(a, b)| TilesCmp(a) == TilesCmp(b))
            }
            _ => false,
        }
    }
}

impl<T: AsRef<Tiles>> Eq for TilesCmp<T> {}

impl<T: AsRef<Tiles>> AsRef<Tiles> for TilesCmp<T> {
    fn as_ref(&self) -> &Tiles {
        self.0.as_ref()
    }
}

impl<T> Deref for TilesCmp<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::fmt::Debug for TilesCmp<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tiles(...)")
    }
}

impl Tile {
    pub fn split_horz(&self) -> impl Fn(MouseEvent) + 'static {
        self.split(Direction::Horizontal)
    }

    pub fn split_vert(&self) -> impl Fn(MouseEvent) + 'static {
        self.split(Direction::Vertical)
    }

    pub fn tabify(&self) -> impl Fn(MouseEvent) + 'static {
        self.split(Direction::Tabbed)
    }

    fn split(&self, direction: Direction) -> impl Fn(MouseEvent) + 'static {
        let tile_id = self.id;
        move |_| {
            spawn_local(async move {
                RootTree::update(super::api::add(direction, tile_id, Side::After).await)
            })
        }
    }

    pub fn close(&self) -> impl Fn(MouseEvent) + 'static {
        let tile_id = self.id;
        move |_| spawn_local(async move { RootTree::update(super::api::remove(tile_id).await) })
    }
}
