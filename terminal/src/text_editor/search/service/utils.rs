use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use futures::Stream;
use futures::StreamExt as _;
use quick_cache::UnitWeighter;
use quick_cache::sync::Cache;
use quick_cache::sync::EntryAction;
use quick_cache::sync::EntryResult;
use tokio::io::AsyncBufReadExt as _;
use tokio::io::BufReader;
use tokio_stream::wrappers::LinesStream;
use tracing::warn;

use super::SearchError;

const REFRESH_AFTER: Duration = Duration::from_secs(60);
const EXPIRE_AFTER: Duration = Duration::from_secs(60 * 60);

type CacheKey = (PathBuf, PathBuf);

static GIT_FILES_CACHE: LazyLock<Cache<CacheKey, Arc<CacheEntry>>> =
    LazyLock::new(|| Cache::with_weighter(64, u64::MAX, UnitWeighter));
static GIT_FILES_CACHE_CLEANUP: LazyLock<()> = LazyLock::new(|| {
    tokio::spawn(async {
        loop {
            tokio::time::sleep(EXPIRE_AFTER).await;
            GIT_FILES_CACHE.retain(|_, entry| entry.updated.elapsed() < EXPIRE_AFTER);
        }
    });
});

struct CacheEntry {
    lines: Arc<Vec<CachedLine>>,
    updated: Instant,
    refreshing: AtomicBool,
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
    LazyLock::force(&GIT_FILES_CACHE_CLEANUP);
    let key = (repo_root.to_path_buf(), base.to_path_buf());
    let result = GIT_FILES_CACHE
        .entry_async(&key, |_, entry| {
            if entry.updated.elapsed() >= EXPIRE_AFTER {
                EntryAction::ReplaceWithGuard
            } else {
                EntryAction::Retain(entry.clone())
            }
        })
        .await;
    let guard = match result {
        EntryResult::Retained(entry) => {
            if entry.updated.elapsed() >= REFRESH_AFTER
                && entry
                    .refreshing
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
            {
                spawn_refresh(key, entry.clone(), repo_root.clone(), pathspec.clone());
            }
            return Ok(entry.lines.clone());
        }
        EntryResult::Replaced(guard, _) | EntryResult::Vacant(guard) => guard,
        EntryResult::Removed(_, _) | EntryResult::Timeout => unreachable!(),
    };

    let lines = Arc::new(load_git_files(&repo_root, &pathspec).await?);
    let entry = Arc::new(CacheEntry {
        lines: lines.clone(),
        updated: Instant::now(),
        refreshing: AtomicBool::new(false),
    });
    let _ = guard.insert(entry);
    Ok(lines)
}

fn spawn_refresh(key: CacheKey, entry: Arc<CacheEntry>, repo_root: Arc<Path>, pathspec: PathBuf) {
    tokio::spawn(async move {
        match load_git_files(&repo_root, &pathspec).await {
            Ok(lines) => {
                let refreshed = Arc::new(CacheEntry {
                    lines: Arc::new(lines),
                    updated: Instant::now(),
                    refreshing: AtomicBool::new(false),
                });
                let _ = GIT_FILES_CACHE.entry(&key, Some(Duration::ZERO), |_, current| {
                    if Arc::ptr_eq(current, &entry) {
                        *current = refreshed;
                    }
                    EntryAction::Retain(())
                });
            }
            Err(error) => {
                warn!(%error, ?repo_root, ?pathspec, "Failed to refresh git files cache");
            }
        }
        entry.refreshing.store(false, Ordering::Release);
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
