use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
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
    img(
        class = style::TOGGLE_EDITOR_DIFF,
        class = (mode == SideViewMode::Git).then_some(style::ACTIVE),
        #[cfg(not(feature = "client-prod"))]
        class = "toggle-git-side-view",
        src = icons::git(),
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

pub fn refresh(manager: &Ptr<TextEditorManager>) {
    let manager = manager.clone();
    let base = manager.path.base.get_value_untracked();
    manager.is_git_repo.set(false);
    spawn_local(async move {
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
    });
}
