use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use lru::LruCache;
use tokio::process::Command;

const LRU_CACHE_SIZE: NonZero<usize> = NonZero::new(10_000).expect("LRU_CACHE_SIZE");

static GIT_REPOS: LazyLock<GitReposCache<TokioFs>> = LazyLock::new(|| GitReposCache::new(TokioFs));

pub async fn git_repo_root(path: impl AsRef<Path>) -> Option<Arc<Path>> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return None;
    }
    GIT_REPOS.git_repo_root(path).await
}

pub async fn file_content_at_commit(
    path: impl AsRef<Path>,
    commit: &str,
) -> std::io::Result<String> {
    let path = tokio::fs::canonicalize(path).await?;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    let repo_root = git_output(parent, ["rev-parse", "--show-toplevel"]).await?;
    let repo_root = PathBuf::from(repo_root.trim_end());
    let relative_path = path.strip_prefix(&repo_root).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "path {} is not under git repo {}: {error}",
                path.display(),
                repo_root.display()
            ),
        )
    })?;
    let object = format!("{}:{}", commit, relative_path.display());
    git_output(&repo_root, ["show", object.as_str()]).await
}

async fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> std::io::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .kill_on_drop(true)
        .output()
        .await?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

trait GitRepoFs {
    fn is_dir(&self, path: &Path) -> impl Future<Output = bool> + Send;
}

struct TokioFs;

impl GitRepoFs for TokioFs {
    async fn is_dir(&self, path: &Path) -> bool {
        tokio::fs::metadata(path)
            .await
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
    }
}

// TODO: add an expiration timeout
struct GitReposCache<F> {
    fs: F,
    cache: Mutex<LruCache<PathBuf, Option<Arc<Path>>>>,
}

impl<F> GitReposCache<F> {
    fn new(fs: F) -> Self {
        Self {
            fs,
            cache: Mutex::new(LruCache::new(LRU_CACHE_SIZE)),
        }
    }
}

impl<F: GitRepoFs> GitReposCache<F> {
    async fn git_repo_root(&self, path: impl AsRef<Path>) -> Option<Arc<Path>> {
        let mut backfill = vec![];
        let mut result = None;
        for ancestor in path.as_ref().ancestors() {
            if let Some(root) = self.maybe_git_repo_root(ancestor, &mut backfill).await {
                result = root;
                break;
            }
            tokio::task::yield_now().await;
        }
        let mut cache = self.cache.lock().unwrap();
        for ancestor in backfill {
            cache.push(ancestor, result.clone());
        }
        result
    }

