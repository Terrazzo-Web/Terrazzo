use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use futures::FutureExt as _;
use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use futures::future::BoxFuture;
use futures::future::Shared;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher as _;
use scopeguard::defer;
use tantivy::Index;
use tantivy::IndexReader;
use tantivy::IndexWriter;
use tantivy::TantivyDocument;
use tantivy::Term;
use tantivy::collector::DocSetCollector;
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::doc;
use tantivy::query::AllQuery;
use tantivy::query::QueryParser;
use tantivy::schema::Field;
use tantivy::schema::STORED;
use tantivy::schema::STRING;
use tantivy::schema::Schema;
use tantivy::schema::TEXT;
use tantivy::schema::Value as _;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tracing::Instrument;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::warn;

use crate::text_editor::fsio;

const INDEX_WRITER_MEMORY_BUDGET: usize = 50_000_000;

type SharedIndexFuture =
    Shared<BoxFuture<'static, Result<Arc<RepositoryIndex>, Arc<SearchIndexError>>>>;

static INDEXES: LazyLock<Mutex<HashMap<PathBuf, SharedIndexFuture>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static ACTIVE_INDEXES: LazyLock<Mutex<HashMap<PathBuf, std::sync::Weak<RepositoryIndex>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub struct IndexSettings {
    pub cache_dir: Arc<Path>,
    pub refresh_interval: Duration,
    pub stale_after: Duration,
}

