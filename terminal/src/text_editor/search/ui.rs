use std::collections::HashMap;
use std::future::ready;
use std::ops::Not;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::Stream;
use futures::StreamExt as _;
use futures::channel::oneshot;
use futures::channel::oneshot::Canceled;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::element_capture::ElementCapture;
use terrazzo::widgets::sleep::sleep;
use wasm_bindgen_futures::spawn_local;
use web_sys::FocusEvent;
use web_sys::HtmlInputElement;
use web_sys::KeyboardEvent;

use self::diagnostics::warn;
use super::state::EditorSearchState;
use crate::api::client_address::ClientAddress;
use crate::assets::icons;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::manager::EditorState;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::notify::manager::SideViewNotify as _;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::text_editor::side::SvnProperties;
use crate::text_editor::side::SvnStatus;
use crate::text_editor::side::opaque::OpaqueNotifyRegistration;
use crate::text_editor::style;

impl TextEditorManager {
    #[autoclone]
    #[html]
    pub fn search_selector(self: &Ptr<Self>) -> XElement {
        let is_active = self.search.is_active.clone();
        let input: ElementCapture<HtmlInputElement> = ElementCapture::default();
        let manager = self.clone();

        return div(
            class = style::PATH_SELECTOR,
            style::flex_basis %= flex_basis(is_active.clone()),
            img(
                class = style::PATH_SELECTOR_ICON,
                class = style::SEARCH_ICON,
                src = icons::search(),
                click = move |_| {
                    autoclone!(manager, is_active, input);
                    if is_active.get_value_untracked() {
                        close_search(&manager);
                        return;
                    }
                    is_active.set(true);
                    let () = input.with(|i| i.focus()).or_throw("focus");
                },
            ),
            search_selector_input(self.clone(), input, self.path.base.clone(), is_active),
        );

        #[template(wrap = true)]
        pub fn flex_basis(#[signal] is_active: bool) -> XAttributeValue {
            is_active.not().then_some("0")
        }
    }
}

#[autoclone]
#[html]
#[template(tag = div)]
fn search_selector_input(
    manager: Ptr<TextEditorManager>,
    input: ElementCapture<HtmlInputElement>,
    #[signal] base: Arc<Path>,
    #[signal] is_active: bool,
) -> XElement {
    if !is_active {
        return tag(style::display = "none", style::visibility = "hidden");
    }
    let do_search = Ptr::new(do_search(manager.clone(), base, input.clone()));
    tag(
        class = style::PATH_SELECTOR_WIDGET,
        class = style::PATH_SELECTOR_INPUT,
        key = "search",
        input(
            before_render = input.capture(),
            r#type = "text",
            class = style::PATH_SELECTOR_FIELD,
            keydown = move |event: KeyboardEvent| {
                autoclone!(manager, input, do_search);
                if event.key() == "Escape" {
                    event.prevent_default();
                    close_search(&manager);
                    let () = input.with(|i| i.blur()).or_throw("blur");
                    return;
                }
                do_search()
            },
            focus = move |_: FocusEvent| start_search(&manager, &do_search),
        ),
    )
}

fn start_search(manager: &Ptr<TextEditorManager>, do_search: &Ptr<impl Fn()>) {
    let batch = Batch::use_batch("start-search");
    let mut started = false;
    manager.editor_state.update(|editor_state| {
        if let EditorState::Search { .. } = editor_state {
            return None;
        }
        manager.search.prev_side_view.update(|prev_side_view| {
            if prev_side_view.is_some() {
                return None;
            }
            Some(manager.side_view.get_value_untracked())
        });
        started = true;
        Some(EditorState::Search(EditorSearchState {
            prev: Box::new(editor_state.clone()),
            results: Default::default(),
        }))
    });
    if started {
        manager.side_view.force(Some(search_side_view(
            manager,
            &manager.path.base.get_value_untracked(),
            &[],
        )));
    }
    drop(batch);
    do_search()
}

fn close_search(manager: &TextEditorManager) {
    manager.editor_state.update(|editor_state| {
        let EditorState::Search(EditorSearchState { prev, .. }) = editor_state else {
            return None;
        };
        Some(prev.as_ref().clone())
    });
    if let Some(prev_side_view) = manager.search.prev_side_view.get_value_untracked() {
        manager.side_view.force(prev_side_view);
        manager.search.prev_side_view.force(None);
    }
    manager.search.is_active.set(false);
}

