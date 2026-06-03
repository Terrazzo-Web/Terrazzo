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
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::mutation::add_displayed_folder_content;
use crate::text_editor::side::mutation::collapse_displayed_children;
use crate::utils::more_path::MorePath as _;

terrazzo_css::import_style!(style, "side.scss");

#[html]
#[template(tag = div, key = "side-view")]
pub fn show_side_view(
    manager: Ptr<TextEditorManager>,
    #[signal] side_view: Arc<SideViewList>,
    resize_manager: MousemoveManager,
) -> XElement {
    let base = manager.path.base.get_value_untracked();
    let base = Path::new(base.as_ref());
    let root = base
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_owned());
    let side_view = [(
        root.into(),
        SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Opened,
            },
            item: SvnItem::Folder(side_view),
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
        show_side_view_list(&manager, "".as_ref(), Arc::new(side_view), true),
    )
}

#[html]
fn show_side_view_list(
    manager: &Ptr<TextEditorManager>,
    path: &Path,
    side_view: Arc<SideViewList>,
    root: bool,
) -> XElement {
    ul(side_view
        .iter()
        .map(|(name, child)| show_side_view_node(manager, path, name, child, root))
        .collect::<Vec<_>>()..)
}

#[autoclone]
#[html]
fn show_side_view_node(
    manager: &Ptr<TextEditorManager>,
    path: &Path,
    name: &Arc<str>,
    side_view: &Arc<SideViewNode>,
    root: bool,
) -> XElement {
    let path: Arc<Path> = if root {
        path.into()
    } else {
        Arc::from(path.join(name.as_ref()))
    };
    li(match &side_view.item {
        SvnItem::Folder(children) => {
            let file_path_signal = manager.path.file.clone();
            let has_displayed_children = children
                .values()
                .any(|child| child.properties.status == SvnStatus::Displayed);
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
                            "{name}",
                            click = move |_| {
                                autoclone!(path);
                                file_path_signal.set(path.to_owned_string())
                            },
                        ),
                    ),
                    folder_expand_icon(manager, &path, has_displayed_children),
                    (*path != "".as_ref()).then(|| close_icon(manager, &path))..,
                ),
                div(
                    class = style::SUB_FOLDER,
                    show_side_view_list(manager, &path, children.clone(), false),
                ),
            )
        }
        SvnItem::File { metadata, .. } => {
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
                let path_vec = path_vec(&path);
                manager.side_view.update(|side_view| {
                    Some(collapse_displayed_children(side_view.clone(), &path_vec))
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
            let path_vec = path_vec(&path);
            let folder_path: Arc<str> = path.to_owned_string().into();
            spawn_local(async move {
                autoclone!(manager);
                let path = FilePath {
                    base: manager.path.base.get_value_untracked(),
                    file: folder_path,
                };
                let content = list_folder(manager.remote.clone(), path.clone())
                    .await
                    .inspect_err(|error| error!("Failed to load folder {path:?}: {error}"));
                let Ok(Some(content)) = content else {
                    return;
                };
                manager.side_view.update(|side_view| {
                    Some(add_displayed_folder_content(
                        &manager,
                        side_view.clone(),
                        &path_vec,
                        content.as_ref(),
                    ))
                });
            });
        },
    )
}

pub fn path_vec(path: &Path) -> Vec<Arc<str>> {
    path.iter()
        .map(|leg| Arc::from(leg.to_owned_string()))
        .collect()
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