#[derive(Clone, Copy)]
pub(super) struct IndexFields {
    path: Field,
    pub(super) body: Field,
    size: Field,
    modified: Field,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fingerprint {
    size: u64,
    modified: u64,
}

enum WriterCommand {
    FullReconcile {
        reconcile_kind: ReconcileKind,
        reply: oneshot::Sender<Result<HashSet<PathBuf>, Arc<SearchIndexError>>>,
    },
    Reconcile {
        path: PathBuf,
        add_if_missing: bool,
    },
}

pub struct RepositoryIndex {
    root: Arc<Path>,
    cache_dir: Arc<Path>,
    pub(super) index: Index,
    pub(super) reader: IndexReader,
    pub(super) fields: IndexFields,
    tx: mpsc::UnboundedSender<WriterCommand>,
    last_full_reconcile: Arc<Mutex<Option<Instant>>>,
    refresh_interval: Duration,
    stale_after: Duration,
    git_files: Arc<RwLock<Arc<HashSet<PathBuf>>>>,
    watcher: Mutex<Option<WatcherState>>,
    query_concurrency_limit: Arc<tokio::sync::Semaphore>,
    watcher_update_lock: tokio::sync::Mutex<()>,
}

struct WatcherState {
    watcher: RecommendedWatcher,
    directories: HashSet<PathBuf>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SearchIndexError {
    #[error("Search index blocking task failed: {0}")]
    BlockingTask(#[from] tokio::task::JoinError),

    #[error("[{n}] Failed to access the Tantivy cache: {0}", n = self.name())]
    Io(#[from] std::io::Error),

    #[error("[{n}] Tantivy index operation failed: {0}", n = self.name())]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("[{n}] Failed to watch repository files: {0}", n = self.name())]
    Notify(#[from] notify::Error),

    #[error("[{n}] Failed to open the Tantivy cache: {0}", n = self.name())]
    OpenDirectory(#[from] tantivy::directory::error::OpenDirectoryError),

    #[error("[{n}] Failed to list Git files: {0}", n = self.name())]
    GitFiles(String),

    #[error("[{n}] Search index writer stopped", n = self.name())]
    WriterStopped,
}

pub async fn repository_index(
    root: Arc<Path>,
    settings: IndexSettings,
) -> Result<Arc<RepositoryIndex>, Arc<SearchIndexError>> {
    let cache_dir = root.join(&settings.cache_dir);
    debug_assert!(cache_dir.is_absolute());
    let future_repository_index = {
        let mut indexes = INDEXES.lock().unwrap();
        indexes
            .entry(cache_dir)
            .or_insert_with(|| {
                async move {
                    RepositoryIndex::initialize(root, settings)
                        .await
                        .map_err(Arc::new)
                }
                .boxed()
                .shared()
            })
            .clone()
    };
    return future_repository_index.await;
}

fn path_query(
    field: Field,
    input: &str,
) -> Result<tantivy::query::RegexQuery, tantivy::TantivyError> {
    let mut pattern = String::from(".*");
    for character in input.chars() {
        if character.is_ascii_alphabetic() {
            pattern.push_str(&format!(
                "[{}{}]",
                character.to_ascii_lowercase(),
                character.to_ascii_uppercase()
            ));
        } else {
            pattern.push_str(&regex::escape(&character.to_string()));
        }
    }
    pattern.push_str(".*");
    tantivy::query::RegexQuery::from_pattern(&pattern, field)
}

pub async fn reconcile_touched_path(path: &Path) {
    let Some(root) = fsio::git::git_repo_root(path).await else {
        return;
    };
    let index = ACTIVE_INDEXES
        .lock()
        .unwrap()
        .get(root.as_ref())
        .and_then(std::sync::Weak::upgrade);
    let Some(index) = index else {
        return;
    };
    index.reconcile(path.to_owned(), true);
}

#[derive(Clone, Copy, Debug)]
pub enum ReconcileKind {
    First,
    Refresh,
}

impl RepositoryIndex {
    async fn open_index_and_start_writer(
        root: Arc<Path>,
        settings: IndexSettings,
    ) -> Result<Arc<Self>, SearchIndexError> {
        let cache_dir: Arc<Path> = root.join(&settings.cache_dir).into();
        tokio::fs::create_dir_all(&cache_dir).await?;

        let index_directory = cache_dir.clone();
        let (index, reader, fields, writer, fingerprints) =
            tokio::task::spawn_blocking(move || {
                let (schema, fields) = make_schema();
                let directory = MmapDirectory::open(&index_directory)?;
                let index = Index::open_or_create(directory, schema)?;
                let reader = index.reader()?;
                let fingerprints = load_fingerprints(&reader, fields)?;
                let writer = index.writer(INDEX_WRITER_MEMORY_BUDGET)?;
                Ok::<_, SearchIndexError>((index, reader, fields, writer, fingerprints))
            })
            .await??;
        let (tx, rx) = mpsc::unbounded_channel();
        let last_full_reconcile = Arc::new(Mutex::new(None));

        let repository = Arc::new(Self {
            root: root.clone(),
            cache_dir,
            index,
            reader: reader.clone(),
            fields,
            tx,
            last_full_reconcile: last_full_reconcile.clone(),
            refresh_interval: settings.refresh_interval,
            stale_after: settings.stale_after,
            git_files: Arc::new(RwLock::new(Arc::new(HashSet::new()))),
            watcher: Mutex::new(None),
            query_concurrency_limit: Arc::new(tokio::sync::Semaphore::new(2)),
            watcher_update_lock: tokio::sync::Mutex::new(()),
        });

        tokio::spawn(forward_commands_to_blocking_writer(
            root.clone(),
            repository.cache_dir.clone(),
            fields,
            writer,
            reader,
            fingerprints,
            last_full_reconcile,
            rx,
        ));
        Ok(repository)
    }

    async fn initialize(
        root: Arc<Path>,
        settings: IndexSettings,
    ) -> Result<Arc<Self>, SearchIndexError> {
        let repository = Self::open_index_and_start_writer(root.clone(), settings).await?;
        let index_to_watch = repository.clone();
        tokio::task::spawn_blocking(move || index_to_watch.install_watcher()).await??;
        repository.full_reconcile(ReconcileKind::First).await?;
        ACTIVE_INDEXES
            .lock()
            .unwrap()
            .insert(root.to_path_buf(), Arc::downgrade(&repository));
        repository.spawn_periodic_refresh();
        Ok(repository)
    }

    pub async fn search(
        self: &Arc<Self>,
        input: &str,
        limit: usize,
    ) -> Result<Vec<PathBuf>, SearchIndexError> {
        let permit = self
            .query_concurrency_limit
            .clone()
            .acquire_owned()
            .await
            .expect("query semaphore closed");
        let repository = self.clone();
        let input = input.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            repository.search_contents_blocking(&input, limit)
        })
        .await?;
        result
    }

    fn search_contents_blocking(
        &self,
        input: &str,
        limit: usize,
    ) -> Result<Vec<PathBuf>, SearchIndexError> {
        let searcher = self.reader.searcher();
        let mut parser = QueryParser::for_index(&self.index, vec![self.fields.body]);
        parser.set_conjunction_by_default();
        let (query, errors) = parser.parse_query_lenient(input);
        for error in errors {
            debug!(%error, "Ignoring Tantivy query parser error");
        }
        let documents = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;
        documents
            .into_iter()
            .map(|(_, address)| {
                let document: TantivyDocument = searcher.doc(address)?;
                document
                    .get_first(self.fields.path)
                    .and_then(|value| value.as_str())
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        SearchIndexError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Indexed document has no path",
                        ))
                    })
            })
            .collect()
    }