fn do_search(
    manager: Ptr<TextEditorManager>,
    base: Arc<Path>,
    input: ElementCapture<HtmlInputElement>,
) -> impl Fn() {
    let cancel_last = Mutex::new(oneshot::channel().0);
    let callback = move |cancel_rx| {
        #[derive(Debug)]
        enum CancelStatus {
            Timeout,
            TimeoutError,
            Canceled,
            Dropped,
        }
        let search = Box::pin(do_search_impl(manager.clone(), base.clone(), input.clone()));
        let cancel_rx = Box::pin(async move {
            match futures::future::select(cancel_rx, Box::pin(sleep(Duration::from_secs(10)))).await
            {
                futures::future::Either::Left((Ok(()), _sleep)) => CancelStatus::Canceled,
                futures::future::Either::Left((Err(Canceled), _sleep)) => CancelStatus::Dropped,
                futures::future::Either::Right((Ok(()), _)) => CancelStatus::Timeout,
                futures::future::Either::Right((Err(_sleep), _)) => CancelStatus::TimeoutError,
            }
        });
        async move {
            match futures::future::select(search, cancel_rx).await {
                futures::future::Either::Left(((), _cancel_rx)) => {}
                futures::future::Either::Right((cancel, _search)) => {
                    warn!("Search canceled: {cancel:?}")
                }
            }
        }
    };
    move || {
        let cancel_rx = {
            let mut lock = cancel_last.lock().unwrap();
            let (cancel_new_tx, cancel_new_rx) = oneshot::channel();
            let cancel_last = std::mem::replace(&mut *lock, cancel_new_tx);
            let _ = cancel_last.send(());
            cancel_new_rx
        };
        spawn_local(callback(cancel_rx))
    }
}

async fn do_search_impl(
    manager: Ptr<TextEditorManager>,
    base: Arc<Path>,
    input: ElementCapture<HtmlInputElement>,
) {
    let Ok(()) = sleep(Duration::from_millis(250))
        .await
        .inspect_err(|error| warn!("Sleep failed: {error}"))
    else {
        return;
    };
    let input = input.with(|input| input.value());
    manager.search.query.force(input.clone());
    let mut results = run_query(manager.remote.clone(), base.clone(), input).await;
    while let Some(results) = results.next().await {
        let side_view = search_side_view(&manager, &base, &results);
        let batch = Batch::use_batch("update-search-results");
        manager.editor_state.update_mut(move |editor_state| {
            let EditorState::Search(search_state) = editor_state else {
                return std::mem::take(editor_state);
            };
            search_state.results = results.into();
            std::mem::take(editor_state)
        });
        if matches!(
            manager.editor_state.get_value_untracked(),
            EditorState::Search(_)
        ) {
            manager.side_view.force(Some(side_view));
        }
        drop(batch);
    }
}

#[derive(Default)]
struct SearchTreeNode {
    children: HashMap<Arc<Path>, SearchTreeNode>,
    metadata: Option<Arc<FileMetadata>>,
}

fn search_side_view(
    manager: &Ptr<TextEditorManager>,
    base: &Arc<Path>,
    results: &[FileMetadata],
) -> Arc<SideViewNode> {
    let mut root = SearchTreeNode::default();
    for metadata in results {
        let path = Path::new(metadata.name.as_ref());
        let mut node = &mut root;
        for component in path.iter() {
            node = node
                .children
                .entry(Arc::from(Path::new(component)))
                .or_default();
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        node.metadata = Some(Arc::new(FileMetadata {
            name: name.to_string_lossy().into_owned().into(),
            ..metadata.clone()
        }));
    }

    let root_path = crate::text_editor::file_path::FilePath {
        base: base.clone(),
        file: Arc::from(PathBuf::new()),
    };
    let notify = manager.watch_side_view_folder(&root_path);
    Arc::new(search_tree_node(root, notify))
}

fn search_tree_node(node: SearchTreeNode, notify: OpaqueNotifyRegistration) -> SideViewNode {
    if let Some(metadata) = node.metadata
        && node.children.is_empty()
    {
        return SideViewNode {
            properties: SvnProperties {
                status: SvnStatus::Active,
            },
            item: SvnItem::File { metadata },
        };
    }
    SideViewNode {
        properties: SvnProperties {
            status: SvnStatus::Active,
        },
        item: SvnItem::Folder {
            folder: Arc::new(
                node.children
                    .into_iter()
                    .map(|(name, child)| (name, Arc::new(search_tree_node(child, notify.clone()))))
                    .collect::<SideViewList>(),
            ),
            notify,
        },
    }
}

async fn run_query(
    remote: ClientAddress,
    base: Arc<Path>,
    input: String,
) -> impl Stream<Item = Vec<FileMetadata>> {
    let stream = match super::client::search(remote, base, input).await {
        Ok(stream) => stream.left_stream(),
        Err(error) => futures::stream::once(ready(Err(error))).right_stream(),
    };
    let mut accu = vec![];
    stream.ready_chunks(100).map(move |items| {
        for item in items {
            accu.push(item.unwrap_or_else(failed_file_metadata));
        }
        accu.clone()
    })
}

fn failed_file_metadata(error: impl ToString) -> FileMetadata {
    FileMetadata {
        name: error.to_string().into(),
        ..FileMetadata::default()
    }
}
