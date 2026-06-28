#![cfg(feature = "client")]

use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::time::Duration;

use terrazzo::envelope;
use terrazzo::prelude::Batch;
use terrazzo::prelude::MutableSignal;
use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;
use terrazzo::widgets::cancellable::Cancellable;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;

use super::api::Direction;
use super::api::FloatingTile as FloatingTileDto;
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
        title: XSignal<XString>,
        selected: XSignal<Option<TileId>>,
        nodes: Vec<Rc<Tiles>>,
        floating_nodes: Vec<Rc<FloatingTile>>,
    },
}

pub struct FloatingTile {
    pub x1: XSignal<i32>,
    pub y1: XSignal<i32>,
    pub x2: XSignal<i32>,
    pub y2: XSignal<i32>,
    pub z_index: XSignal<i32>,
    pub tile: Tiles,
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
            title,
            selected,
            nodes,
            floating_nodes,
        } => {
            let ui_direction = if let Some(ui_direction) = signals.directions.remove(id) {
                ui_direction.set(*direction);
                ui_direction
            } else {
                XSignal::new("direction", *direction)
            };
            let ui_title = if let Some(ui_title) = signals.titles.remove(id) {
                ui_title.set(XString::from(title.clone()));
                ui_title
            } else {
                XSignal::new("title", title.clone().into())
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
            let ui_floating_nodes = floating_nodes
                .iter()
                .map(|floating| transform_floating(signals, floating))
                .map(Rc::new)
                .collect();
            Tiles::Array {
                id: *id,
                direction: ui_direction,
                title: ui_title,
                selected: ui_selected,
                nodes: ui_nodes,
                floating_nodes: ui_floating_nodes,
            }
        }
    }
}

fn transform_floating(signals: &mut TileSignals, floating: &FloatingTileDto) -> FloatingTile {
    let id = dto_node_id(&floating.tile);
    let old = signals.floating.remove(&id);
    FloatingTile {
        x1: reuse_signal("floating-x1", floating.x1, old.as_ref().map(|old| &old.x1)),
        y1: reuse_signal("floating-y1", floating.y1, old.as_ref().map(|old| &old.y1)),
        x2: reuse_signal("floating-x2", floating.x2, old.as_ref().map(|old| &old.x2)),
        y2: reuse_signal("floating-y2", floating.y2, old.as_ref().map(|old| &old.y2)),
        z_index: reuse_signal(
            "floating-z-index",
            floating.z_index,
            old.as_ref().map(|old| &old.z_index),
        ),
        tile: transform(signals, &floating.tile),
    }
}

fn reuse_signal<T: Clone + std::fmt::Debug + Eq + 'static>(
    name: &'static str,
    value: T,
    old: Option<&XSignal<T>>,
) -> XSignal<T> {
    if let Some(old) = old {
        old.set(value);
        old.clone()
    } else {
        XSignal::new(name, value)
    }
}

fn dto_node_id(node: &TilesDto) -> TileId {
    match node {
        TilesDto::Tile(tile) => tile.id,
        TilesDto::Array { id, .. } => *id,
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
    titles: HashMap<TileId, XSignal<XString>>,
    selected: HashMap<TileId, XSignal<Option<TileId>>>,
    tile_ids: HashMap<TileId, TilePtr>,
    floating: HashMap<TileId, Rc<FloatingTile>>,
}

impl<'l> TilesTreeVisitor<UiStateVisitor<'l>> for TileSignals {
    fn visit_tree(&mut self, id: TileId, direction: &XSignal<Direction>) {
        self.directions.insert(id, direction.clone());
    }
    fn visit_selected(&mut self, id: TileId, selected: &XSignal<Option<TileId>>) {
        self.selected.insert(id, selected.clone());
    }
    fn visit_title(&mut self, id: TileId, title: &XSignal<XString>) {
        self.titles.insert(id, title.clone());
    }
    fn visit_tile(&mut self, tile: &TilePtr) {
        self.tile_ids.insert(tile.id, tile.clone());
    }
    fn visit_floating(&mut self, floating: &Rc<FloatingTile>) {
        self.floating
            .insert(node_id(&floating.tile), floating.clone());
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
                    floating_nodes: a_floating_nodes,
                    ..
                },
                Tiles::Array {
                    id: b_id,
                    nodes: b_nodes,
                    floating_nodes: b_floating_nodes,
                    ..
                },
            ) => {
                a_id == b_id
                    && a_nodes.len() == b_nodes.len()
                    && Iterator::zip(a_nodes.iter(), b_nodes.iter())
                        .all(|(a, b)| TilesCmp(a) == TilesCmp(b))
                    && a_floating_nodes.len() == b_floating_nodes.len()
                    && Iterator::zip(a_floating_nodes.iter(), b_floating_nodes.iter()).all(
                        |(a, b)| {
                            node_id(&a.tile) == node_id(&b.tile)
                                && a.x1.get_value_untracked() == b.x1.get_value_untracked()
                                && a.y1.get_value_untracked() == b.y1.get_value_untracked()
                                && a.x2.get_value_untracked() == b.x2.get_value_untracked()
                                && a.y2.get_value_untracked() == b.y2.get_value_untracked()
                                && a.z_index.get_value_untracked()
                                    == b.z_index.get_value_untracked()
                        },
                    )
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
    pub fn split_horz(
        &self,
        show_menu_mut: MutableSignal<bool>,
        hide_menu: Cancellable<Duration>,
    ) -> impl Fn(MouseEvent) + 'static {
        self.split(show_menu_mut, hide_menu, Direction::Horizontal)
    }

    pub fn split_vert(
        &self,
        show_menu_mut: MutableSignal<bool>,
        hide_menu: Cancellable<Duration>,
    ) -> impl Fn(MouseEvent) + 'static {
        self.split(show_menu_mut, hide_menu, Direction::Vertical)
    }

    pub fn tabify(
        &self,
        show_menu_mut: MutableSignal<bool>,
        hide_menu: Cancellable<Duration>,
    ) -> impl Fn(MouseEvent) + 'static {
        self.split(show_menu_mut, hide_menu, Direction::Tabbed)
    }

    pub fn float(
        &self,
        show_menu_mut: MutableSignal<bool>,
        hide_menu: Cancellable<Duration>,
    ) -> impl Fn(MouseEvent) + 'static {
        let tile_id = self.id;
        move |_| {
            let batch = Batch::use_batch("float-tile");
            hide_menu.cancel();
            show_menu_mut.set(false);
            drop(batch);
            spawn_local(async move { RootTree::update(super::api::float(tile_id).await) })
        }
    }

    fn split(
        &self,
        show_menu_mut: MutableSignal<bool>,
        hide_menu: Cancellable<Duration>,
        direction: Direction,
    ) -> impl Fn(MouseEvent) + 'static {
        let tile_id = self.id;
        move |_| {
            let batch = Batch::use_batch("select-app");
            hide_menu.cancel();
            show_menu_mut.set(false);
            drop(batch);
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

pub fn node_id(node: &Tiles) -> TileId {
    match node {
        Tiles::Tile(tile) => tile.id,
        Tiles::Array { id, .. } => *id,
    }
}
