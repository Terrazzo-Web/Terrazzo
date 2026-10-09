use std::time::Duration;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::debounce::DoDebounce as _;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::warn;
use super::git_status;
use crate::assets::icons;
use crate::text_editor::manager::SideViewMode;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::style;
use crate::text_editor::ui::side_view;

#[html]
pub fn git_button(manager: &Ptr<TextEditorManager>) -> XElement {
    git_button_impl(
        manager.clone(),
        manager.is_git_repo.clone(),
        manager.side_view_mode.clone(),
    )
}

#[autoclone]
#[html]
#[template(tag = span)]
fn git_button_impl(
    manager: Ptr<TextEditorManager>,
    #[signal] is_git_repo: bool,
    #[signal] mode: SideViewMode,
) -> XElement {
    if !is_git_repo {
        return tag(style::display = "none", style::visibility = "hidden");
    }
    let src_signal = XSignal::new(
        "git-icon",
        if mode == SideViewMode::Git {
            icons::git()
        } else {
            icons::hdd()
        },
    );

    #[template(wrap = true)]
    pub fn make_src_signal(#[signal] mut src_signal: &'static str) -> XAttributeValue {
        src_signal
    }

    img(
        class = style::TOGGLE_EDITOR_DIFF,
        class = (mode == SideViewMode::Git).then_some(style::ACTIVE),
        #[cfg(not(feature = "client-prod"))]
        class = "toggle-git-side-view",
        src %= make_src_signal(src_signal.clone()),
        mouseover = move |_| {
            autoclone!(src_signal);
            src_signal.set(if mode != SideViewMode::Git {
                icons::git()
            } else {
                icons::hdd()
            })
        },
        mouseout = move |_| {
            autoclone!(src_signal);
            src_signal.set(if mode == SideViewMode::Git {
                icons::git()
            } else {
                icons::hdd()
            })
        },
        title = if mode == SideViewMode::Git {
            "Show files"
        } else {
            "Show Git changes"
        },
        click = move |_| {
            let batch = Batch::use_batch("Toggle Git side view");
            match manager.side_view_mode.get_value_untracked() {
                SideViewMode::Files => {
                    manager
                        .files_side_view
                        .force(manager.side_view.get_value_untracked());
                    manager.side_view_mode.set(SideViewMode::Git);
                    manager
                        .side_view
                        .force(manager.git_side_view.get_value_untracked());
                    refresh(&manager);
                }
                SideViewMode::Git => {
                    manager.side_view_mode.set(SideViewMode::Files);
                    manager
                        .side_view
                        .force(manager.files_side_view.get_value_untracked());
                }
            }
            drop(batch);
        },
    )
}

pub fn refresh_on_mouse_activity(manager: Ptr<TextEditorManager>) -> impl Fn(web_sys::MouseEvent) {
    let refresh = Duration::from_secs(1).async_throttle({
        let manager = manager.clone();
        move |()| {
            let manager = manager.clone();
            async move {
                if manager.side_view_mode.get_value_untracked() == SideViewMode::Git {
                    refresh_impl(&manager).await;
                }
            }
        }
    });
    move |_| {
        if manager.side_view_mode.get_value_untracked() == SideViewMode::Git {
            drop(refresh(()));
        }
    }
}

pub fn refresh(manager: &Ptr<TextEditorManager>) {
    let manager = manager.clone();
    manager.is_git_repo.set(false);
    spawn_local(async move { refresh_impl(&manager).await });
}

async fn refresh_impl(manager: &Ptr<TextEditorManager>) {
    let base = manager.path.base.get_value_untracked();
    let result = git_status(manager.remote.clone(), base.clone()).await;
    if manager.path.base.get_value_untracked() != base {
        return;
    }
    match result {
        Ok(Some(files)) => {
            let side_view = side_view::side_view(&manager, &base, &files);
            manager.git_side_view.force(Some(side_view.clone()));
            manager.is_git_repo.set(true);
            if manager.side_view_mode.get_value_untracked() == SideViewMode::Git {
                manager.side_view.force(Some(side_view));
            }
        }
        Ok(None) => {
            manager.is_git_repo.set(false);
            manager.git_side_view.force(None);
            if manager.side_view_mode.get_value_untracked() == SideViewMode::Git {
                manager
                    .side_view
                    .force(manager.files_side_view.get_value_untracked());
                manager.side_view_mode.set(SideViewMode::Files);
            }
        }
        Err(error) => warn!("Failed to load Git status: {error}"),
    }
}