    pub async fn search_paths(
        self: &Arc<Self>,
        input: &str,
        base: &Path,
        limit: usize,
    ) -> Result<Vec<PathBuf>, SearchIndexError> {
        let permit = self
            .query_concurrency_limit
            .clone()
            .acquire_owned()
            .await
            .expect("query semaphore closed");
        let repository = self.clone();
        let input = input.to_owned();
        let base = base.to_owned();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            repository.search_paths_blocking(&input, &base, limit)
        })
        .await?
    }

    fn search_paths_blocking(
        &self,
        input: &str,
        base: &Path,
        limit: usize,
    ) -> Result<Vec<PathBuf>, SearchIndexError> {
        let query = path_query(self.fields.path, input)?;
        let scope = tantivy::query::RegexQuery::from_pattern(
            &format!(
                "{}.*",
                regex::escape(&if base.as_os_str().is_empty() {
                    String::new()
                } else {
                    format!("{}/", base.display())
                })
            ),
            self.fields.path,
        )?;
        let query = tantivy::query::BooleanQuery::intersection(vec![
            Box::new(query) as Box<dyn tantivy::query::Query>,
            Box::new(scope),
        ]);
        let searcher = self.reader.searcher();
        searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())?
            .into_iter()
            .map(|(_, address)| {
                let document: TantivyDocument = searcher.doc(address)?;
                document
                    .get_first(self.fields.path)
                    .and_then(|value| value.as_str())
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        SearchIndexError::Io(std::io::Error::other("Indexed document has no path"))
                    })
            })
            .collect()
    }

    pub fn refresh_if_stale(self: &Arc<Self>) {
        let is_stale = self
            .last_full_reconcile
            .lock()
            .unwrap()
            .is_none_or(|last| last.elapsed() >= self.stale_after);
        if !is_stale {
            return;
        }
        let index = self.clone();
        tokio::spawn(async move {
            if let Err(error) = index.full_reconcile(ReconcileKind::Refresh).await {
                warn!(%error, root = ?index.root, "Failed to refresh search index");
            }
        });
    }

    async fn full_reconcile(
        self: &Arc<Self>,
        reconcile_kind: ReconcileKind,
    ) -> Result<(), SearchIndexError> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(WriterCommand::FullReconcile {
                reconcile_kind,
                reply,
            })
            .map_err(|_| SearchIndexError::WriterStopped)?;
        let git_files = response
            .await
            .map_err(|_| SearchIndexError::WriterStopped)?
            .map_err(|error| SearchIndexError::Io(std::io::Error::other(error.to_string())))?;
        self.update_watcher(Arc::new(git_files)).await?;
        Ok(())
    }

    fn reconcile(&self, path: PathBuf, add_if_missing: bool) {
        if self.is_cache_path(&path) {
            return;
        }
        let Ok(path) = path.strip_prefix(&self.root).map(Path::to_owned) else {
            return;
        };
        let _ = self.tx.send(WriterCommand::Reconcile {
            path,
            add_if_missing,
        });
    }

    fn is_cache_path(&self, path: &Path) -> bool {
        path.starts_with(&self.cache_dir)
    }

    fn install_watcher(&self) -> Result<(), SearchIndexError> {
        let root = self.root.clone();
        let cache_dir = self.cache_dir.clone();
        let git_files = self.git_files.clone();
        let tx = self.tx.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                match event.kind {
                    notify::EventKind::Create(_)
                    | notify::EventKind::Modify(_)
                    | notify::EventKind::Remove(_) => {}
                    _ => return,
                }
                for path in event.paths {
                    if path.starts_with(&cache_dir) {
                        continue;
                    }
                    let Ok(path) = path.strip_prefix(&root).map(Path::to_owned) else {
                        continue;
                    };
                    if !git_files.read().unwrap().contains(&path) {
                        continue;
                    }
                    let _ = tx.send(WriterCommand::Reconcile {
                        path,
                        add_if_missing: false,
                    });
                }
            })?;
        watcher.watch(&self.root, RecursiveMode::NonRecursive)?;
        *self.watcher.lock().unwrap() = Some(WatcherState {
            watcher,
            directories: HashSet::from([self.root.to_path_buf()]),
        });
        Ok(())
    }

    async fn update_watcher(
        self: &Arc<Self>,
        git_files: Arc<HashSet<PathBuf>>,
    ) -> Result<(), SearchIndexError> {
        let _watcher_update_guard = self.watcher_update_lock.lock().await;
        *self.git_files.write().unwrap() = git_files.clone();
        let directories = watch_directories(&self.root, &git_files).await;
        let watched_directories = self
            .watcher
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.directories.clone())
            .unwrap_or_default();
        let mut directories_to_watch = vec![];
        for (index, directory) in directories.difference(&watched_directories).enumerate() {
            if index % 64 == 0 {
                tokio::task::yield_now().await;
            }
            let Ok(metadata) = tokio::fs::symlink_metadata(directory).await else {
                continue;
            };
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                directories_to_watch.push(directory.clone());
            }
        }
        let repository = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut state = repository.watcher.lock().unwrap();
            let Some(state) = state.as_mut() else {
                return Ok(());
            };
            for directory in directories_to_watch {
                if state.directories.contains(&directory) {
                    continue;
                }
                state
                    .watcher
                    .watch(&directory, RecursiveMode::NonRecursive)?;
                state.directories.insert(directory);
            }
            for directory in state
                .directories
                .difference(&directories)
                .cloned()
                .collect::<Vec<_>>()
            {
                if let Err(error) = state.watcher.unwatch(&directory) {
                    debug!(%error, ?directory, "Failed to remove stale search index watch");
                }
                state.directories.remove(&directory);
            }
            Ok::<_, SearchIndexError>(())
        })
        .await?
    }

    fn spawn_periodic_refresh(self: &Arc<Self>) {
        let index = Arc::downgrade(self);
        let refresh_interval = self.refresh_interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(refresh_interval);
            interval.tick().await;
            loop {
                interval.tick().await;
                let Some(index) = index.upgrade() else {
                    return;
                };
                if let Err(error) = index.full_reconcile(ReconcileKind::Refresh).await {
                    warn!(%error, root = ?index.root, "Periodic search index refresh failed");
                }
            }
        });
    }
}

