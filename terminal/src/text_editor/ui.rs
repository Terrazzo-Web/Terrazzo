#![cfg(feature = "client")]

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use nameth::nameth;
use scopeguard::guard;
use server_fn::ServerFnError;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::warn;
use self::editor::EditorDocument;
use self::editor::editor;
use self::folder::folder;
use super::file_path::FilePath;
use super::fsio;
use super::fsio::ROOT_BASE_PATH;
use super::fsio::ROOT_FILE_PATH;
use super::manager::EditorDataState;
use super::manager::EditorState;
use super::manager::TextEditorManager;
use super::notify::manager::SideViewNotify;
use super::notify::ui::NotifyService;
use super::search::state::EditorSearchState;
use super::search::state::SearchState;
use super::side::SideViewNode;
use super::side::SvnItem;
use super::side::SvnProperties;
use super::side::SvnStatus;
use super::state;
use super::style;
use super::synchronized_state::SynchronizedState;
use super::synchronized_state::show_synchronized_state;
use crate::assets::icons;
use crate::frontend::menu::menu;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::remotes::Remote;
use crate::frontend::remotes_ui::show_remote;
use crate::frontend::resize_bar::resize_bar_horz;
use crate::tiles::app::App;
use crate::tiles::id::TileId;
use crate::tiles::signals::TilePtr;

mod code_mirror;
pub mod drag;
mod editor;
mod folder;
mod html_viewer;
mod pdf_viewer;

pub(super) const STORE_FILE_DEBOUNCE_DELAY: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(1500)
} else {
    Duration::from_millis(500)
};

/// The UI for the text editor app.
#[html]
#[template(tag = div)]
pub fn text_editor(tile: TilePtr) -> XElement {
    tag(
        style = "height: 100%;",
        text_editor_impl(tile.clone(), tile.remote.clone()),
    )
}

#[html]
#[template(tag = div)]
fn text_editor_impl(tile: TilePtr, #[signal] remote: Remote) -> XElement {
    let manager = Ptr::new(TextEditorManager {
        tile,
        remote: remote.clone(),
        path: FilePath {
            base: XSignal::new("base-path", ROOT_BASE_PATH.clone()),
            file: XSignal::new("file-path", ROOT_FILE_PATH.clone()),
        },
        force_edit_path: XSignal::new("force-edit-path", false),
        editor_state: XSignal::new("editor-state", EditorState::default()),
        show_editor_diff: XSignal::new("show-editor-diff", false),
        show_html_preview: XSignal::new("show-html-preview", true),
        synchronized_state: XSignal::new("synchronized-state", SynchronizedState::Sync),
        side_view: XSignal::new("side-view", None),
        notify_service: Ptr::new(NotifyService::new(remote)),
        search: SearchState::new(),
        side_view_resize_manager: MousemoveManager::new(),
    });

    let consumers = Arc::default();
    manager.restore_paths(&consumers);

    div(
        key = "text-editor",
        class = style::TEXT_EDITOR,
        #[cfg(not(feature = "client-prod"))]
        class = "text-editor-app",
        div(
            class = style::HEADER,
            menu(manager.tile.clone()),
            manager.base_path_selector(),
            manager.file_path_selector(),
            manager.search_selector(),
            fsio::ux::create_entry_controls(manager.clone(), manager.editor_state.clone()),
            toggle_html_preview(
                manager.editor_state.clone(),
                manager.show_html_preview.clone(),
            ),
            toggle_editor_diff(
                manager.editor_state.clone(),
                manager.show_editor_diff.clone(),
                manager.show_html_preview.clone(),
            ),
            manager.refresh_editor(),
            show_synchronized_state(manager.synchronized_state.clone()),
            show_remote(manager.tile.remote.clone()),
        ),
        editor_body(manager),
        after_render = move |_| {
            let _moved = &consumers;
        },
    )
}

