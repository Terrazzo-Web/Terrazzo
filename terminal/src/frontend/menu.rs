use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use terrazzo::autoclone;
use terrazzo::drop_list::DropListPtr;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::cancellable::Cancellable;
use terrazzo::widgets::debounce::DoDebounce as _;
use web_sys::MouseEvent;

use crate::assets::icons;
use crate::tiles::app::App;
use crate::tiles::signals::TilePtr;

terrazzo_css::import_style!(style, "menu.scss");

pub type DragHandle = Rc<dyn Fn(MouseEvent)>;

pub struct MenuState {
    pub show: XSignal<bool>,
    pub before: DropListPtr,
    pub drag_handle: RefCell<Option<DragHandle>>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            show: XSignal::new("show-menu", false),
            before: Default::default(),
            drag_handle: Default::default(),
        }
    }
}

#[autoclone]
#[html]
#[template(tag = div)]
pub fn menu(tile: TilePtr) -> XElement {
    let hide_menu = Duration::from_millis(500).cancellable();
    let drag_handle = tile.menu.drag_handle.borrow().clone();
    div(
        class = style::MENU,
        div(
            class = style::MENU_INNER,
            #[cfg(not(feature = "client-prod"))]
            class = "app-menu-trigger",
            img(
                class = style::MENU_ICON,
                src = icons::menu(),
                mousedown = move |ev: MouseEvent| {
                    if let Some(drag_handle) = &drag_handle {
                        ev.prevent_default();
                        drag_handle(ev);
                    }
                },
            ),
            mouseover = move |_: MouseEvent| {
                autoclone!(tile, hide_menu);
                tile.menu.before.reset();
                hide_menu.cancel();
                tile.menu.show.set(true);
            },
        ),
        mouseout = hide_menu.clone().wrap(move |_: MouseEvent| {
            autoclone!(tile);
            tile.menu.show.set(false)
        }),
        menu_items(tile.clone(), tile.menu.show.clone(), hide_menu.clone()),
    )
}

#[autoclone]
#[html]
#[template(tag = ul)]
fn menu_items(
    tile: TilePtr,
    #[signal] mut show_menu: bool,
    hide_menu: Cancellable<Duration>,
) -> XElement {
    if show_menu {
        let mut items: Vec<XElement> = vec![];
        #[cfg(feature = "terminal")]
        items.push(menu_item(
            App::Terminal,
            tile.app.clone(),
            show_menu_mut.clone(),
            hide_menu.clone(),
        ));
        #[cfg(feature = "text-editor")]
        items.push(menu_item(
            App::TextEditor,
            tile.app.clone(),
            show_menu_mut.clone(),
            hide_menu.clone(),
        ));
        #[cfg(feature = "converter")]
        items.push(menu_item(
            App::Converter,
            tile.app.clone(),
            show_menu_mut.clone(),
            hide_menu.clone(),
        ));
        #[cfg(feature = "port-forward")]
        items.push(menu_item(
            App::PortForward,
            tile.app.clone(),
            show_menu_mut.clone(),
            hide_menu.clone(),
        ));
        items.push(li(
            class = style::SPLITS,
            div(img(
                class = style::SPLIT_ICON,
                #[cfg(not(feature = "client-prod"))]
                class = "split-horizontal",
                src = icons::split_horz(),
                click = tile.split_horz(show_menu_mut.clone(), hide_menu.clone()),
            )),
            div(img(
                class = style::SPLIT_ICON,
                #[cfg(not(feature = "client-prod"))]
                class = "split-vertical",
                src = icons::split_vert(),
                click = tile.split_vert(show_menu_mut.clone(), hide_menu.clone()),
            )),
            div(img(
                class = style::SPLIT_ICON,
                #[cfg(not(feature = "client-prod"))]
                class = "split-tabbed",
                src = icons::collection(),
                click = tile.tabify(show_menu_mut.clone(), hide_menu.clone()),
            )),
            div(img(
                class = style::SPLIT_ICON,
                #[cfg(not(feature = "client-prod"))]
                class = "float-tile",
                src = icons::window_stack(),
                click = tile.float(show_menu_mut.clone(), hide_menu.clone()),
            )),
            div(img(
                class = style::SPLIT_ICON,
                #[cfg(not(feature = "client-prod"))]
                class = "tile-close",
                src = icons::close_app(),
                click = tile.close(),
            )),
        ));
        tag(
            class = style::MENU_ITEMS,
            mouseover = move |_: MouseEvent| {
                autoclone!(hide_menu, show_menu_mut);
                hide_menu.cancel();
                show_menu_mut.set(true);
            },
            items..,
        )
    } else {
        tag(style::visibility = "hidden", style::display = "none")
    }
}

#[autoclone]
#[html]
#[template(tag = li)]
fn menu_item(
    app: App,
    #[signal] mut selected_app: App,
    show_menu_mut: MutableSignal<bool>,
    hide_menu: Cancellable<Duration>,
) -> XElement {
    tag(
        img(class = style::APP_ICON, src = app.icon()),
        "{app}",
        class = (selected_app == app).then_some(style::ACTIVE),
        click = move |_| {
            autoclone!(selected_app_mut);
            let batch = Batch::use_batch("select-app");
            hide_menu.cancel();
            show_menu_mut.set(false);
            selected_app_mut.set(app);
            drop(batch);
        },
    )
}

impl App {
    pub fn icon(&self) -> icons::Icon {
        match self {
            App::Default => icons::menu(),
            #[cfg(feature = "terminal")]
            App::Terminal => icons::terminal(),
            #[cfg(feature = "text-editor")]
            App::TextEditor => icons::text_editor(),
            #[cfg(feature = "converter")]
            App::Converter => icons::converter(),
            #[cfg(feature = "port-forward")]
            App::PortForward => icons::hub(),
        }
    }
}