async fn watch_directories(root: &Path, git_files: &HashSet<PathBuf>) -> HashSet<PathBuf> {
    let mut directories = HashSet::from([root.to_path_buf()]);
    for (index, path) in git_files.iter().enumerate() {
        if index % 64 == 0 {
            tokio::task::yield_now().await;
        }
        let Some(parent) = path.parent() else {
            continue;
        };
        for ancestor in parent.ancestors() {
            directories.insert(root.join(ancestor));
        }
    }
    directories
}

fn make_schema() -> (Schema, IndexFields) {
    let mut schema = Schema::builder();
    let path = schema.add_text_field("path", STRING | STORED);
    let body = schema.add_text_field("body", TEXT);
    let size = schema.add_u64_field("size", STORED);
    let modified = schema.add_u64_field("modified", STORED);
    (
        schema.build(),
        IndexFields {
            path,
            body,
            size,
            modified,
        },
    )
}

fn load_fingerprints(
    reader: &IndexReader,
    fields: IndexFields,
) -> Result<HashMap<PathBuf, Fingerprint>, SearchIndexError> {
    let searcher = reader.searcher();
    let limit = searcher.num_docs() as usize;
    if limit == 0 {
        return Ok(HashMap::new());
    }
    let documents = searcher.search(&AllQuery, &DocSetCollector)?;
    let mut fingerprints = HashMap::with_capacity(documents.len());
    for address in documents {
        let document: TantivyDocument = searcher.doc(address)?;
        let Some(path) = document
            .get_first(fields.path)
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        let Some(size) = document
            .get_first(fields.size)
            .and_then(|value| value.as_u64())
        else {
            continue;
        };
        let Some(modified) = document
            .get_first(fields.modified)
            .and_then(|value| value.as_u64())
        else {
            continue;
        };
        fingerprints.insert(path.into(), Fingerprint { size, modified });
    }
    Ok(fingerprints)
}

