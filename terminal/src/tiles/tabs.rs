#![cfg(feature = "client")]

use std::rc::Rc;

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

use super::api::Direction;
use super::signals::Tiles;
use super::ui::RcSlice;
use super::ui::RootTree;
use crate::assets::icons;
use crate::frontend::mousemove::MousemoveManager;
use crate::tiles::id::TileId;

terrazzo_css::import_style!(style, "tabs.scss");

#[derive(Clone)]
pub struct TileTabs {
    tabs: Rc<Vec<TileTab>>,
}

impl TileTabs {
    pub fn new(nodes: &[Rc<Tiles>], selected: &XSignal<Option<TileId>>) -> Self {
        Self {
            tabs: Rc::new(
                nodes
                    .iter()
                    .cloned()
                    .map(|node| TileTab::new(node, selected))
                    .collect(),
            ),
        }
    }
}

impl TabsDescriptor for TileTabs {
    type TabDescriptor = TileTab;
    type State = TileTabsState;

    fn tab_descriptors(&self) -> &[Self::TabDescriptor] {
        &self.tabs
    }

    #[html]
    fn after_titles(&self, state: &TileTabsState) -> impl IntoIterator<Item = impl Into<XNode>> {
        let state = state.clone();
        [div(
            class = style::ADD_TILE_TAB,
            #[cfg(not(feature = "client-prod"))]
            class = "add-tile-tab",
            img(src = icons::add_tab()),
            click = move |_| {
                let state = state.clone();
                spawn_local(async move {
                    let after_child = state.selected.get_value_untracked();
                    match super::api::add_tab(state.array_id, after_child).await {
                        Ok((tree, new_id)) => {
                            state.selected.set(Some(new_id));
                            RootTree::update(Ok::<_, String>(tree));
                        }
                        Err(error) => {
                            diagnostics::warn!("Failed to add tile tab: {error}");
                        }
                    }
                });
            },
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
        let state = Self {
            array_id,
            selected,
            registrations: Rc::new(sync_selection),
        };
        state
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
                    diagnostics::warn!("Invalid tile tab id: {moved_tab_key}");
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
            Tiles::Tile(tile) => tile_title(tile.clone(), state.selected.clone()),
            Tiles::Array { direction, .. } => {
                let title = format!("{:?}", direction.get_value_untracked());
                span("{title}")
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

    fn selected(&self, state: &TileTabsState) -> XSignal<bool> {
        let _ = state;
        self.selected.clone()
    }
}

#[html]
pub fn show_tabbed_tiles(
    array_id: TileId,
    selected: XSignal<Option<TileId>>,
    nodes: &[Rc<Tiles>],
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
                tabs_class: Some(get_class_name("tile-tabs", style::TILE_TABS).into()),
                titles_class: Some(
                    get_class_name("tile-tab-titles", style::TILE_TAB_TITLES).into(),
                ),
                title_class: Some(get_class_name("tile-tab-title", style::TILE_TAB_TITLE).into()),
                items_class: Some(get_class_name("tile-tab-items", style::TILE_TAB_ITEMS).into()),
                item_class: Some(get_class_name("tile-tab-item", style::TILE_TAB_ITEM).into()),
                selected_class: Some(get_class_name("selected", style::SELECTED).into()),
                ..TabsOptions::default()
            }),
        ),
    )
}

fn get_class_name(name: &'static str, class: &'static str) -> impl Into<XString> {
    #[cfg(feature = "client-prod")]
    {
        let _ = name;
        return class;
    }

    #[cfg(not(feature = "client-prod"))]
    return format!("{name} {class}");
}

#[html]
fn tile_title(tile: super::signals::TilePtr, selected: XSignal<Option<TileId>>) -> XElement {
    let tile_id = tile.id;
    let app = tile.app.clone();
    let app2 = tile.app.clone();
    let title = tile.title.clone().derive(
        "resolve-tile-title",
        move |title| {
            title
                .clone()
                .unwrap_or_else(|| app.get_value_untracked().to_string().into())
        },
        if_change(move |_, title: &XString| {
            let default_title = app2.get_value_untracked().to_string();
            if title.is_empty() || title.to_string() == default_title {
                Some(None)
            } else {
                Some(Some(title.clone()))
            }
        }),
    );
    let editing = XSignal::new("editing-tile-title", false);
    let is_editable = selected.view("tile-title-editable", move |selected| {
        *selected == Some(tile_id)
    });
    span(move |template| {
        let title = title.clone();
        let title2 = title.clone();
        editable(
            template,
            title.clone(),
            is_editable.clone(),
            editing.clone(),
            move || {
                let title = title2.clone();
                [span(move |template| print_title(template, title.clone()))]
            },
        )
    })
}

#[html]
#[template]
fn print_title(#[signal] title: XString) -> XElement {
    span("{title}")
}

fn child_id(node: &Tiles) -> TileId {
    match node {
        Tiles::Tile(tile) => tile.id,
        Tiles::Array { id, .. } => *id,
    }
}
