#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;

use crate::assets::icons;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::utils::more_path::MorePath as _;

terrazzo_css::import_style!(style, "side.scss");

#[html]
#[template(tag = div, key = "side-view")]
pub fn show_side_view(
    manager: Ptr<TextEditorManager>,
    #[signal] side_view: Arc<SideViewList>,
) -> XElement {
    let base = manager.path.base.get_value_untracked();
    let base = Path::new(base.as_ref());
    let root = base
        .file_name()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_owned());
    let side_view = [(root.into(), SideViewNode::Folder(side_view).into())]
        .into_iter()
        .collect();
    tag(
        class = style::SIDE,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view",
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
    li(match &**side_view {
        SideViewNode::Folder(children) => {
            let file_path_signal = manager.path.file.clone();
            div(
                key = "folder",
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
                    (*path != "".as_ref()).then(|| close_icon(manager, &path))..,
                ),
                div(
                    class = style::SUB_FOLDER,
                    show_side_view_list(manager, &path, children.clone(), false),
                ),
            )
        }
        SideViewNode::File { metadata, .. } => {
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
