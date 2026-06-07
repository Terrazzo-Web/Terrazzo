#![cfg(feature = "client")]

use std::path::Path;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;
use web_sys::DragEvent;
use web_sys::MouseEvent;

use self::diagnostics::debug;
use self::diagnostics::error;
use super::SideViewList;
use super::SideViewNode;
use super::SvnItem;
use super::SvnProperties;
use super::SvnStatus;
use super::mutation::filter_active_folder_content;
use super::mutation::show_folder_content;
use crate::assets::icons;
use crate::frontend::mousemove::Position;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::fsio::ROOT_BASE_PATH;
use crate::text_editor::fsio::ROOT_FILE_PATH;
use crate::text_editor::fsio::client::list_folder;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::ui::RemoveBehavior;

terrazzo_css::import_style!(style, "side.scss");

const SIDE_VIEW_DRAG_KEY: &str = "text_editor_side_view_path";

#[cfg(not(feature = "client-prod"))]
use crate::utils::more_path::MorePath as _;

impl TextEditorManager {
    pub fn show_side_view(self: &Ptr<TextEditorManager>) -> XElement {
        show_side_view(self.clone(), self.path.base.clone(), self.side_view.clone())
    }
}

#[html]
#[template(tag = div, key = "side-view")]
fn show_side_view(
    manager: Ptr<TextEditorManager>,
    #[signal] base: Arc<Path>,
    #[signal] side_view: Option<Arc<SideViewNode>>,
) -> XElement {
    debug!(?base, "Loading side view");
    let root = base
        .file_name()
        .map(Path::new)
        .unwrap_or_else(|| &ROOT_BASE_PATH);
    return tag(
        class = style::SIDE,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view",
        style::flex %= side_view_width(manager.side_view_resize_manager.delta.clone()),
        side_view.map(|side_view| {
            show_side_view_node(
                &manager,
                &FilePath {
                    base: base.clone(),
                    file: ROOT_FILE_PATH.clone(),
                },
                root,
                &side_view,
            )
        })..,
    );

    #[template(wrap = true)]
    fn side_view_width(#[signal] position: Option<Position>) -> XAttributeValue {
        let position = position.unwrap_or_default();
        format!("0 0 max(8rem, calc(200px + {}px))", position.x)
    }
}

#[html]
fn show_side_view_node(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    name: &Path,
    side_view: &SideViewNode,
) -> XElement {
    debug!(?path, ?name, "Show side view node");
    li(match &side_view.item {
        SvnItem::Folder { folder, notify: _ } => {
            show_side_view_folder(manager, path, name, &side_view.properties, folder)
        }
        SvnItem::File { metadata, .. } => show_side_view_file(manager, path, &side_view.properties, metadata),
    })
}

#[autoclone]
#[html]
fn show_side_view_folder(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    name: &Path,
    properties: &SvnProperties,
    folder: &Arc<SideViewList>,
) -> XElement {
    let name_display = name.display();
    let is_expanded = folder
        .values()
        .any(|child| child.properties.status == SvnStatus::Show);
    div(
        key = "folder",
        draggable = (*path.file != *Path::new("")).then_some("true"),
        dragstart = drag_side_view_node(path),
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-folder",
        #[cfg(not(feature = "client-prod"))]
        data_folder_path = path.file.to_owned_string(),
        div(
            class = style::FOLDER,
            #[cfg(not(feature = "client-prod"))]
            class = "side-view-folder-row",
            dragover = move |event: DragEvent| {
                event.prevent_default();
                if let Some(data_transfer) = event.data_transfer() {
                    data_transfer.set_drop_effect("move");
                }
            },
            drop = drop_side_view_node(manager, path),
            img(src = icons::folder(), class = style::ICON),
            div(
                class %= selected_item(manager.path.file.clone(), path.file.clone()),
                dblclick = expand_folder(manager, path),
                click = move |_| {
                    autoclone!(manager, path);
                    manager.path.file.set(path.file.clone())
                },
                span("{name_display}", class = name_display_class(properties)),
            ),
            folder_expand_icon(manager, path, is_expanded),
            (*path.file != "".as_ref()).then(|| {
                close_icon(
                    manager,
                    path,
                    match properties.status {
                        SvnStatus::Active => RemoveBehavior::Soft,
                        SvnStatus::Show => RemoveBehavior::Hard,
                    },
                )
            })..,
        ),
        div(
            class = style::SUB_FOLDER,
            ul(folder.sorted_iter().map(|(name, child)| {
                show_side_view_node(
                    manager,
                    &FilePath {
                        base: path.base.clone(),
                        file: path.file.join(name.as_ref()).into(),
                    },
                    name,
                    child,
                )
            })..),
        ),
    )
}

#[autoclone]
#[html]
fn show_side_view_file(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    properties: &SvnProperties,
    metadata: &FileMetadata,
) -> XElement {
    let name = &metadata.name;
    div(
        key = "file",
        class = style::FILE,
        draggable = true,
        dragstart = drag_side_view_node(path),
        #[cfg(not(feature = "client-prod"))]
        data_file_path = path.file.to_owned_string(),
        img(src = icons::file(), class = style::ICON),
        div(
            class %= selected_item(manager.path.file.clone(), path.file.clone()),
            span(class = name_display_class(properties), "{name}"),
            click = move |_| {
                autoclone!(manager, path);
                manager.path.file.force(path.file.clone())
            },
        ),
        close_icon(manager, path, RemoveBehavior::Hard),
    )
}