#[allow(clippy::too_many_arguments)]
async fn forward_commands_to_blocking_writer(
    root: Arc<Path>,
    cache_dir: Arc<Path>,
    fields: IndexFields,
    writer: IndexWriter,
    reader: IndexReader,
    fingerprints: HashMap<PathBuf, Fingerprint>,
    last_full_reconcile: Arc<Mutex<Option<Instant>>>,
    mut incoming_commands: mpsc::UnboundedReceiver<WriterCommand>,
) {
    // The async task owns the blocking writer's input, so shutdown closes it even
    // when a cached RepositoryIndex still holds a sender to this task.
    let (writer_commands, pending_writer_commands) = mpsc::unbounded_channel();
    let runtime_handle = tokio::runtime::Handle::current();
    let blocking_writer = tokio::task::spawn_blocking(move || {
        run_blocking_index_writer(
            root,
            cache_dir,
            fields,
            writer,
            reader,
            fingerprints,
            last_full_reconcile,
            pending_writer_commands,
            runtime_handle,
        );
    });
    while let Some(command) = incoming_commands.recv().await {
        if writer_commands.send(command).is_err() {
            break;
        }
    }
    drop(writer_commands);
    if let Err(error) = blocking_writer.await {
        warn!(%error, "Search index writer failed");
    }
}

#[allow(clippy::too_many_arguments)]
fn run_blocking_index_writer(
    root: Arc<Path>,
    cache_dir: Arc<Path>,
    fields: IndexFields,
    mut writer: IndexWriter,
    reader: IndexReader,
    mut fingerprints: HashMap<PathBuf, Fingerprint>,
    last_full_reconcile: Arc<Mutex<Option<Instant>>>,
    mut rx: mpsc::UnboundedReceiver<WriterCommand>,
    runtime: tokio::runtime::Handle,
) {
    let _span = info_span!("Tantivy Index").entered();
    while let Some(command) = rx.blocking_recv() {
        match command {
            WriterCommand::FullReconcile {
                reconcile_kind,
                reply,
            } => {
                runtime.block_on(
                    full_reconcile(
                        &root,
                        &cache_dir,
                        fields,
                        &mut writer,
                        &reader,
                        &mut fingerprints,
                        &last_full_reconcile,
                        reconcile_kind,
                        reply,
                    )
                    .instrument(info_span!("Full reconcile", ?reconcile_kind)),
                );
            }
            WriterCommand::Reconcile {
                path,
                add_if_missing,
            } => {
                let _span = info_span!("Reconcile", ?path).entered();
                reconcile(
                    &root,
                    fields,
                    &mut writer,
                    &reader,
                    &mut fingerprints,
                    path,
                    add_if_missing,
                );
            }
        }
    }
}

async fn full_reconcile(
    root: &Arc<Path>,
    cache_dir: &Arc<Path>,
    fields: IndexFields,
    writer: &mut IndexWriter,
    reader: &IndexReader,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    last_full_reconcile: &Arc<Mutex<Option<Instant>>>,
    reconcile_kind: ReconcileKind,
    reply: oneshot::Sender<Result<HashSet<PathBuf>, Arc<SearchIndexError>>>,
) {
    let start = Instant::now();
    info!("Start");
    defer!(info!("End"));
    let result = async {
        let paths = super::utils::git_files(root.clone(), root.clone())
            .await
            .map_err(|error| SearchIndexError::GitFiles(error.to_string()))?;
        let paths = match reconcile_kind {
            ReconcileKind::First => collect_paths(paths).await?,
            ReconcileKind::Refresh => {
                let mut i = 0;
                collect_paths(paths.flat_map(|item| {
                    i += 1;
                    futures::stream::once(async move {
                        if i % 100 == 0 {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            debug!(i, "Listing git files...");
                        }
                        item
                    })
                }))
                .await?
            }
        };
        info!("Found {} git files", paths.len());
        let mut indexable_paths = HashSet::with_capacity(paths.len());
        for (index, path) in paths.into_iter().enumerate() {
            if index % 64 == 0 {
                tokio::task::yield_now().await;
            }
            if !root.join(&path).starts_with(cache_dir.as_ref()) {
                indexable_paths.insert(path);
            }
        }
        let paths = indexable_paths;
        reconcile_all(root, fields, writer, reader, fingerprints, &paths)?;
        Ok(paths)
    }
    .await
    .map_err(Arc::new);
    if result.is_ok() {
        *last_full_reconcile.lock().unwrap() = Some(Instant::now());
    }
    let _ = reply.send(result);
    info!("Elapsed: {}", humantime::format_duration(start.elapsed()))
}

