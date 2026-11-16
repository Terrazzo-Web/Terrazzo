//! Temporary DOM relocation. The saved tile tree and the live app instance stay intact.
use std::cell::RefCell;

use terrazzo::prelude::OrElseLog as _;
use terrazzo::widgets::resize_event::ResizeEvent;
use wasm_bindgen::JsCast as _;
use web_sys::Element;

thread_local! {
    static MAXIMIZED: RefCell<Option<Maximized>> = const { RefCell::new(None) };
}

pub(super) struct Session;

impl Drop for Session {
    fn drop(&mut self) {
        MAXIMIZED.with_borrow_mut(|active| *active = None);
    }
}

struct Maximized {
    tile: Element,
    slot: Element,
    host: Element,
}

pub(super) fn toggle(tile: Element) {
    MAXIMIZED.with_borrow_mut(|active| {
        let previous = active.take();
        let restoring = previous.as_ref().is_some_and(|old| old.tile == tile);
        drop(previous);
        if !restoring {
            let document = tile.owner_document().or_throw("tile document");
            // A shallow clone preserves flex sizing and renderer keys, but creates no app.
            let slot: Element = tile
                .clone_node_with_deep(false)
                .or_throw("clone tile slot")
                .unchecked_into();
            slot.set_class_name(super::ui::style::APP_TILE);
            let host = document.create_element("div").or_throw("maximize host");
            host.set_class_name(super::ui::style::MAXIMIZED_TILE);
            #[cfg(not(feature = "client-prod"))]
            host.set_class_name(&format!(
                "{} maximized-tile",
                super::ui::style::MAXIMIZED_TILE
            ));
            document
                .body()
                .or_throw("body")
                .append_child(&host)
                .or_throw("attach maximize host");
            *active = Some(Maximized { tile, slot, host });
            active.as_ref().unwrap().lift();
        }
    });
    ResizeEvent::signal().force(());
}

/// Return the real node to the renderer before it reconciles the saved layout.
pub(super) fn before_render() {
    MAXIMIZED.with_borrow(|active| {
        if let Some(active) = active {
            active.restore();
        }
    });
}

pub(super) fn release(tile: &Element) {
    MAXIMIZED.with_borrow_mut(|active| {
        if active.as_ref().is_some_and(|active| active.tile == *tile) {
            *active = None;
        }
    });
}

pub(super) fn after_render() {
    MAXIMIZED.with_borrow_mut(|active| {
        if let Some(current) = active {
            if current.tile.is_connected() {
                current.lift();
            } else {
                // Closing/replacing the tile must not leave an orphan overlay.
                *active = None;
            }
            ResizeEvent::signal().force(());
        }
    });
}

impl Maximized {
    fn restore(&self) {
        if let Some(parent) = self.slot.parent_node() {
            parent
                .replace_child(&self.tile, &self.slot)
                .or_throw("restore tile");
        }
    }

    fn lift(&self) {
        if self.tile.parent_element().as_ref() == Some(&self.host) {
            return;
        }
        if let Some(parent) = self.tile.parent_node() {
            parent
                .replace_child(&self.slot, &self.tile)
                .or_throw("reserve tile slot");
            self.host.append_child(&self.tile).or_throw("maximize tile");
        }
    }
}

impl Drop for Maximized {
    fn drop(&mut self) {
        self.restore();
        self.host.remove();
    }
}
