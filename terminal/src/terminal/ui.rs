use std::collections::HashSet;
use std::ops::ControlFlow;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::tabs::TabsDescriptor as _;
use terrazzo::widgets::tabs::TabsOptions;
use terrazzo::widgets::tabs::TabsState as _;
use terrazzo::widgets::tabs::tabs;
use wasm_bindgen::JsCast as _;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys::JsString;

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::info;
use self::diagnostics::warn;
use super::terminal_tab::TerminalTab;
use super::terminal_tabs::TerminalTabs;
use crate::api::client::terminal_api;
use crate::terminal_id::TerminalId;
use crate::tiles::app::App;
use crate::tiles::id::TileId;
use crate::tiles::signals::TilePtr;
use crate::tiles::ui::RootTree;

terrazzo_css::import_style!(pub(super) style, "terminal.scss");

#[derive(Clone)]
pub struct TerminalsState {
    pub tile: TilePtr,
    pub selected_tab: XSignal<TerminalId>,
    pub terminal_tabs: XSignal<TerminalTabs>,
}

pub fn terminals(template: XTemplate, tile: TilePtr) -> Consumers {
    let terminal_id = TerminalId::from("Terminal");
    let selected_tab = XSignal::new("selected-tab", terminal_id.clone());
    let terminal_tabs = XSignal::new("terminal-tabs", TerminalTabs::from(Ptr::new(vec![])));
    refresh_terminal_tabs(tile.id, selected_tab.clone(), terminal_tabs.clone());
    let state = TerminalsState {
        tile: tile.clone(),
        selected_tab: selected_tab.clone(),
        terminal_tabs: terminal_tabs.clone(),
    };
    render_terminals(template, state, terminal_tabs.clone()).append(selected_tab.add_subscriber(
        move |terminal_id| {
            let terminal_tabs = terminal_tabs.get_value_untracked();
            let Some(current) = terminal_tabs.lookup_tab(&terminal_id) else {
                return;
            };
            let client_address = &current.address.via;
            tile.remote.set(client_address.clone())
        },
    ))
}