fn reconcile(
    root: &Arc<Path>,
    fields: IndexFields,
    writer: &mut IndexWriter,
    reader: &IndexReader,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    path: PathBuf,
    add_if_missing: bool,
) {
    debug!("Start");
    defer!(debug!("End"));
    let result = reconcile_one(root, fields, writer, fingerprints, &path, add_if_missing).and_then(
        |changed| {
            if changed {
                debug!("Updated index");
                writer.commit()?;
                reader.reload()?;
            } else {
                debug!("Ignored");
            }
            Ok(())
        },
    );
    if let Err(error) = result {
        warn!(%error, ?path, "Failed to reconcile search index file");
    }
}

async fn collect_paths(
    paths: impl Stream<Item = Result<PathBuf, super::SearchError>>,
) -> Result<Vec<PathBuf>, SearchIndexError> {
    let mut path_count = 0usize;
    paths
        .then(move |path| {
            path_count += 1;
            let should_yield = path_count % 64 == 0;
            async move {
                if should_yield {
                    tokio::task::yield_now().await;
                }
                path
            }
        })
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| SearchIndexError::GitFiles(error.to_string()))
}

fn reconcile_all(
    root: &Path,
    fields: IndexFields,
    writer: &mut IndexWriter,
    reader: &IndexReader,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    paths: &HashSet<PathBuf>,
) -> Result<(), SearchIndexError> {
    let mut changed = false;
    for path in paths {
        changed |= reconcile_one(root, fields, writer, fingerprints, path, true)?;
    }
    let deleted: Vec<_> = fingerprints
        .keys()
        .filter(|path| !paths.contains(*path))
        .cloned()
        .collect();
    for path in deleted {
        writer.delete_term(Term::from_field_text(fields.path, &path.to_string_lossy()));
        fingerprints.remove(&path);
        changed = true;
    }
    if changed {
        writer.commit()?;
        reader.reload()?;
    }
    Ok(())
}

fn reconcile_one(
    root: &Path,
    fields: IndexFields,
    writer: &mut IndexWriter,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    path: &Path,
    add_if_missing: bool,
) -> Result<bool, SearchIndexError> {
    let previous = fingerprints.get(path).copied();
    if previous.is_none() && !add_if_missing {
        return Ok(false);
    }
    let full_path = root.join(path);
    let metadata = match std::fs::metadata(&full_path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return remove_document(writer, fields, fingerprints, path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return remove_document(writer, fields, fingerprints, path);
        }
        Err(error) => return Err(error.into()),
    };
    let fingerprint = Fingerprint {
        size: metadata.len(),
        modified: modified_nanos(metadata.modified()?),
    };
    if previous == Some(fingerprint) {
        return Ok(false);
    }
    let body = match std::fs::read_to_string(&full_path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            return remove_document(writer, fields, fingerprints, path);
        }
        Err(error) => return Err(error.into()),
    };
    writer.delete_term(Term::from_field_text(fields.path, &path.to_string_lossy()));
    writer.add_document(tantivy::doc!(
        fields.path => path.to_string_lossy().into_owned(),
        fields.body => body,
        fields.size => fingerprint.size,
        fields.modified => fingerprint.modified,
    ))?;
    fingerprints.insert(path.to_owned(), fingerprint);
    Ok(true)
}

fn remove_document(
    writer: &IndexWriter,
    fields: IndexFields,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    path: &Path,
) -> Result<bool, SearchIndexError> {
    if fingerprints.remove(path).is_none() {
        return Ok(false);
    }
    writer.delete_term(Term::from_field_text(fields.path, &path.to_string_lossy()));
    Ok(true)
}

