use std::cell::Cell;
use std::rc::Rc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::editable::editable;
use terrazzo::widgets::tabs::TabDescriptor;
use terrazzo::widgets::tabs::TabsDescriptor;
use terrazzo::widgets::tabs::TabsOptions;
use terrazzo::widgets::tabs::TabsState;
use terrazzo::widgets::tabs::tabs;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;

use self::diagnostics::warn;
use super::api::Direction;
use super::signals::FloatingTile;
use super::signals::Tiles;
use super::ui::RcSlice;
use super::ui::RootTree;
use crate::assets::icons;
use crate::frontend::menu::DragHandle;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::tiles::api::set_tab_title;
use crate::tiles::id::TileId;

terrazzo_css::import_style!(style, "tabs.scss");

#[derive(Clone)]
pub struct TileTabs {
    tabs: Rc<Vec<TileTab>>,
}

impl TileTabs {
    pub fn new(
        nodes: &[Rc<Tiles>],
        selected: &XSignal<Option<TileId>>,
        drag_handle: Option<DragHandle>,
    ) -> Self {
        let tabs = nodes
            .iter()
            .map(|node| TileTab::new(node.clone(), selected, drag_handle.clone()))
            .collect();
        Self {
            tabs: Rc::new(tabs),
        }
    }
}

impl TabsDescriptor for TileTabs {
    type TabDescriptor = TileTab;
    type State = TileTabsState;

    fn tab_descriptors(&self) -> &[Self::TabDescriptor] {
        &self.tabs
    }

    #[autoclone]
    #[html]
    fn after_titles(&self, state: &TileTabsState) -> impl IntoIterator<Item = impl Into<XNode>> {
        [div(
            key = "add-tile-icon",
            class = style::ADD_TILE_TAB,
            #[cfg(not(feature = "client-prod"))]
            class = "add-tile-tab",
            div(
                img(src = icons::add_tab()),
                click = move |_| {
                    autoclone!(state);
                    spawn_local(async move {
                        autoclone!(state);
                        let after_child = state.selected.get_value_untracked();
                        match super::api::add_tab(state.array_id, after_child).await {
                            Ok((tree, new_id)) => {
                                state.selected.set(Some(new_id));
                                RootTree::update(Ok::<_, String>(tree));
                            }
                            Err(error) => warn!("Failed to add tile tab: {error}"),
                        }
                    });
                },
            ),
        )]
    }
}

#[derive(Clone)]
pub struct TileTabsState {
    array_id: TileId,
    selected: XSignal<Option<TileId>>,
    #[expect(dead_code)]
    registrations: Rc<Consumers>,
}

impl TileTabsState {
    #[autoclone]
    pub fn new(array_id: TileId, selected: XSignal<Option<TileId>>, nodes: &[Rc<Tiles>]) -> Self {
        if selected.get_value_untracked().is_none() {
            selected.set(nodes.first().map(|node| node.id()));
        }
        let pending = Rc::new(Cell::new(None::<Option<TileId>>));
        let syncing = Rc::new(Cell::new(false));
        let sync_selection = selected.add_subscriber(move |selected| {
            pending.set(Some(selected));
            if syncing.replace(true) {
                return;
            }
            spawn_local(async move {
                autoclone!(pending, syncing);
                while let Some(selected) = pending.take() {
                    if let Err(error) = super::api::select_child(array_id, selected).await {
                        warn!("Failed to select tile tab: {error}");
                    }
                }
                syncing.set(false);
            })
        });
        Self {
            array_id,
            selected,
            registrations: Rc::new(sync_selection),
        }
    }
}

impl TabsState for TileTabsState {
    type TabDescriptor = TileTab;

    fn move_tab(&self, after_tab: Option<TileTab>, moved_tab_key: String) {
        let array_id = self.array_id;
        let selected = self.selected.clone();
        spawn_local(async move {
            let moved_child = match moved_tab_key
                .parse::<i64>()
                .ok()
                .and_then(|moved_child| TileId::try_from(moved_child).ok())
            {
                Some(moved_child) => moved_child,
                None => {
                    warn!("Invalid tile tab id: {moved_tab_key}");
                    return;
                }
            };
            let after_child = after_tab.map(|tab| tab.id);
            selected.set(Some(moved_child));
            RootTree::update(super::api::move_child(array_id, after_child, moved_child).await);
        });
    }

    fn drag_key() -> &'static str {
        "tile_tab_id"
    }

    fn zone_id(&self) -> Option<String> {
        Some(format!("tile_tab_array:{}", self.array_id))
    }
}

#[derive(Clone)]
pub struct TileTab {
    id: TileId,
    node: Rc<Tiles>,
    selected: XSignal<bool>,
    drag_handle: Option<DragHandle>,
}

