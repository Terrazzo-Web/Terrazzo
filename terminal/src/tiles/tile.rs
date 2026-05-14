#![cfg(feature = "client")]

use terrazzo::envelope;
use terrazzo::prelude::XSignal;

use super::app::App;
use crate::frontend::menu::MenuState;
use crate::frontend::remotes::Remote;

#[envelope]
pub struct Tile {
    pub app: XSignal<App>,
    pub remote: XSignal<Remote>,
    pub menu: MenuState,
}