fn modified_nanos(modified: SystemTime) -> u64 {
    modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;

    use super::watch_directories;

    #[test]
    fn runtime_shutdown_releases_the_writer_even_with_a_cached_repository() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let repository = runtime.block_on(async {
            let root = tokio::fs::canonicalize(directory.path()).await.unwrap();
            let output = tokio::process::Command::new("git")
                .arg("init")
                .current_dir(&root)
                .output()
                .await
                .unwrap();
            assert!(output.status.success());
            let repository = super::RepositoryIndex::open_index_and_start_writer(
                root.into(),
                super::IndexSettings {
                    cache_dir: Path::new(".git/terrazzo-test-index").into(),
                    refresh_interval: std::time::Duration::from_secs(3600),
                    stale_after: std::time::Duration::from_secs(3600),
                },
            )
            .await
            .unwrap();
            repository
                .full_reconcile(super::ReconcileKind::First)
                .await
                .unwrap();
            repository
        });
        runtime.shutdown_timeout(std::time::Duration::from_secs(5));
        // A live cached sender must not keep the old writer's file lock held.
        let _writer: tantivy::IndexWriter = repository.index.writer(50_000_000).unwrap();
    }

    #[tokio::test]
    async fn disk_index_queries_and_reconciliation_stay_consistent() {
        let directory = tempfile::tempdir().unwrap();
        let root = tokio::fs::canonicalize(directory.path()).await.unwrap();
        let output = tokio::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        tokio::fs::write(root.join("first.txt"), "originalword")
            .await
            .unwrap();
        let repository = super::RepositoryIndex::open_index_and_start_writer(
            root.clone().into(),
            super::IndexSettings {
                cache_dir: Path::new(".git/terrazzo-test-index").into(),
                refresh_interval: std::time::Duration::from_secs(3600),
                stale_after: std::time::Duration::from_secs(3600),
            },
        )
        .await
        .unwrap();
        repository
            .full_reconcile(super::ReconcileKind::First)
            .await
            .unwrap();
        assert_eq!(
            repository.search("originalword", 10).await.unwrap(),
            vec![PathBuf::from("first.txt")]
        );
        assert_eq!(
            repository
                .search_paths("FIRST", Path::new(""), 10)
                .await
                .unwrap(),
            vec![PathBuf::from("first.txt")]
        );
        tokio::fs::write(root.join("first.txt"), "replacementword")
            .await
            .unwrap();
        repository
            .full_reconcile(super::ReconcileKind::First)
            .await
            .unwrap();
        assert!(
            repository
                .search("originalword", 10)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            repository.search("replacementword", 10).await.unwrap(),
            vec![PathBuf::from("first.txt")]
        );
        tokio::fs::remove_file(root.join("first.txt"))
            .await
            .unwrap();
        repository
            .full_reconcile(super::ReconcileKind::First)
            .await
            .unwrap();
        assert!(
            repository
                .search("replacementword", 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn autocomplete_searches_paths_only_and_treats_input_literally() {
        let (schema, fields) = super::make_schema();
        let index = tantivy::Index::create_in_ram(schema);
        let mut writer = index.writer(50_000_000).unwrap();
        writer
            .add_document(
                tantivy::doc!(fields.path => "src/Nested/File.rs", fields.body => "unrelated"),
            )
            .unwrap();
        writer
            .add_document(tantivy::doc!(fields.path => "other.txt", fields.body => "file.rs"))
            .unwrap();
        writer
            .add_document(tantivy::doc!(fields.path => "src/fileXrs", fields.body => "unrelated"))
            .unwrap();
        writer.commit().unwrap();
        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let query = super::path_query(fields.path, "file.rs").unwrap();
        let docs = searcher
            .search(
                &query,
                &tantivy::collector::TopDocs::with_limit(30).order_by_score(),
            )
            .unwrap();
        assert_eq!(docs.len(), 1);
        let document: tantivy::TantivyDocument = searcher.doc(docs[0].1).unwrap();
        use tantivy::schema::Value as _;
        assert_eq!(
            document.get_first(fields.path).unwrap().as_str(),
            Some("src/Nested/File.rs")
        );
    }

    #[tokio::test]
    async fn watch_only_directories_containing_git_files() {
        let git_files = HashSet::from([
            PathBuf::from("README.md"),
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/nested/lib.rs"),
        ]);

        assert_eq!(
            HashSet::from([
                PathBuf::from("/repo"),
                PathBuf::from("/repo/src"),
                PathBuf::from("/repo/src/nested"),
            ]),
            watch_directories(Path::new("/repo"), &git_files).await,
        );
    }
}
