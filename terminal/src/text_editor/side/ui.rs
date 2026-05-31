#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use crate::assets::icons;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SideViewNodeItem;
use crate::text_editor::side::UiStatus;
use crate::utils::more_path::MorePath as _;

terrazzo_css::import_style!(style, "side.scss");

#[html]
#[template(tag = div, key = "side-view")]
pub fn show_side_view(
    manager: Ptr<TextEditorManager>,
    #[signal] side_view: Arc<SideViewList>,
) -> XElement {
    tag(
        class = style::SIDE,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view",
        show_side_view_list(&manager, "".as_ref(), side_view),
    )
}

#[html]
fn show_side_view_list(
    manager: &Ptr<TextEditorManager>,
    path: &Path,
    side_view: Arc<SideViewList>,
) -> XElement {
    ul(side_view
        .iter()
        .map(|(name, child)| show_side_view_node(manager, path, name, child))
        .collect::<Vec<_>>()..)
}

#[autoclone]
#[html]
fn show_side_view_node(
    manager: &Ptr<TextEditorManager>,
    path: &Path,
    name: &Arc<str>,
    side_view: &Arc<SideViewNode>,
) -> XElement {
    let path: Arc<Path> = Arc::from(path.join(name.as_ref()));
    li(match &side_view.item {
        SideViewNodeItem::Folder(children) => {
            let file_path_signal = manager.path.file.clone();
            let has_displayed_children = children
                .values()
                .any(|child| child.properties.ui_status == UiStatus::Displayed);
            div(
                key = "folder",
                #[cfg(not(feature = "client-prod"))]
                class = "side-view-folder",
                #[cfg(not(feature = "client-prod"))]
                data_folder_path = path.to_owned_string(),
                div(
                    class = style::FOLDER,
                    img(src = icons::folder(), class = style::ICON),
                    div(
                        class %= selected_item(manager.path.file.clone(), path.clone()),
                        span(
                            "{name}",
                            click = move |_| {
                                autoclone!(path);
                                file_path_signal.set(path.to_owned_string())
                            },
                        ),
                    ),
                    folder_arrow(manager, &path, has_displayed_children),
                    close_icon(manager, &path),
                ),
                div(
                    class = style::SUB_FOLDER,
                    show_side_view_list(manager, &path, children.clone()),
                ),
            )
        }
        SideViewNodeItem::File { metadata, .. } => {
            let name = &metadata.name;
            let file_path_signal = manager.path.file.clone();
            div(
                key = "file",
                class = style::FILE,
                #[cfg(not(feature = "client-prod"))]
                data_file_path = path.to_owned_string(),
                img(src = icons::file(), class = style::ICON),
                div(
                    class %= selected_item(manager.path.file.clone(), path.clone()),
                    span("{name}"),
                    click = move |_| {
                        autoclone!(path);
                        file_path_signal.set(path.to_owned_string())
                    },
                ),
                close_icon(manager, &path),
            )
        }
    })
}

#[template(wrap = true)]
fn selected_item(#[signal] file_path: Arc<str>, path: Arc<Path>) -> XAttributeValue {
    let file_path: &Path = (*file_path).as_ref();
    if file_path == path.as_ref() {
        style::SELECTED_LABEL
    } else {
        style::LABEL
    }
}

#[autoclone]
#[html]
fn folder_arrow(
    manager: &Ptr<TextEditorManager>,
    path: &Arc<Path>,
    has_displayed_children: bool,
) -> XElement {
    if has_displayed_children {
        return img(
            src = icons::arrows_collapse(),
            class = style::ICON,
            #[cfg(not(feature = "client-prod"))]
            class = "side-view-collapse-folder",
            click = move |_ev| {
                autoclone!(manager, path);
                let path_vec = path_vec(&path);
                manager.side_view.update(|side_view| {
                    Some(crate::text_editor::side::mutation::collapse_displayed_children(
                        side_view.clone(),
                        &path_vec,
                    ))
                });
            },
        );
    }
    img(
        src = icons::arrows_expand(),
        class = style::ICON,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-expand-folder",
        click = move |_ev| {
            autoclone!(manager, path);
            let path_vec = path_vec(&path);
            let folder_path: Arc<str> = path.to_owned_string().into();
            let manager_for_task = manager.clone();
            spawn_local(async move {
                let content = crate::text_editor::fsio::ui::list_folder(
                    manager_for_task.remote.clone(),
                    FilePath {
                        base: manager_for_task.path.base.get_value_untracked(),
                        file: folder_path,
                    },
                )
                .await;
                let Ok(Some(content)) = content else {
                    return;
                };
                manager_for_task.side_view.update(|side_view| {
                    Some(crate::text_editor::side::mutation::add_displayed_folder_content(
                        side_view.clone(),
                        &path_vec,
                        content.as_ref(),
                    ))
                });
            });
        },
    )
}

fn path_vec(path: &Path) -> Vec<Arc<str>> {
    path.iter()
        .map(|leg| Arc::from(leg.to_owned_string()))
        .collect()
}

#[autoclone]
#[html]
fn close_icon(manager: &Ptr<TextEditorManager>, path: &Arc<Path>) -> XElement {
    img(
        src = icons::close_tab(),
        class = format!("{} {}", style::ICON, style::CLOSE),
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-close-file",
        click = move |_ev| {
            autoclone!(manager, path);
            manager.remove_from_side_view(&path);
        },
    )
}
