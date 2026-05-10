#![cfg(feature = "server")]

use std::num::NonZero;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;

use lru::LruCache;

const LRU_CACHE_SIZE: NonZero<usize> = NonZero::new(10_000).expect("LRU_CACHE_SIZE");

static GIT_REPOS: LazyLock<GitReposCache<StdFs>> = LazyLock::new(|| GitReposCache::new(StdFs));

pub fn is_in_git_repo(path: impl AsRef<Path>) -> bool {
    GIT_REPOS.is_in_git_repo(path.as_ref())
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
    cache: Mutex<LruCache<PathBuf, bool>>,
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
    fn is_in_git_repo(&self, path: impl AsRef<Path>) -> bool {
        let mut cache = self.cache.lock().unwrap();

        let ancestors = path.as_ref().ancestors();
        let mut backfill = vec![];
        let mut result = false;
        for ancestor in ancestors {
            if let Some(t) = self.maybe_in_git_repo(ancestor, &mut cache, &mut backfill) {
                result = t;
                break;
            }
        }
        for ancestor in backfill {
            cache.push(ancestor.to_owned(), result);
        }
        return result;
    }

    fn maybe_in_git_repo<'l>(
        &self,
        path: &'l Path,
        cache: &mut LruCache<PathBuf, bool>,
        backfill: &mut Vec<&'l Path>,
    ) -> Option<bool> {
        if let Some(is_in_git_repo) = cache.get(path) {
            return Some(*is_in_git_repo);
        }
        backfill.push(path);
        return self.fs.is_dir(&path.join(".git")).then_some(true);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashSet;

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
    fn file_uses_cached_parent_result() {
        let fs = MockFs::default()
            .with_dir("/repo")
            .with_dir("/repo/src")
            .with_dir("/repo/.git");
        let cache = GitReposCache::new(fs);

        assert!(cache.is_in_git_repo("/repo/src"));
        assert_eq!(
            &["/repo/src/.git", "/repo/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert!(cache.is_in_git_repo("/repo/src/main.rs"));
        assert_eq!(
            &["/repo/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert!(cache.is_in_git_repo("/repo/src/main.rs"));
        assert!(cache.fs.calls.borrow().is_empty());
        cache.fs.calls.borrow_mut().clear();
    }

    #[test]
    fn caches_negative_results() {
        let fs = MockFs::default()
            .with_dir("/workspace")
            .with_dir("/workspace/src");
        let cache = GitReposCache::new(fs);

        assert!(!cache.is_in_git_repo("/workspace/src/lib.rs"));
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

        assert!(!cache.is_in_git_repo("/workspace/src/main.rs"));
        assert_eq!(
            &["/workspace/src/main.rs/.git"].map(PathBuf::from),
            cache.fs.calls.borrow().as_slice()
        );
        cache.fs.calls.borrow_mut().clear();

        assert!(!cache.is_in_git_repo("/workspace/src/main.rs"));
        assert!(cache.fs.calls.borrow().is_empty());
        cache.fs.calls.borrow_mut().clear();
    }
}
