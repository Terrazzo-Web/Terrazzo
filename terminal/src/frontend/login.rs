use std::sync::OnceLock;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::more_event::MoreEvent as _;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use web_sys::HtmlInputElement;

use self::diagnostics::Instrument as _;
use self::diagnostics::info;
use self::diagnostics::warn;
use crate::assets::icons;
use crate::tiles::ui::show_tiles;

terrazzo_css::import_style!(style, "login.scss");

#[autoclone]
#[html]
#[template]
pub fn login(#[signal] mut logged_in: LoggedInStatus) -> XElement {
    match logged_in {
        LoggedInStatus::Login => show_tiles(),
        LoggedInStatus::Logout => div(
            key = "login",
            class = style::LOGIN,
            img(class = style::KEY_ICON, src = icons::key_icon()),
            input(
                r#type = "password",
                after_render = |password: &Element| {
                    let password: &HtmlElement = password.dyn_ref().or_throw("password");
                    let () = password.focus().or_throw("password focus");
                },
                change = move |ev: web_sys::Event| {
                    let Ok(password): Result<HtmlInputElement, _> = ev
                        .current_target_element("The password field")
                        .map_err(|error| warn!("{error}"))
                    else {
                        return;
                    };

                    let login_task = async move {
                        autoclone!(logged_in_mut);
                        match crate::api::client::login::login(Some(&password.value())).await {
                            Ok(()) => logged_in_mut.set(LoggedInStatus::Login),
                            Err(error) => warn!("{error}"),
                        }
                    };
                    spawn_local(login_task.in_current_span());
                },
            ),
        ),
        LoggedInStatus::Unknown => {
            let login_task = async move {
                autoclone!(logged_in_mut);
                match crate::api::client::login::login(None).await {
                    Ok(()) => logged_in_mut.set(LoggedInStatus::Login),
                    Err(error) => {
                        logged_in_mut.set(LoggedInStatus::Logout);
                        info!("Authentication is required: {error}")
                    }
                }
            };
            spawn_local(login_task.in_current_span());
            div(key = "login-pending", class = style::LOGIN)
        }
    }
}

pub fn logged_in() -> XSignal<LoggedInStatus> {
    static LOGGED_IN: OnceLock<XSignal<LoggedInStatus>> = OnceLock::new();
    LOGGED_IN
        .get_or_init(|| XSignal::new("logged-in", LoggedInStatus::Unknown))
        .clone()
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum LoggedInStatus {
    Login,
    Logout,

    #[default]
    Unknown,
}
