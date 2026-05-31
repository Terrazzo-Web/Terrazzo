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
                    close_icon(manager, &path),
                ),
                div(
                    class = style::SUB_FOLDER,
                    show_side_view_list(manager, &path, children.clone()),
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

/*
TODO:
- add icons terminal/assets/icons/arrows-expand.svg and terminal/assets/icons/arrows-collapse.svg before the close-icon of folder nodes.
- add an enum UiStatus Opened/Displayed on SideViewNode
- refactor SideViewNode to
  - struct SideViewNode { properties, item}
  - struct SideViewNodeProps { ui_status }
  - enum SideViewNodeItem { Folder(...), File(...) }
- when clicking on arrows-expand.svg, expand the folder by
  - fetch the folder content from server usig add a #[server(protocol = Http<Json, Json>)] list_folder() API to terminal/src/text_editor/fsio.rs.
  - update the folder of the arrows-expand.svg by adding missing elements with UiStatus=Displayed, exising elements are kept as-is.
- when clicking on arrows-collapse.svg, remove all child elements with UiStatus=Displayed
- add integration tests that create a folder tree in a temp folder with some files and check that opening files adds nodes that show in the side panel, clicking expand shows all the files in the folder, and clicking collapse removes from the side panel all the items that were not open.
  - folder structure:
    - root/
      - a/
        - a1.txt <- file content is "I am Alice"
        - a2.txt <- file content is "I am Bob"
        - c/
         - c.txt <- file content "I am Charlie"
      - b/
  - test scenario:
    - open text editor on base path root
    - search for a/a2.txt, open it
    - editor shows a file with content "I am Bob"
    - check that side view panel shows root/a/a2.txt
    - check that side view panel does **not** show root/a/a1.txt
    - click expand on root/a
    - check that side view panel **does** show root/a/a1.txt
    - check that side view panel **does** show root/a/c
    - check that side view panel does **not** show root/a/c/c.txt
    - click collapse on root/a
    - check that side view panel does **not** show root/a/a1.txt
    - check that side view panel does **not** show root/a/c
  - Consider adding classes to nodes to help the unit test navigate:
    ```
    #[cfg(not(feature = "client-prod"))]
    class = "<class name>",
    ```
*/

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
