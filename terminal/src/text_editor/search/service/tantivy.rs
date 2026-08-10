use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
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
        reply: oneshot::Sender<Result<(), Arc<SearchIndexError>>>,
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
    watcher: Mutex<Option<RecommendedWatcher>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchIndexError {
    #[error("Failed to access the Tantivy cache: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tantivy index error: {0}")]
    Tantivy(#[from] tantivy::TantivyError),

    #[error("Failed to watch repository files: {0}")]
    Notify(#[from] notify::Error),

    #[error("Failed to open the Tantivy cache: {0}")]
    OpenDirectory(#[from] tantivy::directory::error::OpenDirectoryError),

    #[error("Failed to list Git files: {0}")]
    GitFiles(String),

    #[error("Search index writer stopped")]
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

pub fn reconcile_touched_path(path: &Path) {
    let Some(root) = fsio::git::git_repo_root(path) else {
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
    async fn initialize(
        root: Arc<Path>,
        settings: IndexSettings,
    ) -> Result<Arc<Self>, SearchIndexError> {
        let cache_dir: Arc<Path> = root.join(&settings.cache_dir).into();
        tokio::fs::create_dir_all(&cache_dir).await?;

        let (schema, fields) = make_schema();
        let directory = MmapDirectory::open(&cache_dir)?;
        let index = Index::open_or_create(directory, schema)?;
        let reader = index.reader()?;
        let fingerprints = load_fingerprints(&reader, fields)?;
        let writer = index.writer(INDEX_WRITER_MEMORY_BUDGET)?;
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
            watcher: Mutex::new(None),
        });

        tokio::spawn(writer_loop(
            root.clone(),
            repository.cache_dir.clone(),
            fields,
            writer,
            reader,
            fingerprints,
            last_full_reconcile,
            rx,
        ));
        repository.full_reconcile(ReconcileKind::First).await?;
        repository.install_watcher()?;
        ACTIVE_INDEXES
            .lock()
            .unwrap()
            .insert(root.to_path_buf(), Arc::downgrade(&repository));
        repository.spawn_periodic_refresh();
        Ok(repository)
    }

    pub fn search(&self, input: &str, limit: usize) -> Result<Vec<PathBuf>, SearchIndexError> {
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

    async fn full_reconcile(&self, reconcile_kind: ReconcileKind) -> Result<(), SearchIndexError> {
        let (reply, response) = oneshot::channel();
        self.tx
            .send(WriterCommand::FullReconcile {
                reconcile_kind,
                reply,
            })
            .map_err(|_| SearchIndexError::WriterStopped)?;
        response
            .await
            .map_err(|_| SearchIndexError::WriterStopped)?
            .map_err(|error| SearchIndexError::Io(std::io::Error::other(error.to_string())))
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
                    let _ = tx.send(WriterCommand::Reconcile {
                        path,
                        add_if_missing: false,
                    });
                }
            })?;
        watcher.watch(&self.root, RecursiveMode::Recursive)?;
        *self.watcher.lock().unwrap() = Some(watcher);
        Ok(())
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
async fn writer_loop(
    root: Arc<Path>,
    cache_dir: Arc<Path>,
    fields: IndexFields,
    mut writer: IndexWriter,
    reader: IndexReader,
    mut fingerprints: HashMap<PathBuf, Fingerprint>,
    last_full_reconcile: Arc<Mutex<Option<Instant>>>,
    mut rx: mpsc::UnboundedReceiver<WriterCommand>,
) {
    async {
        while let Some(command) = rx.recv().await {
            match command {
                WriterCommand::FullReconcile {
                    reconcile_kind,
                    reply,
                } => {
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
                    .instrument(info_span!("Full reconcile", ?reconcile_kind))
                    .await;
                }
                WriterCommand::Reconcile {
                    path,
                    add_if_missing,
                } => {
                    let span = info_span!("Reconcile", ?path);
                    reconcile(
                        &root,
                        fields,
                        &mut writer,
                        &reader,
                        &mut fingerprints,
                        path,
                        add_if_missing,
                    )
                    .instrument(span)
                    .await;
                }
            }
        }
    }
    .instrument(info_span!("Tantivy Index"))
    .await
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
    reply: oneshot::Sender<Result<(), Arc<SearchIndexError>>>,
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
        reconcile_all(
            root,
            cache_dir,
            fields,
            writer,
            reader,
            fingerprints,
            paths,
        )
        .await
    }
    .await
    .map_err(Arc::new);
    if result.is_ok() {
        *last_full_reconcile.lock().unwrap() = Some(Instant::now());
    }
    let _ = reply.send(result);
    info!("Elapsed: {}", humantime::format_duration(start.elapsed()))
}

async fn reconcile(
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
    let result = reconcile_one(root, fields, writer, fingerprints, &path, add_if_missing)
        .await
        .and_then(|changed| {
            if changed {
                debug!("Updated index");
                writer.commit()?;
                reader.reload()?;
            } else {
                debug!("Ignored");
            }
            Ok(())
        });
    if let Err(error) = result {
        warn!(%error, ?path, "Failed to reconcile search index file");
    }
}

async fn collect_paths(
    paths: impl Stream<Item = Result<PathBuf, super::SearchError>>,
) -> Result<Vec<PathBuf>, SearchIndexError> {
    paths
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| SearchIndexError::GitFiles(error.to_string()))
}

async fn reconcile_all(
    root: &Path,
    cache_dir: &Path,
    fields: IndexFields,
    writer: &mut IndexWriter,
    reader: &IndexReader,
    fingerprints: &mut HashMap<PathBuf, Fingerprint>,
    paths: Vec<PathBuf>,
) -> Result<(), SearchIndexError> {
    let paths: HashSet<_> = paths
        .into_iter()
        .filter(|path| !root.join(path).starts_with(cache_dir))
        .collect();
    let mut changed = false;
    for path in &paths {
        changed |= reconcile_one(root, fields, writer, fingerprints, path, true).await?;
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

async fn reconcile_one(
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
    let metadata = match tokio::fs::metadata(&full_path).await {
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

    let body = match tokio::fs::read_to_string(&full_path).await {
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
