#![cfg(feature = "server")]

use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use lru::LruCache;

const LRU_CACHE_SIZE: NonZero<usize> = NonZero::new(10_000).expect("LRU_CACHE_SIZE");

static GIT_REPOS: LazyLock<GitReposCache<StdFs>> = LazyLock::new(|| GitReposCache::new(StdFs));

pub fn git_repo_root(path: impl AsRef<Path>) -> Option<Arc<Path>> {
    GIT_REPOS.git_repo_root(path.as_ref())
}

pub fn file_content_at_commit(path: impl AsRef<Path>, commit: &str) -> std::io::Result<String> {
    let path = path.as_ref().canonicalize()?;
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("path has no parent: {}", path.display()),
        )
    })?;
    let repo_root = git_output(parent, ["rev-parse", "--show-toplevel"])?;
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
    git_output(&repo_root, ["show", object.as_str()])
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> std::io::Result<String> {
    let output = Command::new("git").args(args).current_dir(cwd).output()?;
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
    fn is_dir(&self, path: &Path) -> bool;
}

struct StdFs;

impl GitRepoFs for StdFs {
    fn is_dir(&self, path: &Path) -> bool {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
    }
}

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
    fn git_repo_root(&self, path: impl AsRef<Path>) -> Option<Arc<Path>> {
        let mut cache = self.cache.lock().unwrap();

        let ancestors = path.as_ref().ancestors();
        let mut backfill = vec![];
        let mut result = None;
        for ancestor in ancestors {
            if let Some(root) = self.maybe_git_repo_root(ancestor, &mut cache, &mut backfill) {
                result = root;
                break;
            }
        }
        for ancestor in backfill {
            cache.push(ancestor.to_owned(), result.clone());
        }
        return result;
    }

    fn maybe_git_repo_root<'l>(
        &self,
        path: &'l Path,
        cache: &mut LruCache<PathBuf, Option<Arc<Path>>>,
        backfill: &mut Vec<&'l Path>,
    ) -> Option<Option<Arc<Path>>> {
        if let Some(root) = cache.get(path) {
            return Some(root.clone());
        }
        backfill.push(path);
        return self
            .fs
            .is_dir(&path.join(".git"))
            .then(|| Some(Arc::from(path.to_owned())));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::process::Command;

    use super::*;

    #[derive(Default)]
    struct MockFs {
        dirs: HashSet<PathBuf>,
        calls: RefCell<Vec<PathBuf>>,
    }

    impl MockFs {
        fn with_dir(mut self, path: impl Into<PathBuf>) -> Self {
            self.dirs.insert(path.into());
            self
        }
    }

    impl GitRepoFs for MockFs {
        fn is_dir(&self, path: &Path) -> bool {
            self.calls.borrow_mut().push(path.to_owned());
            self.dirs.contains(path)
        }
    }

    #[test]
    fn file_uses_cached_repo_root() {
        let fs = MockFs::default()
            .with_dir("/repo")
            .with_dir("/repo/src")
            .with_dir("/repo/.git");
        let cache = GitReposCache::new(fs);

        assert_eq!(
            Path::new("/repo"),
            cache.git_repo_root("/repo/src").unwrap().as_ref()
        );
        assert_eq!(
            &["/repo/src/.git", "/repo/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert_eq!(
            Path::new("/repo"),
            cache.git_repo_root("/repo/src/main.rs").unwrap().as_ref()
        );
        assert_eq!(
            &["/repo/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert_eq!(
            Path::new("/repo"),
            cache.git_repo_root("/repo/src/main.rs").unwrap().as_ref()
        );
        assert!(cache.fs.calls.borrow().is_empty());
        cache.fs.calls.borrow_mut().clear();
    }

    #[test]
    fn caches_negative_results() {
        let fs = MockFs::default()
            .with_dir("/workspace")
            .with_dir("/workspace/src");
        let cache = GitReposCache::new(fs);

        assert!(cache.git_repo_root("/workspace/src/lib.rs").is_none());
        assert_eq!(
            &[
                "/workspace/src/lib.rs/.git",
                "/workspace/src/.git",
                "/workspace/.git",
                "/.git"
            ]
            .map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert!(cache.git_repo_root("/workspace/src/main.rs").is_none());
        assert_eq!(
            &["/workspace/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert!(cache.git_repo_root("/workspace/src/main.rs").is_none());
        assert!(cache.fs.calls.borrow().is_empty());
        cache.fs.calls.borrow_mut().clear();
    }

    #[test]
    fn reads_file_content_at_git_commit() {
        let tempdir = tempfile::tempdir().unwrap();
        let repo = tempdir.path();
        let file = repo.join("dummy.txt");

        git(repo, &["init"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "Test User"]);

        std::fs::write(&file, "init").unwrap();
        commit_file(repo, "init");

        std::fs::write(&file, "commit 1").unwrap();
        commit_file(repo, "commit 1");
        let commit1 = git(repo, &["rev-parse", "HEAD"]);

        std::fs::write(&file, "commit 2").unwrap();
        commit_file(repo, "commit 2");
        let commit2 = git(repo, &["rev-parse", "HEAD"]);

        git(repo, &["checkout", "-b", "test_branch"]);
        std::fs::write(&file, "branched").unwrap();
        commit_file(repo, "branched");

        git(repo, &["checkout", "-b", "test_branch2"]);
        std::fs::write(&file, "tagged").unwrap();
        commit_file(repo, "tagged");
        git(repo, &["tag", "test_tag"]);

        std::fs::write(&file, "final").unwrap();
        commit_file(repo, "final");

        std::fs::write(&file, "current").unwrap();

        assert_eq!("final", file_content_at_commit(&file, "HEAD").unwrap());
        assert_eq!("tagged", file_content_at_commit(&file, "HEAD^").unwrap());
        assert_eq!(
            "branched",
            file_content_at_commit(&file, "test_branch").unwrap()
        );
        assert_eq!("tagged", file_content_at_commit(&file, "test_tag").unwrap());
        assert_eq!(
            "commit 1",
            file_content_at_commit(&file, commit1.trim_end()).unwrap()
        );
        assert_eq!(
            "commit 2",
            file_content_at_commit(&file, commit2.trim_end()).unwrap()
        );
    }

    fn commit_file(repo: &Path, message: &str) {
        git(repo, &["add", "dummy.txt"]);
        git(repo, &["commit", "-m", message]);
    }

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
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
