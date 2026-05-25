#![cfg(feature = "client")]

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
use crate::text_editor::ui::STORE_FILE_DEBOUNCE_DELAY;

pub async fn load_file(
    remote: Remote,
    path: FilePath<Arc<str>>,
) -> Result<Option<super::File>, ServerFnError> {
    super::load_file(remote, path).await
}

static DEBOUNCED_STORE_FILE_FN: LazyLock<StoreFileFn> = LazyLock::new(make_debounced_store_file_fn);
static STORE_FILE_STATE: LazyLock<Mutex<StoreFileState>> = LazyLock::new(Mutex::default);

pub async fn store_file<B: Send + 'static, A: Send + 'static>(
    remote: Remote,
    path: FilePath<Arc<str>>,
    content: String,
    before: B,
    after: A,
) {
    assert!(std::mem::needs_drop::<B>());
    assert!(std::mem::needs_drop::<A>());
    let debounced_store_file_fn = &*DEBOUNCED_STORE_FILE_FN;
    let store_file_state = &*STORE_FILE_STATE;
    while let Some(done) = pending_different_file(&remote, &path) {
        done.await;
    }

    let (done_tx, done_rx) = oneshot::channel();
    let done = {
        let done: BoxFuture = Box::pin(done_rx.map(|_result: Result<(), oneshot::Canceled>| ()));
        done.shared()
    };
    {
        let mut store_file_state = store_file_state.lock().expect("store_file_state");
        store_file_state.pending = Some(PendingStoreFile {
            remote: remote.clone(),
            path: path.clone(),
            done,
        });
    }
    debounced_store_file_fn(StoreFileFnArg {
        remote,
        path,
        content,
        before: Box::new(before),
        after: Box::new(after),
        done: done_tx,
    })
    .await;
}

fn pending_different_file(remote: &Remote, path: &FilePath<Arc<str>>) -> Option<Shared<BoxFuture>> {
    let store_file_state = STORE_FILE_STATE.lock().expect("store_file_state");
    let Some(PendingStoreFile {
        remote: pending_remote,
        path: pending_path,
        done,
    }) = &store_file_state.pending
    else {
        return None;
    };
    (pending_remote != remote || pending_path != path).then(|| done.clone())
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

fn clear_pending_store_file(remote: &Remote, path: &FilePath<Arc<str>>) {
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
    path: FilePath<Arc<str>>,
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
    path: FilePath<Arc<str>>,
    done: Shared<BoxFuture>,
}
