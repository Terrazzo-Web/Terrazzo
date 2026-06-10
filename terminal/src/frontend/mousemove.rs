#![cfg(feature = "client")]

use std::sync::Mutex;

use terrazzo::autoclone;
use terrazzo::envelope;
use terrazzo::prelude::Closure;
use terrazzo::prelude::OrElseLog;
use terrazzo::prelude::XSignal;
use wasm_bindgen::JsCast;
use web_sys::EventTarget;
use web_sys::MouseEvent;
use web_sys::window;

#[envelope]
pub struct MousemoveManagerImpl {
    start: Mutex<Option<Position>>,
    pub delta: XSignal<Option<Position>>,
    events: Mutex<Vec<RegisteredEvent<MouseEvent>>>,
    mouseup: Option<Box<dyn Fn()>>,
}

pub use MousemoveManagerImplPtr as MousemoveManager;

use crate::tiles::api::Direction;

unsafe impl Send for MousemoveManager {}
unsafe impl Sync for MousemoveManager {}

impl MousemoveManager {
    pub fn new() -> Self {
        MousemoveManagerImpl {
            start: Default::default(),
            delta: XSignal::new("mousemove-delta", Default::default()),
            events: Default::default(),
            mouseup: None,
        }
        .into()
    }

    pub fn new2(mouseup: impl Fn() + 'static) -> Self {
        MousemoveManagerImpl {
            start: Default::default(),
            delta: XSignal::new("mousemove-delta", Default::default()),
            events: Default::default(),
            mouseup: Some(Box::new(mouseup)),
        }
        .into()
    }

    #[autoclone]
    pub fn mousedown(&self) -> impl Fn(MouseEvent) + 'static {
        let this = self.clone();
        move |ev| {
            *this.start.lock().or_throw("start") =
                Some(Position::from(ev) - this.delta.get_value_untracked().unwrap_or_default());
            this.events.lock().or_throw("events").clear();

            let window = window().or_throw("window");
            let mousemove: Closure<dyn Fn(MouseEvent)> = Closure::new(move |ev: MouseEvent| {
                autoclone!(this);
                let Some(start) = &*this.start.lock().or_throw("start") else {
                    return;
                };
                let cur = Position::from(ev);
                this.delta.set(Some(cur - *start));
            });
            let mousemove = RegisteredEvent::register(window.clone(), "mousemove", mousemove);

            let mouseup: Closure<dyn Fn(MouseEvent)> = Closure::new(move |_: MouseEvent| {
                autoclone!(this);
                *this.start.lock().or_throw("start") = None;
                this.events.lock().or_throw("events").clear();
                if let Some(mouseup) = &this.mouseup {
                    mouseup()
                }
            });
            let mouseup = RegisteredEvent::register(window.clone(), "mouseup", mouseup);

            let mut events = this.events.lock().or_throw("events");
            events.push(mousemove);
            events.push(mouseup);
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn get(&self, direction: Direction) -> i32 {
        match direction {
            Direction::Horizontal => self.x,
            Direction::Vertical => self.y,
        }
    }
}

impl From<MouseEvent> for Position {
    fn from(ev: MouseEvent) -> Self {
        Self {
            x: ev.page_x(),
            y: ev.page_y(),
        }
    }
}

impl std::ops::Sub for Position {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

struct RegisteredEvent<E> {
    target: EventTarget,
    event_type: &'static str,
    listener: Closure<dyn Fn(E)>,
}

impl<E> RegisteredEvent<E> {
    fn register(
        target: impl JsCast,
        event_type: &'static str,
        listener: Closure<dyn Fn(E)>,
    ) -> Self {
        let target: EventTarget = target.dyn_into().or_throw("Not an EventTarget");
        let () = target
            .add_event_listener_with_callback(event_type, listener.as_ref().unchecked_ref())
            .or_throw("Failed to attach event listener");
        Self {
            target,
            event_type,
            listener,
        }
    }
}

impl<E> Drop for RegisteredEvent<E> {
    fn drop(&mut self) {
        let Self {
            target,
            event_type,
            listener,
        } = self;
        let () = target
            .remove_event_listener_with_callback(event_type, listener.as_ref().unchecked_ref())
            .or_throw("Failed to detach event listener");
    }
}