#[autoclone]
fn drag_side_view_node(path: &FilePath<Arc<Path>>) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(path);
        let Some(data_transfer) = event.data_transfer() else {
            return;
        };
        let _ = data_transfer.set_data(SIDE_VIEW_DRAG_KEY, &path.file.to_string_lossy());
        data_transfer.set_effect_allowed("move");
    }
}

#[autoclone]
fn drop_side_view_node(
    manager: &Ptr<TextEditorManager>,
    destination_folder: &FilePath<Arc<Path>>,
) -> impl Fn(DragEvent) + 'static {
    move |event| {
        autoclone!(manager, destination_folder);
        event.prevent_default();
        event.stop_propagation();
        let Some(data_transfer) = event.data_transfer() else {
            return;
        };
        let Ok(source_file) = data_transfer.get_data(SIDE_VIEW_DRAG_KEY) else {
            return;
        };
        if source_file.is_empty() {
            return;
        }

        let source_path = Path::new(&source_file);
        let destination_path = destination_folder.file.as_ref();
        if source_path.parent() == Some(destination_path) {
            return;
        }
        if source_path.starts_with(destination_path) && source_path == destination_path {
            return;
        }
        if destination_path.starts_with(source_path) {
            return;
        }

        let move_manager = manager.clone();
        let move_destination_folder = destination_folder.clone();
        spawn_local(move_side_view_node(
            move_manager,
            FilePath {
                base: destination_folder.base.clone(),
                file: Arc::<Path>::from(source_path),
            },
            move_destination_folder,
        ));
    }
}

async fn move_side_view_node(
    manager: Ptr<TextEditorManager>,
    source: FilePath<Arc<Path>>,
    destination_folder: FilePath<Arc<Path>>,
) {
    let result = crate::text_editor::fsio::client::move_file(
        manager.remote.clone(),
        source.clone(),
        destination_folder.clone(),
    )
    .await;
    if let Err(error) = result {
        error!("Failed to move side-view entry: {error}");
        return;
    }

    let current = manager.path.file.get_value_untracked();
    let source_parent = source.file.parent().map(Arc::<Path>::from);
    if current == destination_folder.file || source_parent.as_ref() == Some(&current) {
        manager.path.file.force(current);
    } else {
        manager.tile.app.force(crate::tiles::app::App::TextEditor);
    }
}

#[template(wrap = true)]
fn selected_item(#[signal] file_path: Arc<Path>, path: Arc<Path>) -> XAttributeValue {
    if file_path == path {
        style::SELECTED_LABEL
    } else {
        style::LABEL
    }
}

#[html]
fn folder_expand_icon(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    is_expanded: bool,
) -> XElement {
    if !is_expanded {
        img(
            src = icons::split_vert(),
            class = style::BUTTON_HOVER_ICON,
            #[cfg(not(feature = "client-prod"))]
            class = "side-view-expand-folder",
            click = expand_folder(manager, path),
        )
    } else {
        img(
            src = icons::collapse_vert(),
            class = style::BUTTON_ICON,
            #[cfg(not(feature = "client-prod"))]
            class = "side-view-collapse-folder",
            click = collapse_folder(manager, path),
        )
    }
}

#[autoclone]
fn expand_folder(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
) -> impl Fn(MouseEvent) + 'static {
    move |_| {
        autoclone!(manager, path);
        spawn_local(async move {
            autoclone!(manager, path);
            debug!(?path, "Expand folder view");
            let content = list_folder(manager.remote.clone(), path.clone())
                .await
                .inspect_err(|error| error!("Failed to load folder {path:?}: {error}"));
            let Ok(Some(content)) = content else {
                debug!(?path, "Folder was not found or not a folder");
                return;
            };
            debug!(?path, "Found {} items", content.len());
            manager.side_view.update(|side_view| {
                let new_node =
                    show_folder_content(&manager, side_view.as_deref(), &path, content.as_ref());
                new_node.map(|new_node| Some(Arc::new(new_node)))
            });
        });
    }
}

#[autoclone]
fn collapse_folder(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
) -> impl Fn(MouseEvent) + 'static {
    move |_| {
        autoclone!(manager, path);
        manager.side_view.update(|side_view| {
            debug!(?path, "Collapse folder view");
            let new_node = filter_active_folder_content(&manager, side_view.as_deref(), &path);
            new_node.map(|new_node| Some(Arc::new(new_node)))
        });
    }
}

#[autoclone]
#[html]
fn close_icon(
    manager: &Ptr<TextEditorManager>,
    path: &FilePath<Arc<Path>>,
    behavior: RemoveBehavior,
) -> XElement {
    img(
        src = icons::close_tab(),
        class = style::BUTTON_HOVER_ICON,
        #[cfg(not(feature = "client-prod"))]
        class = "side-view-close-file",
        click = move |_ev| {
            autoclone!(manager, path);
            debug!(?path, "Remove item from side view");
            manager.remove_from_side_view(&path, behavior);
        },
    )
}

fn name_display_class(properties: &SvnProperties) -> impl Into<XAttributeValue> {
    (properties.status == SvnStatus::Show).then_some(style::SHOW_ONLY_ITEM)
}

impl SideViewList {
    pub fn sorted_iter(&self) -> impl Iterator<Item = (&Arc<Path>, &Arc<SideViewNode>)> {
        let iter = self.iter();
        let mut data = iter.collect::<Vec<_>>();
        data.sort_by_key(|(path, node)| {
            (
                !matches!(node.item, SvnItem::Folder { .. }),
                path.file_name()
                    .map(|n| n.to_ascii_lowercase())
                    .unwrap_or_default(),
            )
        });
        data.into_iter()
    }
}
