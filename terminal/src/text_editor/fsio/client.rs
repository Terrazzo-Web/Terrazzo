#![cfg(feature = "client")]

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use futures::FutureExt as _;
use futures::channel::oneshot;
use futures::future::Shared;
use server_fn::ServerFnError;
use terrazzo::prelude::diagnostics;
use terrazzo::widgets::debounce::DoDebounce;

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
    tree: Arc<SideViewNode<()>>,
) -> Result<Option<Arc<SideViewNode<()>>>, ServerFnError> {
    super::prune_side_view(remote, base, tree).await
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

pub async fn delete_file(remote: Remote, path: FilePath<Arc<Path>>) -> Result<(), ServerFnError> {
    super::delete_file(remote, path).await
}

static DEBOUNCED_STORE_FILE_FN: LazyLock<StoreFileFn> = LazyLock::new(make_debounced_store_file_fn);
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
    let debounced_store_file_fn = &*DEBOUNCED_STORE_FILE_FN;
    let done = wait_for_pending_store_file(&remote, &path).await;
    debounced_store_file_fn(StoreFileFnArg {
        remote,
        path,
        content,
        before: Box::new(before),
        after: Box::new(after),
        done,
    })
    .await;
}

async fn wait_for_pending_store_file(
    remote: &Remote,
    path: &FilePath<Arc<Path>>,
) -> oneshot::Sender<()> {
    loop {
        let done = {
            let mut store_file_state = STORE_FILE_STATE.lock().expect("store_file_state");
            match &store_file_state.pending {
                Some(PendingStoreFile {
                    remote: pending_remote,
                    path: pending_path,
                    done,
                }) if pending_remote != remote || pending_path != path => done.clone(),
                _ => {
                    let (done_tx, done_rx) = oneshot::channel();
                    let done: BoxFuture =
                        Box::pin(done_rx.map(|_result: Result<(), oneshot::Canceled>| ()));
                    store_file_state.pending = Some(PendingStoreFile {
                        remote: remote.clone(),
                        path: path.clone(),
                        done: done.shared(),
                    });
                    return done_tx;
                }
            }
        };
        done.await;
    }
}

fn make_debounced_store_file_fn() -> StoreFileFn {
    let debounced = STORE_FILE_DEBOUNCE_DELAY.async_debounce(
        move |StoreFileFnArg {
                  remote,
                  path,
                  content,
                  before,
                  after,
                  done,
              }| async move {
            drop(before);
            let () = super::store_file_impl(remote.clone(), path.clone(), content)
                .await
                .unwrap_or_else(|error| warn!("Failed to store file: {error}"));
            drop(after);
            clear_pending_store_file(&remote, &path);
            let _ = done.send(());
        },
    );
    return Box::new(debounced);
}

fn clear_pending_store_file(remote: &Remote, path: &FilePath<Arc<Path>>) {
    let mut store_file_state = STORE_FILE_STATE.lock().expect("store_file_state");
    if store_file_state
        .pending
        .as_ref()
        .is_some_and(|pending| pending.remote == *remote && pending.path == *path)
    {
        store_file_state.pending = None;
    }
}

type StoreFileFn = Box<dyn Fn(StoreFileFnArg) -> Shared<BoxFuture> + Send + Sync>;
type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + Sync>>;

struct StoreFileFnArg {
    remote: Remote,
    path: FilePath<Arc<Path>>,
    content: String,
    before: Box<dyn Send>,
    after: Box<dyn Send>,
    done: oneshot::Sender<()>,
}

#[derive(Default)]
struct StoreFileState {
    pending: Option<PendingStoreFile>,
}

struct PendingStoreFile {
    remote: Remote,
    path: FilePath<Arc<Path>>,
    done: Shared<BoxFuture>,
}
