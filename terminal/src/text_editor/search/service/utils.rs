use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use futures::Stream;
use futures::StreamExt as _;
use tokio::io::AsyncBufReadExt as _;
use tokio::io::BufReader;
use tokio_stream::wrappers::LinesStream;
use tracing::warn;

use super::SearchError;

const REFRESH_AFTER: Duration = Duration::from_secs(60);
const EXPIRE_AFTER: Duration = Duration::from_secs(60 * 60);

type CacheKey = (PathBuf, PathBuf);

static GIT_FILES_CACHE: LazyLock<Mutex<HashMap<CacheKey, Arc<CacheEntry>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct CacheEntry {
    state: Mutex<CacheState>,
    changed: tokio::sync::Notify,
}

struct CacheState {
    data: Option<Arc<Vec<CachedLine>>>,
    updated: Instant,
    refreshing: bool,
}

#[derive(Clone)]
enum CachedLine {
    Line(String),
    Error(ErrorKind, String),
}

impl CachedLine {
    fn into_result(self) -> std::io::Result<String> {
        match self {
            Self::Line(line) => Ok(line),
            Self::Error(kind, message) => Err(std::io::Error::new(kind, message)),
        }
    }
}

pub async fn git_files(
    repo_root: Arc<Path>,
    base: Arc<Path>,
) -> Result<impl Stream<Item = Result<PathBuf, SearchError>>, SearchError> {
    let pathspec = base
        .strip_prefix(&repo_root)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let pathspec = if pathspec.as_os_str().is_empty() {
        Path::new(".")
    } else {
        pathspec
    }
    .to_owned();

    let lines = cached_git_files(repo_root, base, pathspec.clone()).await?;
    let stream =
        futures::stream::iter(0..lines.len()).map(move |index| lines[index].clone().into_result());
    let stream = stream.map(|line| line.map_err(SearchError::GitLsFilesError));
    let stream = stream.map(move |line| {
        if pathspec == Path::new(".") {
            return Ok(PathBuf::from(line?));
        }
        Ok(PathBuf::from(line?)
            .strip_prefix(&pathspec)
            .map_err(SearchError::InvalidRepoRootPrefix)?
            .to_owned())
    });
    #[cfg(debug_assertions)]
    let stream = {
        let mut i = 0;
        stream.flat_map(move |row| {
            i += 1;
            futures::stream::once(async move {
                if i % 100 == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                row
            })
        })
    };
    #[cfg(debug_assertions)]
    let stream = stream.inspect(|row| tracing::trace!("Row: {row:?}"));
    Ok(stream)
}

async fn cached_git_files(
    repo_root: Arc<Path>,
    base: Arc<Path>,
    pathspec: PathBuf,
) -> Result<Arc<Vec<CachedLine>>, SearchError> {
    let key = (repo_root.to_path_buf(), base.to_path_buf());

    loop {
        let (entry, load) = {
            let mut cache = GIT_FILES_CACHE.lock().unwrap();
            if let Some(entry) = cache.get(&key) {
                let state = entry.state.lock().unwrap();
                let expired = state
                    .data
                    .as_ref()
                    .is_some_and(|_| state.updated.elapsed() >= EXPIRE_AFTER);
                if !expired || state.refreshing {
                    (entry.clone(), false)
                } else {
                    drop(state);
                    let entry = new_loading_entry();
                    cache.insert(key.clone(), entry.clone());
                    (entry, true)
                }
            } else {
                let entry = new_loading_entry();
                cache.insert(key.clone(), entry.clone());
                (entry, true)
            }
        };

        if load {
            let result = load_git_files(&repo_root, &pathspec).await;
            match result {
                Ok(lines) => {
                    let lines = Arc::new(lines);
                    let mut state = entry.state.lock().unwrap();
                    state.data = Some(lines.clone());
                    state.updated = Instant::now();
                    state.refreshing = false;
                    drop(state);
                    entry.changed.notify_waiters();
                    return Ok(lines);
                }
                Err(error) => {
                    entry.state.lock().unwrap().refreshing = false;
                    entry.changed.notify_waiters();
                    return Err(error);
                }
            }
        }

        let changed = entry.changed.notified();
        let (cached, should_load) = {
            let mut state = entry.state.lock().unwrap();
            if let Some(lines) = state.data.clone() {
                let should_refresh = state.updated.elapsed() >= REFRESH_AFTER && !state.refreshing;
                if should_refresh {
                    state.refreshing = true;
                }
                (Some((lines, should_refresh)), false)
            } else if !state.refreshing {
                state.refreshing = true;
                (None, true)
            } else {
                (None, false)
            }
        };
        if let Some((lines, should_refresh)) = cached {
            if should_refresh {
                spawn_refresh(entry.clone(), repo_root.clone(), pathspec.clone());
            }
            return Ok(lines);
        }
        if should_load {
            let result = load_git_files(&repo_root, &pathspec).await;
            match result {
                Ok(lines) => {
                    let lines = Arc::new(lines);
                    let mut state = entry.state.lock().unwrap();
                    state.data = Some(lines.clone());
                    state.updated = Instant::now();
                    state.refreshing = false;
                    drop(state);
                    entry.changed.notify_waiters();
                    return Ok(lines);
                }
                Err(error) => {
                    entry.state.lock().unwrap().refreshing = false;
                    entry.changed.notify_waiters();
                    return Err(error);
                }
            }
        }
        changed.await;
    }
}

fn new_loading_entry() -> Arc<CacheEntry> {
    Arc::new(CacheEntry {
        state: Mutex::new(CacheState {
            data: None,
            updated: Instant::now(),
            refreshing: true,
        }),
        changed: tokio::sync::Notify::new(),
    })
}

fn spawn_refresh(entry: Arc<CacheEntry>, repo_root: Arc<Path>, pathspec: PathBuf) {
    tokio::spawn(async move {
        match load_git_files(&repo_root, &pathspec).await {
            Ok(lines) => {
                let mut state = entry.state.lock().unwrap();
                state.data = Some(Arc::new(lines));
                state.updated = Instant::now();
                state.refreshing = false;
            }
            Err(error) => {
                entry.state.lock().unwrap().refreshing = false;
                warn!(%error, ?repo_root, ?pathspec, "Failed to refresh git files cache");
            }
        }
        entry.changed.notify_waiters();
    });
}

async fn load_git_files(repo_root: &Path, pathspec: &Path) -> Result<Vec<CachedLine>, SearchError> {
    let process = tokio::process::Command::new("git")
        .current_dir(repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(pathspec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(SearchError::GitLsFilesError)?;
    let stdout = process.stdout.ok_or_else(|| {
        SearchError::GitLsFilesError(std::io::Error::new(
            ErrorKind::BrokenPipe,
            "git ls-files didn't return anything",
        ))
    })?;

    let lines = BufReader::new(stdout).lines();
    Ok(LinesStream::new(lines)
        .map(|line| match line {
            Ok(line) => CachedLine::Line(line),
            Err(error) => CachedLine::Error(error.kind(), error.to_string()),
        })
        .collect()
        .await)
}
