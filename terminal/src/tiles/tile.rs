#![cfg(feature = "client")]

use terrazzo::envelope;
use terrazzo::prelude::XSignal;

use crate::frontend::menu::MenuState;
use crate::frontend::remotes::Remote;

#[envelope]
pub struct Tile {
    pub remote: XSignal<Remote>,
    pub menu: MenuState,
}
