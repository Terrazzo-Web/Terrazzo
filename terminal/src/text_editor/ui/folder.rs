use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;
use web_sys::DragEvent;
use web_sys::MouseEvent;

use self::diagnostics::debug;
use self::diagnostics::warn;
use crate::assets::icons;
use crate::frontend::timestamp;
use crate::frontend::timestamp::datetime::DateTime;
use crate::frontend::timestamp::display_timestamp;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::manager::EditorDataState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::server_fn::EventKind;
use crate::text_editor::notify::server_fn::FileEventKind;
use crate::text_editor::notify::ui::NotifyRegistration;
use crate::text_editor::ui::ROOT_FILE_PATH;
use crate::text_editor::ui::drag::on_move_dragover;
use crate::text_editor::ui::drag::on_move_dragstart;
use crate::text_editor::ui::drag::on_move_drop;
use crate::tiles::app::App;

terrazzo_css::import_style!(style, "folder.scss");

#[autoclone]
#[html]
#[template(tag = div)]
pub fn folder(
    manager: Ptr<TextEditorManager>,
    editor_state: Option<EditorDataState>,
    list: Arc<Vec<FileMetadata>>,
) -> XElement {
    let FolderState {
        parent,
        notify_registration,
        file_path: folder_path,
    } = process_folder_state(&manager, editor_state);
    let mut rows = vec![];
    for file in parent.iter().chain(list.iter()) {
        let name = &file.name;
        let is_dir = file.is_dir;
        let display_name = if is_dir {
            format!("{name}/")
        } else {
            name.to_string()
        };
        let size = file.size.map(print_size).unwrap_or_else(|| "-".to_owned());
        let modified = file
            .modified
            .map(DateTime::from_utc)
            .map(|m| timestamp(display_timestamp(m)))
            .unwrap_or_else(|| span("-"));
        let user = file.user.clone().unwrap_or_default();
        let group = file.group.clone().unwrap_or_default();
        let permissions = file
            .mode
            .map(|m| {
                format!(
                    "{}{}",
                    if is_dir { 'd' } else { '-' },
                    mode_to_permissions(m)
                )
            })
            .unwrap_or_default();
        let file_path = FilePath {
            base: manager.path.base.get_value_untracked(),
            file: folder_entry_path(&folder_path, name).into(),
        };
        rows.push(tr(
            id = "{name}",
            #[cfg(not(feature = "client-prod"))]
            class = "folder-row",
            click = move |_| {
                autoclone!(manager, file_path);
                manager.path.file.set(file_path.file.clone())
            },
            td(
                draggable = (name.as_ref() != "..").then_some("true"),
                dragstart = on_move_dragstart(&file_path),
                #[cfg(not(feature = "client-prod"))]
                class = "folder-name",
                "{display_name}",
            ),
            td("{size}"),
            td(modified),
            td("{user}"),
            td("{group}"),
            td("{permissions}"),
            td(
                class = style::FOLDER_ACTIONS,
                trash_action(
                    manager.clone(),
                    folder_path.clone(),
                    file_path.clone(),
                    name.clone(),
                ),
            ),
        ));
    }

    let destination_folder = FilePath {
        base: manager.path.base.get_value_untracked(),
        file: folder_path.clone(),
    };
    let dragover_class: XSignal<Option<&'static str>> = XSignal::new("folder-view-dragover", None);
    #[template(wrap = true)]
    fn get_dragover_class(#[signal] dragover_class: Option<&'static str>) -> XAttributeValue {
        dragover_class
    }
    let on_move_drop = on_move_drop(&manager, &destination_folder);

    tag(
        class = style::FOLDER,
        dragover = move |event: DragEvent| {
            autoclone!(dragover_class);
            if !on_move_dragover(event) {
                return;
            }
            dragover_class.set(style::MOVE_DRAGOVER);
        },
        dragleave = move |_| {
            autoclone!(dragover_class);
            dragover_class.set(None)
        },
        drop = move |event| {
            dragover_class.set(None);
            on_move_drop(event);
        },
        class %= get_dragover_class(dragover_class.clone()),
        table(
            thead(tr(
                th("Name"),
                th("Size"),
                th("Modified"),
                th("User"),
                th("Group"),
                th("Permissions"),
                th(""),
            )),
            tbody(
                mouseover = move |_: MouseEvent| {
                    manager.tile.menu.before.reset();
                },
                rows..,
            ),
        ),
        after_render = move |_| {
            let _moved = &notify_registration;
        },
    )
}

#[autoclone]
#[html]
fn trash_action(
    manager: Ptr<TextEditorManager>,
    folder_path: Arc<Path>,
    file_path: FilePath<Arc<Path>>,
    name: Arc<str>,
) -> XElement {
    if &*name == ".." {
        return span();
    }
    img(
        class = style::TRASH_ACTION,
        #[cfg(not(feature = "client-prod"))]
        class = "folder-trash-icon",
        src = icons::trash(),
        title = "Move to trash",
        click = move |event: MouseEvent| {
            autoclone!(manager, folder_path, file_path);
            event.stop_propagation();
            spawn_local(delete_file(
                manager.clone(),
                folder_path.clone(),
                file_path.clone(),
            ));
        },
    )
}

