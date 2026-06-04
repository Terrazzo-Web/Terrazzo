#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::error;
use super::super::ui::side_view_width;
use crate::assets::icons;
use crate::frontend::mousemove::MousemoveManager;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::client::list_folder;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::manager::SideViewNotify;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::mutation::filter_active_folder_content;
use crate::text_editor::side::mutation::show_folder_content;
use crate::utils::more_path::MorePath as _;

terrazzo_css::import_style!(style, "side.scss");

#[html]
#[template(tag = div, key = "side-view")]
pub fn show_side_view(
    manager: Ptr<TextEditorManager>,
    #[signal] base: Arc<Path>,
    #[signal] side_view: Arc<SideViewList>,
    resize_manager: MousemoveManager,
) -> XElement {
    let root = base
        .file_name()
        .map(Path::new)
        .unwrap_or_else(|| "/".as_ref());
    let current_path = manager.path.as_ref().map(XSignal::get_value_untracked);
    let side_view = [(
        root.into(),
        SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Active,
            },
            item: SvnItem::Folder {
                folder: side_view,
                notify: manager.watch_side_view_folder(&current_path),
            },
        }
        .into(),
    )]
    .into_iter()
    .collect();
    tag(
        class = style::SIDE,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view",
        style::flex %= side_view_width(resize_manager.delta.clone()),
        show_side_view_list(&manager, Path::new("").into(), Arc::new(side_view), true),
    )
}

#[html]
fn show_side_view_list(
    manager: &Ptr<TextEditorManager>,
    path: Arc<Path>,
    side_view: Arc<SideViewList>,
    root: bool,
) -> XElement {
    let side_view = side_view.iter();
    let side_view = side_view.map(|(name, child)| {
        show_side_view_node(
            manager,
            if root {
                path.clone()
            } else {
                path.join(name.as_ref()).into()
            },
            name,
            child,
        )
    });
    ul(side_view.collect::<Vec<_>>()..)
}

#[autoclone]
#[html]
fn show_side_view_node(
    manager: &Ptr<TextEditorManager>,
    path: Arc<Path>,
    name: &Path,
    side_view: &Arc<SideViewNode>,
) -> XElement {
    let name_display = name.display();
    li(match &side_view.item {
        SvnItem::Folder { folder, notify: _ } => {
            let is_expanded = folder
                .values()
                .any(|child| child.properties.status == SvnStatus::Show);
            div(
                key = "folder",
                #[cfg(not(feature = "client-prod"))]
                class = "side-view-folder",
                #[cfg(not(feature = "client-prod"))]
                data_folder_path = path.to_owned_string(),
                div(
                    class = style::FOLDER,
                    #[cfg(not(feature = "client-prod"))]
                    class = "side-view-folder-row",
                    img(src = icons::folder(), class = style::ICON),
                    div(
                        class %= selected_item(manager.path.file.clone(), path.clone()),
                        span(
                            "{name_display}",
                            click = move |_| {
                                autoclone!(manager, path);
                                manager.path.file.set(path.clone())
                            },
                        ),
                    ),
                    folder_expand_icon(manager, &path, is_expanded),
                    (*path != "".as_ref()).then(|| close_icon(manager, &path))..,
                ),
                div(
                    class = style::SUB_FOLDER,
                    show_side_view_list(manager, path, folder.clone(), false),
                ),
            )
        }
        SvnItem::File { metadata, .. } => {
            let name = &metadata.name;
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
                        autoclone!(manager, path);
                        manager.path.file.set(path.clone())
                    },
                ),
                close_icon(manager, &path),
            )
        }
    })
}

#[template(wrap = true)]
fn selected_item(#[signal] file_path: Arc<Path>, path: Arc<Path>) -> XAttributeValue {
    if file_path == path {
        style::SELECTED_LABEL
    } else {
        style::LABEL
    }
}

#[autoclone]
#[html]
fn folder_expand_icon(
    manager: &Ptr<TextEditorManager>,
    path: &Arc<Path>,
    has_displayed_children: bool,
) -> XElement {
    if has_displayed_children {
        return img(
            src = icons::collapse_vert(),
            class = style::ICON,
            #[cfg(not(feature = "client-prod"))]
            class = "side-view-collapse-folder",
            click = move |_ev| {
                autoclone!(manager, path);
                manager.side_view.update(|side_view| {
                    let path = FilePath {
                        base: manager.path.base.get_value_untracked(),
                        file: path.clone(),
                    };
                    Some(filter_active_folder_content(
                        &manager,
                        side_view.clone(),
                        &path,
                    ))
                });
            },
        );
    }
    img(
        src = icons::split_vert(),
        class = style::ICON,
        class = style::HOVER_IMG,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-expand-folder",
        click = move |_ev| {
            autoclone!(manager, path);
            spawn_local(async move {
                autoclone!(manager, path);
                let path = FilePath {
                    base: manager.path.base.get_value_untracked(),
                    file: path.clone(),
                };
                let content = list_folder(manager.remote.clone(), path.clone())
                    .await
                    .inspect_err(|error| error!("Failed to load folder {path:?}: {error}"));
                let Ok(Some(content)) = content else {
                    return;
                };
                manager.side_view.update(|side_view| {
                    Some(show_folder_content(
                        &manager,
                        side_view.clone(),
                        &path,
                        content.as_ref(),
                    ))
                });
            });
        },
    )
}

#[autoclone]
#[html]
fn close_icon(manager: &Ptr<TextEditorManager>, path: &Arc<Path>) -> XElement {
    img(
        src = icons::close_tab(),
        class = style::ICON,
        class = style::HOVER_IMG,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-close-file",
        click = move |_ev| {
            autoclone!(manager, path);
            manager.remove_from_side_view(&path);
        },
    )
}
