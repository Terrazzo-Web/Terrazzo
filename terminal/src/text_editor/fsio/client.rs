#![cfg(feature = "client")]

use std::mem;
use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use futures::channel::oneshot;
use server_fn::ServerFnError;
use terrazzo::prelude::diagnostics;
use terrazzo::widgets::sleep::sleep;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::warn;
use crate::frontend::remotes::Remote;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::ui::STORE_FILE_DEBOUNCE_DELAY;

pub async fn load_file(
    remote: Remote,
    path: FilePath<Arc<Path>>,
) -> Result<Option<super::File>, ServerFnError> {
    super::load_file(remote, path).await
}

pub async fn load_file_metadata(
    remote: Remote,
    path: FilePath<Arc<Path>>,
) -> Result<Option<super::File>, ServerFnError> {
    super::load_file_metadata(remote, path).await
}

pub async fn list_folder(
    remote: Remote,
    path: FilePath<Arc<Path>>,
) -> Result<Option<Arc<Vec<super::FileMetadata>>>, ServerFnError> {
    super::list_folder(remote, path).await
}

pub async fn file_exists(remote: Remote, path: FilePath<Arc<Path>>) -> Result<bool, ServerFnError> {
    super::file_exists(remote, path).await
}

pub async fn prune_side_view(
    remote: Remote,
    base: Arc<Path>,
    side_view: Option<Arc<SideViewNode<()>>>,
) -> Result<Option<Arc<SideViewNode<()>>>, ServerFnError> {
    let Some(side_view) = side_view else {
        return Ok(None);
    };
    super::prune_side_view(remote, base, side_view).await
}

pub async fn create_file(
    remote: Remote,
    path: FilePath<Arc<Path>>,
    name: String,
) -> Result<(), ServerFnError> {
    super::create_file(remote, path, name).await
}

pub async fn create_folder(
    remote: Remote,
    path: FilePath<Arc<Path>>,
    name: String,
) -> Result<(), ServerFnError> {
    super::create_folder(remote, path, name).await
}

pub async fn move_file(
    remote: Remote,
    source: FilePath<Arc<Path>>,
    destination_folder: FilePath<Arc<Path>>,
) -> Result<(), ServerFnError> {
    super::move_file(remote, source, destination_folder).await
}

pub async fn delete_file(remote: Remote, path: FilePath<Arc<Path>>) -> Result<(), ServerFnError> {
    super::delete_file(remote, path).await
}

static STORE_FILE_STATE: LazyLock<Mutex<StoreFileState>> = LazyLock::new(Mutex::default);

pub async fn store_file<B: Send + 'static, A: Send + 'static>(
    remote: Remote,
    path: FilePath<Arc<Path>>,
    content: String,
    before: B,
    after: A,
) {
    assert!(std::mem::needs_drop::<B>());
    assert!(std::mem::needs_drop::<A>());
    let (done_tx, done_rx) = oneshot::channel();
    let schedule = {
        let mut state = STORE_FILE_STATE.lock().expect("store_file_state");
        let waiter = StoreFileWaiter {
            before: Box::new(before),
            after: Box::new(after),
            done: done_tx,
        };
        if let Some(pending) = state
            .pending
            .iter_mut()
            .find(|pending| pending.remote == remote && pending.path == path)
        {
            pending.content = content;
            pending.waiters.push(waiter);
        } else {
            state.pending.push(PendingStoreFile {
                remote,
                path,
                content,
                waiters: vec![waiter],
            });
        }
        let schedule = !state.scheduled;
        state.scheduled = true;
        schedule
    };
    if schedule {
        spawn_local(flush_pending_store_files());
    }
    let _ = done_rx.await;
}

async fn flush_pending_store_files() {
    if let Err(error) = sleep(STORE_FILE_DEBOUNCE_DELAY).await {
        warn!("Failed to wait before storing files: {error}");
    }
    let pending = {
        let mut state = STORE_FILE_STATE.lock().expect("store_file_state");
        state.scheduled = false;
        mem::take(&mut state.pending)
    };
    for PendingStoreFile {
        remote,
        path,
        content,
        waiters,
    } in pending
    {
        let (before, after): (Vec<_>, Vec<_>) = waiters
            .into_iter()
            .map(
                |StoreFileWaiter {
                     before,
                     after,
                     done,
                 }| {
                    let after = (after, done);
                    (before, after)
                },
            )
            .unzip();
        drop(before);
        let () = super::store_file_impl(remote, path, content)
            .await
            .unwrap_or_else(|error| warn!("Failed to store file: {error}"));
        for (after, done) in after {
            drop(after);
            let _ = done.send(());
        }
    }
}

#[derive(Default)]
struct StoreFileState {
    pending: Vec<PendingStoreFile>,
    scheduled: bool,
}

struct PendingStoreFile {
    remote: Remote,
    path: FilePath<Arc<Path>>,
    content: String,
    waiters: Vec<StoreFileWaiter>,
}

struct StoreFileWaiter {
    before: Box<dyn Send>,
    after: Box<dyn Send>,
    done: oneshot::Sender<()>,
}
