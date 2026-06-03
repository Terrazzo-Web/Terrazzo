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

use self::diagnostics::debug;
use self::diagnostics::warn;
use self::editor::EditorDocument;
use self::editor::editor;
use self::folder::folder;
use super::file_path::FilePath;
use super::fsio;
use super::manager::EditorDataState;
use super::manager::EditorState;
use super::manager::TextEditorManager;
use super::notify::ui::NotifyService;
use super::search::state::SearchState;
use super::side::SideViewList;
use super::side::mutation::side_view_notify_registrations;
use super::side::ui::show_side_view;
use super::state;
use super::style;
use super::synchronized_state::SynchronizedState;
use super::synchronized_state::show_synchronized_state;
use crate::assets::icons;
use crate::frontend::menu::menu;
use crate::frontend::mousemove::MousemoveManager;
use crate::frontend::mousemove::Position;
use crate::frontend::remotes::Remote;
use crate::frontend::remotes_ui::show_remote;
use crate::frontend::resize_bar::resize_bar_horz;
use crate::text_editor::search::state::EditorSearchState;
use crate::tiles::app::App;
use crate::tiles::id::TileId;
use crate::tiles::signals::TilePtr;

mod code_mirror;
mod editor;
mod folder;
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
    let side_view: XSignal<Arc<SideViewList>> = XSignal::new("side-view", Default::default());
    let show_editor_diff = XSignal::new("show-editor-diff", true);
    let manager = Ptr::new(TextEditorManager {
        tile,
        remote: remote.clone(),
        path: FilePath {
            base: XSignal::new("base-path", Arc::default()),
            file: XSignal::new("file-path", Arc::default()),
        },
        force_edit_path: XSignal::new("force-edit-path", false),
        editor_state: XSignal::new("editor-state", EditorState::default()),
        synchronized_state: XSignal::new("synchronized-state", SynchronizedState::Sync),
        side_view,
        notify_service: Ptr::new(NotifyService::new(remote)),
        search: SearchState::new(),
    });

    let consumers = Arc::default();
    manager.restore_paths(&consumers);
    let side_view_resize_manager = MousemoveManager::new();

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
            toggle_editor_diff(manager.editor_state.clone(), show_editor_diff.clone()),
            manager.refresh_editor(),
            show_synchronized_state(manager.synchronized_state.clone()),
            show_remote(manager.tile.remote.clone()),
        ),
        editor_body(
            manager.clone(),
            manager.editor_state.clone(),
            show_editor_diff,
            side_view_resize_manager,
        ),
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
fn editor_body(
    manager: Ptr<TextEditorManager>,
    editor_state: XSignal<EditorState>,
    show_editor_diff: XSignal<bool>,
    side_view_resize_manager: MousemoveManager,
) -> XElement {
    div(
        class = super::style::BODY,
        #[cfg(not(feature = "client-prod"))]
        class = "editor-body",
        show_side_view(
            manager.clone(),
            manager.side_view.clone(),
            side_view_resize_manager.clone(),
        ),
        resize_bar_horz(side_view_resize_manager, Default::default()),
        editor_container(manager, editor_state, show_editor_diff),
    )
}

#[html]
#[template(tag = span)]
fn toggle_editor_diff(
    #[signal] editor_state: EditorState,
    show_editor_diff: XSignal<bool>,
) -> XElement {
    let has_diff = match editor_state {
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
    img(
        class = style::TOGGLE_EDITOR_DIFF,
        class %= toggle_editor_diff_class(show_editor_diff.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "toggle-editor-diff",
        src = icons::split_vert(),
        title %= toggle_editor_diff_title(show_editor_diff.clone()),
        click = move |_| show_editor_diff.update(|show| Some(!show)),
    )
}

#[template(wrap = true)]
fn toggle_editor_diff_class(#[signal] show_editor_diff: bool) -> XAttributeValue {
    show_editor_diff.then_some(style::ACTIVE)
}

#[template(wrap = true)]
fn toggle_editor_diff_title(#[signal] show_editor_diff: bool) -> XAttributeValue {
    if show_editor_diff {
        "Hide diff"
    } else {
        "Show diff"
    }
}

#[template(wrap = true)]
pub(super) fn side_view_width(#[signal] position: Option<Position>) -> XAttributeValue {
    let position = position.unwrap_or_default();
    format!("0 0 max(8rem, calc(200px + {}px))", position.x)
}

#[html]
#[template(tag = div)]
fn editor_container(
    manager: Ptr<TextEditorManager>,
    #[signal] editor_state: EditorState,
    show_editor_diff: XSignal<bool>,
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
                editor(manager, editor_state, editor_document, show_editor_diff)
            }
            fsio::File::PdfFile { base64, .. } => {
                let base64 = base64.clone();
                editor(
                    manager,
                    editor_state,
                    EditorDocument::Pdf(base64),
                    show_editor_diff,
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
                    .append(this.save_on_change(this.side_view.clone(), state::side_view::set))
                    .append(this.save_on_change(this.search.query.clone(), state::search::set))
                    .append(this.path.base.add_subscriber(move |_base_path| {
                        autoclone!(this);
                        this.side_view.force(Arc::default());
                        this.path.file.force(Arc::default());
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
            let batch = Batch::use_batch(Self::RESTORE_PATHS);
            if let Ok(p) = get_base_path {
                this.path.base.force(p);
            }
            if let Ok(p) = get_file_path {
                this.path.file.force(p);
            }
            if let Ok(side_view) = get_side_view {
                debug!("Setting side_view to {side_view:?}");
                let base_path = this.path.base.get_value_untracked();
                match fsio::client::prune_side_view(
                    this.remote.clone(),
                    base_path.clone(),
                    side_view.clone(),
                )
                .await
                {
                    Ok(Some(pruned_side_view)) => {
                        debug!("Pruned stale side_view entries: {pruned_side_view:?}");
                        this.side_view.force(side_view_notify_registrations(
                            &this,
                            base_path,
                            pruned_side_view,
                        ));
                    }
                    Ok(None) => {
                        this.side_view
                            .force(side_view_notify_registrations(&this, base_path, side_view));
                    }
                    Err(error) => warn!("Failed to prune stale side_view entries: {error}"),
                }
            }
            this.force_edit_path.set(
                this.path.base.get_value_untracked().is_empty()
                    || this.path.file.get_value_untracked().is_empty(),
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

                if this.path.base.get_value_untracked() != path.base
                    || this.path.file.get_value_untracked() != path.file
                {
                    debug!("Ignoring stale file load for {:?}", path);
                    drop(loading);
                    return;
                }

                if let Some(
                    fsio::File::TextFile { metadata, .. } | fsio::File::PdfFile { metadata, .. },
                ) = data.as_deref()
                {
                    this.add_to_side_view(metadata, &path);
                }

                if let Some(data) = data {
                    this.editor_state
                        .force(EditorState::Data(EditorDataState { path, data }))
                } else {
                    this.editor_state.force(EditorState::default());
                }
                drop(loading);
            };
            spawn_local(task);
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