#[autoclone]
#[html]
#[template]
pub fn render_terminals(state: TerminalsState, #[signal] terminal_tabs: TerminalTabs) -> XElement {
    info!("Render terminals: {terminal_tabs:?}");
    div(
        style = "height: 100%;",
        div(
            key = "terminals",
            class = style::TERMINALS,
            #[cfg(not(feature = "client-prod"))]
            class = "terminals",
            tabs(
                terminal_tabs.clone(),
                state.clone(),
                Ptr::new(TabsOptions {
                    tabs_class: Some(get_class_name("tabs", style::TABS).into()),
                    titles_class: Some(get_class_name("titles", style::TITLES).into()),
                    title_class: Some(get_class_name("title", style::TITLE).into()),
                    items_class: Some(get_class_name("items", style::ITEMS).into()),
                    item_class: Some(get_class_name("item", style::ITEM).into()),
                    selected_class: Some(get_class_name("selected", style::SELECTED).into()),
                    ..TabsOptions::default()
                }),
            ),
            dragover = move |ev: web_sys::DragEvent| {
                autoclone!(state);
                let is_from_other_tile = || {
                    let zone_id = state.zone_id()?;
                    let dt = ev.data_transfer()?;
                    let types = dt.types();
                    let mut is_terminal = false;
                    let mut is_current_tile = false;
                    for i in 0..types.length() {
                        let ty = types.get(i);
                        let Some(ty) = ty.dyn_ref::<JsString>() else {
                            continue;
                        };
                        is_terminal |= ty == TerminalsState::drag_key();
                        is_current_tile |= *ty == zone_id;
                    }
                    (is_terminal && !is_current_tile).then_some(())
                };

                if is_from_other_tile().is_some() {
                    ev.prevent_default();
                    ev.stop_propagation();
                }
            },
            drop = move |ev: web_sys::DragEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                let Some(data_transfer) = ev.data_transfer() else {
                    return;
                };
                let Ok(terminal_id) = data_transfer.get_data(TerminalsState::drag_key()) else {
                    return;
                };
                let terminal_id = TerminalId::from(terminal_id);
                spawn_local(async move {
                    autoclone!(state);
                    let terminal_defs = match terminal_api::list::list().await {
                        Ok(terminal_defs) => terminal_defs,
                        Err(error) => {
                            warn!("Failed to load terminal definitions: {error}");
                            return;
                        }
                    };
                    let Some(terminal_def) = terminal_defs
                        .into_iter()
                        .find(|def| def.address.id == terminal_id)
                    else {
                        warn!("Terminal '{terminal_id}' not found");
                        return;
                    };
                    if let Err(error) = super::api::set_tile_id(
                        terminal_def.address.via,
                        terminal_id.clone(),
                        state.tile.id,
                    )
                    .await
                    {
                        warn!("Failed to set terminal tile: {error}");
                        return;
                    }
                    state.selected_tab.force(terminal_id);
                    refresh_terminal_tabs(state.tile.id, state.selected_tab, state.terminal_tabs);
                });
            },
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

fn refresh_terminal_tabs(
    tile: TileId,
    selected_tab: XSignal<TerminalId>,
    terminal_tabs: XSignal<TerminalTabs>,
) {
    let refresh_terminal_tabs_task = async move {
        let terminal_defs = match terminal_api::list::list().await {
            Ok(terminal_defs) => terminal_defs,
            Err(error) => {
                warn!("Failed to load terminal definitions: {error}");
                return;
            }
        };
        let mut all_tile_ids: Option<HashSet<TileId>> = None;
        let _: ControlFlow<()> = RootTree::foreach(|t| {
            if t.app.get_value_untracked() != App::Terminal {
                return ControlFlow::Continue(());
            }
            if let Some(all_tile_ids) = &mut all_tile_ids {
                all_tile_ids.insert(t.id);
                ControlFlow::Continue(())
            } else {
                // This is the first Terminal tile
                if t.id != tile {
                    ControlFlow::Break(())
                } else {
                    all_tile_ids = Some([tile].into());
                    ControlFlow::Continue(())
                }
            }
        });
        let terminal_defs = terminal_defs
            .into_iter()
            .filter(|def| {
                def.tile == tile
                    || if let Some(all_tile_ids) = &all_tile_ids {
                        !all_tile_ids.contains(&def.tile)
                    } else {
                        false
                    }
            })
            .collect::<Vec<_>>();
        let batch = Batch::use_batch("refresh_terminal_tabs");
        if let Some(first_terminal) = terminal_defs.first() {
            let selected_tab_value = selected_tab.get_value_untracked();
            if !terminal_defs
                .iter()
                .any(|def| def.address.id == selected_tab_value)
            {
                selected_tab.force(first_terminal.address.id.clone());
            }
        }
        terminal_tabs.set(TerminalTabs::from(Ptr::new(
            terminal_defs
                .into_iter()
                .map(|def| TerminalTab::of(def, &selected_tab))
                .collect::<Vec<_>>(),
        )));
        drop(batch);
    };
    spawn_local(refresh_terminal_tabs_task.in_current_span());
}

impl TerminalsState {
    /// Callback on end-of-stream to drop the terminal tab from the UI after the process is closed in the backend.
    pub fn on_eos(&self, terminal_id: &TerminalId) {
        debug!("Closing the terminal tab");
        let _batch = Batch::use_batch("close-tab");
        let TerminalsState {
            tile: _,
            selected_tab,
            terminal_tabs,
        } = self;
        if selected_tab.get_value_untracked() == *terminal_id
            && let Some(next_selected_tab) = next_selected_tab(terminal_tabs, terminal_id)
        {
            selected_tab.set(next_selected_tab);
        }
        terminal_tabs
            .update_ne(|terminal_tabs| Some(terminal_tabs.clone().remove_tab(terminal_id)));
        if let Some(last_dispatcher) = terminal_api::stream::drop_dispatcher(terminal_id) {
            spawn_local(terminal_api::stream::close_pipe(last_dispatcher).in_current_span());
        }
    }
}

fn next_selected_tab(
    terminal_tabs: &XSignal<TerminalTabs>,
    closed_terminal_id: &TerminalId,
) -> Option<TerminalId> {
    let terminal_tabs = terminal_tabs.get_value_untracked();
    let mut terminal_tabs = terminal_tabs.tab_descriptors().iter();
    let mut prev = None;
    while let Some(next) = terminal_tabs.next() {
        if next.address.id == *closed_terminal_id {
            if let Some(tab) = terminal_tabs.next() {
                return Some(tab.address.id.clone());
            } else {
                return prev;
            }
        }
        prev = Some(next.address.id.clone());
    }
    return prev;
}