async fn delete_file(
    manager: Ptr<TextEditorManager>,
    folder_path: Arc<Path>,
    file_path: FilePath<Arc<Path>>,
) {
    let result = fsio::client::delete_file(manager.remote.clone(), file_path).await;
    if let Err(error) = result {
        warn!("Failed to move entry to trash: {error}");
        return;
    }
    if manager.path.file.get_value_untracked() == folder_path {
        manager.path.file.force(folder_path);
    } else {
        manager.tile.app.force(App::TextEditor);
    }
}

fn folder_entry_path(folder_path: &Path, name: &str) -> PathBuf {
    let folder_path = folder_path.strip_prefix("/").unwrap_or(folder_path);
    if name == ".." {
        folder_path.parent().map(Path::to_owned).unwrap_or_default()
    } else {
        folder_path.join(name)
    }
}

struct FolderState {
    parent: Option<FileMetadata>,
    notify_registration: Option<Ptr<NotifyRegistration>>,
    file_path: Arc<Path>,
}

impl Default for FolderState {
    fn default() -> Self {
        Self {
            parent: Default::default(),
            notify_registration: Default::default(),
            file_path: ROOT_FILE_PATH.clone(),
        }
    }
}

#[autoclone]
fn process_folder_state(
    manager: &Ptr<TextEditorManager>,
    editor_state: Option<EditorDataState>,
) -> FolderState {
    let Some(editor_state) = editor_state else {
        return FolderState::default();
    };

    let path = &editor_state.path;
    let file_path = &path.file;

    let parent_path = file_path.parent();
    let parent = parent_path.map(|_| FileMetadata {
        name: "..".into(),
        is_dir: true,
        ..FileMetadata::default()
    });

    let notify_registration =
        manager
            .notify_service
            .watch_folder(&editor_state.path, move |event| {
                autoclone!(path, manager);
                debug!("Folder view notification: {event:?}");
                let EventKind::File(kind) = event.kind else {
                    return;
                };
                let full_path = path.as_deref().full_path();
                match (event.path.as_ref() == full_path.as_path(), kind) {
                    (
                        false,
                        FileEventKind::Create | FileEventKind::Modify | FileEventKind::Delete,
                    ) => {
                        debug!("File inside the folder was added/removed ==> refresh the view");
                    }
                    (true, FileEventKind::Create) => {
                        debug!("The folder was created !?!");
                        return;
                    }
                    (true, FileEventKind::Modify) => {
                        debug!("The folder was modified");
                    }
                    (true, FileEventKind::Delete) => {
                        debug!("The folder was deleted");
                        manager.path.file.update(|file_path| {
                            let parent = file_path.parent().unwrap_or_else(|| "/".as_ref());
                            Some(Arc::from(parent))
                        });
                        return;
                    }
                    (true | false, FileEventKind::Error) => {
                        debug!("Error polling notifications");
                        return;
                    }
                }

                debug!("Force reload folder view");
                manager
                    .path
                    .file
                    .force(manager.path.file.get_value_untracked());
            });
    FolderState {
        parent,
        notify_registration: Some(notify_registration),
        file_path: file_path.clone(),
    }
}

#[html]
#[template(tag = span)]
fn timestamp(#[signal] mut t: Box<timestamp::Timestamp>) -> XElement {
    tag(
        "{t}",
        before_render = move |_| {
            let _moved = &t_mut;
        },
    )
}

fn mode_to_permissions(mode: u32) -> String {
    // Unix permission bits: user, group, others
    const PERMISSIONS: [(u32, char); 9] = [
        (0o400, 'r'), // user read
        (0o200, 'w'), // user write
        (0o100, 'x'), // user execute
        (0o040, 'r'), // group read
        (0o020, 'w'), // group write
        (0o010, 'x'), // group execute
        (0o004, 'r'), // other read
        (0o002, 'w'), // other write
        (0o001, 'x'), // other execute
    ];

    PERMISSIONS
        .iter()
        .map(|(bit, ch)| if mode & bit != 0 { *ch } else { '-' })
        .collect()
}

fn print_size(size: u64) -> String {
    if size < 1000 {
        return format!("{size}b");
    }

    let mut sizef = size as f64 / 1024.;
    for suffix in ["Kb", "Mb", "Gb", "Tb"] {
        if sizef < 1000. {
            return format!("{sizef:.2}{suffix}");
        }
        sizef /= 1024.
    }
    return format!("{sizef:.2}Pb");
}

#[cfg(test)]
mod tests {
    #[test]
    fn print_size() {
        assert_eq!(
            "[123b, 120.24Kb, 117.42Mb, 114.67Gb, 111.98Tb, 109.36Pb, 10935.53Pb]",
            format!(
                "[{}, {}, {}, {}, {}, {}, {}]",
                super::print_size(123),
                super::print_size(123123),
                super::print_size(123123123),
                super::print_size(123123123123),
                super::print_size(123123123123123),
                super::print_size(123123123123123123),
                super::print_size(12312312312312312312)
            )
        )
    }
}