    async fn maybe_git_repo_root(
        &self,
        path: &Path,
        backfill: &mut Vec<PathBuf>,
    ) -> Option<Option<Arc<Path>>> {
        let cached = self.cache.lock().unwrap().get(path).cloned();
        if let Some(root) = cached {
            return Some(root);
        }
        backfill.push(path.to_owned());
        self.fs
            .is_dir(&path.join(".git"))
            .await
            .then(|| Some(Arc::from(path.to_owned())))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[derive(Default)]
    struct MockFs {
        dirs: HashSet<PathBuf>,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl MockFs {
        fn with_dir(mut self, path: impl Into<PathBuf>) -> Self {
            self.dirs.insert(path.into());
            self
        }
    }

    impl GitRepoFs for MockFs {
        async fn is_dir(&self, path: &Path) -> bool {
            self.calls.lock().unwrap().push(path.to_owned());
            self.dirs.contains(path)
        }
    }

    #[tokio::test]
    async fn empty_path_has_no_repo_root() {
        assert!(git_repo_root("").await.is_none());
    }

    #[tokio::test]
    async fn file_uses_cached_repo_root() {
        let fs = MockFs::default()
            .with_dir("/repo")
            .with_dir("/repo/src")
            .with_dir("/repo/.git");
        let cache = GitReposCache::new(fs);

        assert_eq!(
            Path::new("/repo"),
            cache.git_repo_root("/repo/src").await.unwrap().as_ref()
        );
        assert_eq!(
            &["/repo/src/.git", "/repo/.git"].map(PathBuf::from),
            cache.fs.calls.lock().unwrap().as_slice()
        );
        cache.fs.calls.lock().unwrap().clear();

        assert_eq!(
            Path::new("/repo"),
            cache
                .git_repo_root("/repo/src/main.rs")
                .await
                .unwrap()
                .as_ref()
        );
        assert_eq!(
            &["/repo/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.lock().unwrap().as_slice()
        );
        cache.fs.calls.lock().unwrap().clear();

        assert_eq!(
            Path::new("/repo"),
            cache
                .git_repo_root("/repo/src/main.rs")
                .await
                .unwrap()
                .as_ref()
        );
        assert!(cache.fs.calls.lock().unwrap().is_empty());
        cache.fs.calls.lock().unwrap().clear();
    }

    #[tokio::test]
    async fn caches_negative_results() {
        let fs = MockFs::default()
            .with_dir("/workspace")
            .with_dir("/workspace/src");
        let cache = GitReposCache::new(fs);

        assert!(cache.git_repo_root("/workspace/src/lib.rs").await.is_none());
        assert_eq!(
            &[
                "/workspace/src/lib.rs/.git",
                "/workspace/src/.git",
                "/workspace/.git",
                "/.git"
            ]
            .map(PathBuf::from),
            cache.fs.calls.lock().unwrap().as_slice()
        );
        cache.fs.calls.lock().unwrap().clear();

        assert!(
            cache
                .git_repo_root("/workspace/src/main.rs")
                .await
                .is_none()
        );
        assert_eq!(
            &["/workspace/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.lock().unwrap().as_slice()
        );
        cache.fs.calls.lock().unwrap().clear();

        assert!(
            cache
                .git_repo_root("/workspace/src/main.rs")
                .await
                .is_none()
        );
        assert!(cache.fs.calls.lock().unwrap().is_empty());
        cache.fs.calls.lock().unwrap().clear();
    }

    #[tokio::test]
    async fn reads_file_content_at_git_commit() {
        let tempdir = tempfile::tempdir().unwrap();
        let repo = tempdir.path();
        let file = repo.join("dummy.txt");

        git(repo, &["init"]).await;
        git(repo, &["config", "user.email", "test@example.com"]).await;
        git(repo, &["config", "user.name", "Test User"]).await;

        tokio::fs::write(&file, "init").await.unwrap();
        commit_file(repo, "init").await;

        tokio::fs::write(&file, "commit 1").await.unwrap();
        commit_file(repo, "commit 1").await;
        let commit1 = git(repo, &["rev-parse", "HEAD"]).await;

        tokio::fs::write(&file, "commit 2").await.unwrap();
        commit_file(repo, "commit 2").await;
        let commit2 = git(repo, &["rev-parse", "HEAD"]).await;

        git(repo, &["checkout", "-b", "test_branch"]).await;
        tokio::fs::write(&file, "branched").await.unwrap();
        commit_file(repo, "branched").await;

        git(repo, &["checkout", "-b", "test_branch2"]).await;
        tokio::fs::write(&file, "tagged").await.unwrap();
        commit_file(repo, "tagged").await;
        git(repo, &["tag", "test_tag"]).await;

        tokio::fs::write(&file, "final").await.unwrap();
        commit_file(repo, "final").await;

        tokio::fs::write(&file, "current").await.unwrap();

        assert_eq!(
            "final",
            file_content_at_commit(&file, "HEAD").await.unwrap()
        );
        assert_eq!(
            "tagged",
            file_content_at_commit(&file, "HEAD^").await.unwrap()
        );
        assert_eq!(
            "branched",
            file_content_at_commit(&file, "test_branch").await.unwrap()
        );
        assert_eq!(
            "tagged",
            file_content_at_commit(&file, "test_tag").await.unwrap()
        );
        assert_eq!(
            "commit 1",
            file_content_at_commit(&file, commit1.trim_end())
                .await
                .unwrap()
        );
        assert_eq!(
            "commit 2",
            file_content_at_commit(&file, commit2.trim_end())
                .await
                .unwrap()
        );
    }

    async fn commit_file(repo: &Path, message: &str) {
        git(repo, &["add", "dummy.txt"]).await;
        git(repo, &["commit", "-m", message]).await;
    }

    async fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }
}