impl TileTab {
    fn new(
        node: Rc<Tiles>,
        selected: &XSignal<Option<TileId>>,
        drag_handle: Option<DragHandle>,
    ) -> Self {
        let id = node.id();
        Self {
            id,
            node,
            drag_handle,
            selected: selected.derive(
                "selected-tile-tab",
                move |selected| *selected == Some(id),
                if_change(move |_, selected: &bool| selected.then_some(Some(id))),
            ),
        }
    }
}

impl TabDescriptor for TileTab {
    type State = TileTabsState;

    fn key(&self) -> XString {
        self.id.to_string().into()
    }

    #[html]
    fn title(&self, state: &TileTabsState) -> impl Into<XNode> {
        match &*self.node {
            Tiles::Tile(tile) => {
                tile_title(tile.id, tile.title.clone(), state.selected.clone(), false)
            }
            Tiles::Array { title, .. } => {
                tile_title(self.id, title.clone(), state.selected.clone(), true)
            }
        }
    }

    #[html]
    fn item(&self, _state: &TileTabsState) -> impl Into<XNode> {
        super::ui::show_tiles_rec(
            &self.node,
            1,
            MousemoveManager::new(),
            XSignal::new("tile-tab-parent-direction", Direction::Horizontal),
            RcSlice::new(Rc::default(), 0..0),
            self.drag_handle.clone(),
        )
    }

    fn selected(&self, _state: &TileTabsState) -> XSignal<bool> {
        self.selected.clone()
    }
}

#[html]
pub fn show_tabbed_tiles(
    array_id: TileId,
    selected: XSignal<Option<TileId>>,
    nodes: &[Rc<Tiles>],
    floating_nodes: &[Rc<FloatingTile>],
    drag_handle: Option<DragHandle>,
) -> XElement {
    let descriptor = TileTabs::new(nodes, &selected, drag_handle);
    let state = TileTabsState::new(array_id, selected, nodes);
    div(
        class = style::TABBED_TILE,
        #[cfg(not(feature = "client-prod"))]
        class = "tabbed-tile",
        tabs(
            descriptor,
            state,
            Ptr::new(TabsOptions {
                tabs_class: Some(get_class_name("tile-tabs", style::TILE_TABS)),
                titles_class: Some(get_class_name("tile-tab-titles", style::TILE_TAB_TITLES)),
                title_class: Some(get_class_name("tile-tab-title", style::TILE_TAB_TITLE)),
                items_class: Some(get_class_name("tile-tab-items", style::TILE_TAB_ITEMS)),
                item_class: Some(get_class_name("tile-tab-item", style::TILE_TAB_ITEM)),
                selected_class: Some(get_class_name("selected", style::SELECTED)),
                ..TabsOptions::default()
            }),
        ),
        show_floating_tiles(array_id, floating_nodes),
    )
}

