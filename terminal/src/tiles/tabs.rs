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

use self::diagnostics::warn;
use super::api::Direction;
use super::signals::FloatingTile;
use super::signals::Tiles;
use super::signals::node_id;
use super::ui::RcSlice;
use super::ui::RootTree;
use crate::assets::icons;
use crate::frontend::mousemove::MousemoveManager;
use crate::tiles::api::set_tab_title;
use crate::tiles::id::TileId;

terrazzo_css::import_style!(style, "tabs.scss");

#[derive(Clone)]
pub struct TileTabs {
    tabs: Rc<Vec<TileTab>>,
}

impl TileTabs {
    pub fn new(nodes: &[Rc<Tiles>], selected: &XSignal<Option<TileId>>) -> Self {
        let tabs = nodes
            .iter()
            .map(|node| TileTab::new(node.clone(), selected))
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
    pub fn new(array_id: TileId, selected: XSignal<Option<TileId>>, nodes: &[Rc<Tiles>]) -> Self {
        if selected.get_value_untracked().is_none() {
            selected.set(nodes.first().map(|node| child_id(node)));
        }
        let sync_selection = selected.add_subscriber(move |selected| {
            spawn_local(async move {
                RootTree::update(super::api::select_child(array_id, selected).await);
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
}

impl TileTab {
    fn new(node: Rc<Tiles>, selected: &XSignal<Option<TileId>>) -> Self {
        let id = child_id(&node);
        Self {
            id,
            node,
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
) -> XElement {
    let descriptor = TileTabs::new(nodes, &selected);
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
            let floating_id = node_id(&floating.tile);
            let z_indices = z_indices.clone();
            div(
                key = format!("floating-{floating_id}"),
                class = style::FLOATING_TILE,
                #[cfg(not(feature = "client-prod"))]
                class = "floating-tile",
                style::left %= percent(floating.x1.clone()),
                style::top %= percent(floating.y1.clone()),
                style::width %= span_percent(floating.x1.clone(), floating.x2.clone()),
                style::height %= span_percent(floating.y1.clone(), floating.y2.clone()),
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
                ),
            )
        })
        .collect::<Vec<_>>()..)
}

#[template(wrap = true)]
fn percent(#[signal] value: i32) -> XAttributeValue {
    format!("{value}%")
}

#[template(wrap = true)]
fn span_percent(#[signal] start: i32, #[signal] end: i32) -> XAttributeValue {
    format!("{}%", end - start)
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

fn child_id(node: &Tiles) -> TileId {
    node_id(node)
}