impl TextEditorManager {
    #[html]
    fn refresh_editor(&self) -> XElement {
        let tile = self.tile.clone();
        img(
            class = style::REFRESH_EDITOR,
            #[cfg(not(feature = "client-prod"))]
            class = "refresh-editor",
            src = icons::refresh(),
            title = "Refresh editor",
            click = move |_| tile.app.force(App::TextEditor),
        )
    }
}

#[html]
fn editor_body(manager: Ptr<TextEditorManager>) -> XElement {
    div(
        class = super::style::BODY,
        #[cfg(not(feature = "client-prod"))]
        class = "editor-body",
        manager.show_side_view(),
        resize_bar_horz(manager.side_view_resize_manager.clone(), Default::default()),
        editor_container(
            manager.clone(),
            manager.editor_state.clone(),
            manager.show_editor_diff.clone(),
            manager.show_html_preview.clone(),
        ),
    )
}

#[html]
#[template(tag = span)]
fn toggle_html_preview(
    #[signal] editor_state: EditorState,
    show_html_preview: XSignal<bool>,
) -> XElement {
    let is_html = match editor_state {
        EditorState::Data(editor_state) => {
            editor_state.path.file.extension() == Some("html".as_ref())
        }
        _ => false,
    };
    if !is_html {
        return tag(style::display = "none", style::visibility = "hidden");
    }

    #[template(wrap = true)]
    fn make_class(#[signal] show_html_preview: bool) -> XAttributeValue {
        show_html_preview.then_some(style::ACTIVE)
    }

    #[template(wrap = true)]
    fn make_title(#[signal] show_html_preview: bool) -> XAttributeValue {
        if show_html_preview {
            "Show HTML source"
        } else {
            "Preview HTML"
        }
    }

    img(
        class = style::TOGGLE_HTML_PREVIEW,
        class %= make_class(show_html_preview.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "toggle-html-preview",
        src = icons::text_editor(),
        title %= make_title(show_html_preview.clone()),
        click = move |_| show_html_preview.update(|show| Some(!show)),
    )
}

#[html]
#[template(tag = span)]
fn toggle_editor_diff(
    #[signal] editor_state: EditorState,
    show_editor_diff: XSignal<bool>,
    #[signal] show_html_preview: bool,
) -> XElement {
    let has_diff = match editor_state {
        EditorState::Data(editor_state)
            if editor_state.path.file.extension() == Some("html".as_ref()) && show_html_preview =>
        {
            false
        }
        EditorState::Data(editor_state) => match &*editor_state.data {
            fsio::File::TextFile {
                original: Some(original),
                content,
                ..
            } => original != content,
            _ => false,
        },
        _ => false,
    };
    if !has_diff {
        return tag(style::display = "none", style::visibility = "hidden");
    }

    #[template(wrap = true)]
    fn make_class(#[signal] show_editor_diff: bool) -> XAttributeValue {
        show_editor_diff.then_some(style::ACTIVE)
    }

    #[template(wrap = true)]
    fn make_title(#[signal] show_editor_diff: bool) -> XAttributeValue {
        if show_editor_diff {
            "Hide diff"
        } else {
            "Show diff"
        }
    }

    return img(
        class = style::TOGGLE_EDITOR_DIFF,
        class %= make_class(show_editor_diff.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "toggle-editor-diff",
        src = icons::diff(),
        title %= make_title(show_editor_diff.clone()),
        click = move |_| show_editor_diff.update(|show| Some(!show)),
    );
}

#[html]
#[template(tag = div)]
fn editor_container(
    manager: Ptr<TextEditorManager>,
    #[signal] editor_state: EditorState,
    #[signal] show_editor_diff: bool,
    #[signal] show_html_preview: bool,
) -> XElement {
    let body = match editor_state {
        EditorState::Data(editor_state) => match &*editor_state.data {
            fsio::File::TextFile {
                original, content, ..
            } => {
                let editor_document = EditorDocument::Text {
                    original: original.clone(),
                    content: content.clone(),
                };
                editor(
                    manager,
                    editor_state,
                    editor_document,
                    show_editor_diff,
                    show_html_preview,
                )
            }
            fsio::File::PdfFile { base64, .. } => {
                let base64 = base64.clone();
                editor(
                    manager,
                    editor_state,
                    EditorDocument::Pdf(base64),
                    show_editor_diff,
                    show_html_preview,
                )
            }
            fsio::File::Folder(list) => {
                let list = list.clone();
                folder(manager, Some(editor_state), list)
            }
            fsio::File::Error(error) => {
                warn!("Failed to load file: {error}");
                return tag(
                    class = super::style::EDITOR_CONTAINER,
                    #[cfg(not(feature = "client-prod"))]
                    class = "editor-container",
                );
            }
        },
        EditorState::Search(EditorSearchState { results, .. }) => {
            let results = results.clone();
            folder(manager, None, results)
        }
        EditorState::Empty => {
            return tag(
                class = super::style::EDITOR_CONTAINER,
                #[cfg(not(feature = "client-prod"))]
                class = "editor-container",
            );
        }
    };
    tag(
        class = super::style::EDITOR_CONTAINER,
        #[cfg(not(feature = "client-prod"))]
        class = "editor-container",
        body,
    )
}

impl TextEditorManager {
    /// Restores the paths
    #[autoclone]
    #[nameth]
    fn restore_paths(self: &Ptr<Self>, consumers: &Arc<Mutex<Consumers>>) {
        let this = self;
        spawn_local(async move {
            autoclone!(this, consumers);
            let registrations = Consumers::default().append(this.make_file_async_view());
            let registrations = guard(registrations, |registrations| {
                *consumers.lock().unwrap() = registrations
                    .append(this.save_on_change(this.path.base.clone(), state::base_path::set))
                    .append(this.save_on_change(this.path.file.clone(), state::file_path::set))
                    .append(this.save_side_view_on_change())
                    .append(this.save_on_change(this.search.query.clone(), state::search::set))
                    .append(this.path.base.add_subscriber(move |base_path| {
                        autoclone!(this);
                        let batch = Batch::use_batch("Update base path");
                        this.path.file.force(ROOT_FILE_PATH.clone());
                        this.side_view.force(Some(Arc::new(SideViewNode {
                            properties: SvnProperties {
                                status: SvnStatus::Active,
                            },
                            item: SvnItem::Folder {
                                folder: Default::default(),
                                notify: this.watch_side_view_folder(&FilePath {
                                    base: base_path,
                                    file: ROOT_FILE_PATH.clone(),
                                }),
                            },
                        })));
                        drop(batch);
                    }))
            });
            let tile_id = this.tile.id;
            let remote: Remote = this.remote.clone();
            let (get_side_view, get_base_path, get_file_path, get_search) = futures::future::join4(
                state::side_view::get(Some(tile_id), remote.clone()),
                state::base_path::get(Some(tile_id), remote.clone()),
                state::file_path::get(Some(tile_id), remote.clone()),
                state::search::get(Some(tile_id), remote.clone()),
            )
            .await;
            let base_path = get_base_path.as_ref().ok().cloned();
            let side_view = if let Ok(side_view) = get_side_view {
                debug!("Setting side_view to {side_view:?}");
                let base_path = base_path
                    .clone()
                    .unwrap_or_else(|| this.path.base.get_value_untracked());
                match fsio::client::prune_side_view(
                    this.remote.clone(),
                    base_path.clone(),
                    side_view.clone(),
                )
                .await
                {
                    Ok(pruned_side_view) => {
                        debug!("Pruned stale side_view entries: {pruned_side_view:?}");
                        Some((base_path, pruned_side_view))
                    }
                    Err(error) => {
                        warn!("Failed to prune stale side_view entries: {error}");
                        None
                    }
                }
            } else {
                None
            };
            let batch = Batch::use_batch(Self::RESTORE_PATHS);
            if let Some(p) = base_path {
                this.path.base.force(p);
            }
            if let Ok(p) = get_file_path {
                this.path.file.force(p);
            }
            if let Some((base_path, side_view)) = side_view {
                this.side_view
                    .force(this.live_side_view(&base_path, side_view));
            }
            this.force_edit_path.set(
                this.path.base.get_value_untracked().iter().next().is_none()
                    || this.path.file.get_value_untracked().iter().next().is_none(),
            );
            if let Ok(p) = get_search {
                this.search.query.force(p);
            }

            drop(batch);
            drop(registrations);
        });
    }

    #[autoclone]
    fn make_file_async_view(self: &Ptr<Self>) -> Consumers {
        let this = self;
        this.path.file.add_subscriber(move |file_path| {
            autoclone!(this);
            let _span = debug_span!("File path changed").entered();
            let end = guard((), |()| debug!("End"));
            debug!("Start");
            let loading = SynchronizedState::enqueue(this.synchronized_state.clone());
            let task = async move {
                autoclone!(this);
                let path = FilePath {
                    base: this.path.base.get_value_untracked(),
                    file: file_path,
                };
                let data = fsio::client::load_file(this.remote.clone(), path.clone())
                    .await
                    .unwrap_or_else(|error| Some(fsio::File::Error(error.to_string())))
                    .map(Arc::new);
                let cursor_position = match data.as_deref() {
                    Some(fsio::File::TextFile { .. }) => {
                        fsio::client::load_cursor_position(this.remote.clone(), path.clone())
                            .await
                            .unwrap_or_else(|error| {
                                warn!("Failed to load cursor position: {error}");
                                None
                            })
                    }
                    _ => None,
                };

                if this.path.as_ref().map(|s| s.get_value_untracked()) != path {
                    debug!("Ignoring stale file load for {:?}", path);
                    drop(loading);
                    return;
                }

                if let Some(
                    fsio::File::TextFile { metadata, .. } | fsio::File::PdfFile { metadata, .. },
                ) = data.as_deref()
                {
                    let metadata = metadata.clone();
                    this.add_to_side_view(&path, |_| {
                        Some(SideViewNode {
                            properties: SvnProperties {
                                status: SvnStatus::Active,
                            },
                            item: SvnItem::File { metadata },
                        })
                    });
                }

                if let Some(data) = data {
                    this.editor_state.force(EditorState::Data(EditorDataState {
                        path,
                        data,
                        cursor_position,
                    }))
                } else {
                    this.editor_state.force(EditorState::default());
                }
                drop(loading);
                drop(end);
            };
            spawn_local(task.in_current_span());
        })
    }

    #[autoclone]
    fn save_side_view_on_change(&self) -> Consumers {
        let tile_id = self.tile.id;
        let remote = self.remote.clone();
        self.side_view.add_subscriber(move |side_view| {
            spawn_local(async move {
                autoclone!(remote);
                let side_view = Self::stored_side_view(side_view);
                let () = state::side_view::set(Some(tile_id), remote, side_view)
                    .await
                    .unwrap_or_else(|error| warn!("Failed to save: {error}"));
            })
        })
    }

    #[autoclone]
    fn save_on_change<T>(
        &self,
        path: XSignal<Arc<T>>,
        setter: impl AsyncFn(Option<TileId>, Remote, Arc<T>) -> Result<(), ServerFnError>
        + Copy
        + 'static,
    ) -> Consumers
    where
        T: ?Sized + 'static,
    {
        let tile_id = self.tile.id;
        let remote = self.remote.clone();
        path.add_subscriber(move |p| {
            spawn_local(async move {
                autoclone!(remote);
                let () = setter(Some(tile_id), remote, p)
                    .await
                    .unwrap_or_else(|error| warn!("Failed to save: {error}"));
            })
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RemoveBehavior {
    Hard,
    Soft,
}