#[html]
fn show_floating_tiles(array_id: TileId, floating_nodes: &[Rc<FloatingTile>]) -> XElement {
    let z_indices: Rc<[XSignal<i32>]> = floating_nodes
        .iter()
        .map(|floating| floating.z_index.clone())
        .collect();
    div(floating_nodes
        .iter()
        .map(|floating| {
            let floating = floating.clone();
            let floating_id = floating.tile.id();
            let z_indices = z_indices.clone();
            let x = floating.x.clone();
            let y = floating.y.clone();
            let persist_x = x.clone();
            let persist_y = y.clone();
            let drag_manager = MousemoveManager::new2(move || {
                let x = persist_x.get_value_untracked();
                let y = persist_y.get_value_untracked();
                spawn_local(async move {
                    RootTree::update(
                        super::api::set_floating_position(array_id, floating_id, x, y).await,
                    )
                });
            });
            let initial_x = x.get_value_untracked();
            let initial_y = y.get_value_untracked();
            let update_position = drag_manager.delta.add_subscriber(move |delta| {
                if let Some(Position {
                    x: delta_x,
                    y: delta_y,
                }) = delta
                {
                    let _batch = Batch::use_batch("move-floating-tile");
                    x.set(0.max(initial_x + delta_x));
                    y.set(0.max(initial_y + delta_y));
                }
            });
            let drag_handle: DragHandle = Rc::new(drag_manager.mousedown());
            let width = floating.width.clone();
            let height = floating.height.clone();
            let persist_width = width.clone();
            let persist_height = height.clone();
            let resize_manager = MousemoveManager::new2(move || {
                let width = persist_width.get_value_untracked();
                let height = persist_height.get_value_untracked();
                spawn_local(async move {
                    RootTree::update(
                        super::api::set_floating_size(array_id, floating_id, width, height).await,
                    )
                });
            });
            let initial_width = width.get_value_untracked();
            let initial_height = height.get_value_untracked();
            let update_size = resize_manager.delta.add_subscriber(move |delta| {
                if let Some(Position {
                    x: delta_x,
                    y: delta_y,
                }) = delta
                {
                    let _batch = Batch::use_batch("resize-floating-tile");
                    width.set(100.max(initial_width + delta_x));
                    height.set(100.max(initial_height + delta_y));
                }
            });
            let resize_handle = resize_manager.mousedown();
            div(
                key = format!("floating-{floating_id}"),
                before_render = move |_| {
                    let _ = &update_position;
                    let _ = &update_size;
                },
                class = style::FLOATING_TILE,
                #[cfg(not(feature = "client-prod"))]
                class = "floating-tile",
                style::left %= pixels(floating.x.clone()),
                style::top %= pixels(floating.y.clone()),
                style::width %= pixels(floating.width.clone()),
                style::height %= pixels(floating.height.clone()),
                style::z_index %= integer(floating.z_index.clone()),
                mousedown = move |_| {
                    let next = z_indices
                        .iter()
                        .map(XSignal::get_value_untracked)
                        .max()
                        .unwrap_or_default()
                        + 1;
                    floating.z_index.set(next);
                    spawn_local(async move {
                        RootTree::update(super::api::raise_floating(array_id, floating_id).await)
                    });
                },
                super::ui::show_tiles_rec(
                    &floating.tile,
                    1,
                    MousemoveManager::new(),
                    XSignal::new("floating-tile-parent-direction", Direction::Horizontal),
                    RcSlice::new(Rc::default(), 0..0),
                    Some(drag_handle),
                ),
                img(
                    class = style::RESIZE_HANDLE,
                    #[cfg(not(feature = "client-prod"))]
                    class = "floating-resize-handle",
                    src = icons::drag_handle_corner(),
                    mousedown = move |ev: MouseEvent| {
                        ev.prevent_default();
                        resize_handle(ev);
                    },
                ),
            )
        })
        .collect::<Vec<_>>()..)
}

#[template(wrap = true)]
fn pixels(#[signal] value: i32) -> XAttributeValue {
    format!("{value}px")
}

#[template(wrap = true)]
fn integer(#[signal] value: i32) -> XAttributeValue {
    value.to_string()
}

fn get_class_name(name: &'static str, class: &'static str) -> XString {
    #[cfg(feature = "client-prod")]
    {
        let _ = name;
        return class.into();
    }

    #[cfg(not(feature = "client-prod"))]
    return format!("{name} {class}").into();
}

#[autoclone]
#[html]
fn tile_title(
    tile_id: TileId,
    title: XSignal<XString>,
    selected: XSignal<Option<TileId>>,
    update_title: bool,
) -> XElement {
    let editing = XSignal::new("editing-tile-title", false);
    let is_editable = selected.view("tile-title-editable", move |selected| {
        *selected == Some(tile_id)
    });
    let title_link = span(move |template| {
        autoclone!(title);
        editable(
            template,
            title.clone(),
            is_editable.clone(),
            editing.clone(),
            move || {
                autoclone!(title);
                [terrazzo::widgets::link::link(
                    move |_ev| {},
                    move || {
                        autoclone!(title);
                        [print_title(
                            tile_id,
                            title.clone(),
                            title.clone(),
                            update_title,
                        )]
                    },
                )]
            },
        )
    });
    let close_button = img(
        key = "close-icon",
        class = style::CLOSE_ICON,
        #[cfg(not(feature = "client-prod"))]
        class = "close-icon",
        src = icons::close_tab(),
        click = move |ev: web_sys::MouseEvent| {
            ev.stop_propagation();
            let close_task = async move { RootTree::update(super::api::remove(tile_id).await) };
            spawn_local(close_task);
        },
    );

    div([title_link, close_button]..)
}

#[html]
#[template(tag = span)]
fn print_title(
    array_id: TileId,
    title_signal: XSignal<XString>,
    #[signal] mut title: XString,
    update_title: bool,
) -> XElement {
    let update_title = if update_title {
        title_signal.add_subscriber(move |title: XString| {
            spawn_local(async move {
                RootTree::update(set_tab_title(array_id, title.to_string()).await)
            })
        })
    } else {
        Default::default()
    };
    let title = if title.is_empty() {
        XString::from("UNNAMED")
    } else {
        title
    };
    span(
        after_render = move |_| {
            let _ = &update_title;
        },
        "{title}",
        class = style::TITLE_SPAN,
    )
}
