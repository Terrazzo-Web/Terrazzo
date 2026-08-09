# Search

See search functionality in terminal/src/text_editor/search/service.rs

Extend search by returning two streams combined with `flatten_unordered`:

- One stream matches files by name.
- One stream searches file contents with Tantivy.

The Tantivy search should:

1. Maintain one cached search index per Git repository root. Within an index, uniquely
   identify each file by its path relative to the Git root.
2. Initialize the search index on the first search. Store initialization as a `Shared<...>`
   future so concurrent searches wait for the same initialization instead of starting duplicate
   work.
3. Store the index in `<git root>/.tantivy-cache`. Make the cache directory configurable through
   `terminal/src/backend/cli.rs` and
   `pub struct ServerConfig<T: ConfigTypes = RuntimeTypes>`.
4. Always reconcile an existing cache during the first search. Run `git ls-files` as today and
   compare each file's size and last-modified time with the values stored in Tantivy. Matching
   files are a cheap no-op.
5. Record each indexed file's size and last-modified time in Tantivy. When reconciling a file:
   - Remove its document if the file has been deleted.
   - Remove and reinsert its document if its size or last-modified time has changed.
   - Do nothing if both values still match.
6. Use the `notify` crate to watch the Git root for file changes. Submit changed paths to the same
   reconciliation routine described above instead of reindexing unconditionally.
7. Run the full index reconciliation every hour by default; make this interval configurable.
8. After a search, if full reconciliation has not run within the last five minutes, run it again.
   Make this interval configurable.
9. Exclude `.tantivy-cache` from both `git ls-files` results and `notify` events. Index writes must
   not index the cache itself or trigger recursive reconciliation.

Concurrency and index updates:

- Searches may run while initialization or reconciliation modifies the index. Tantivy searches
  use a stable committed reader snapshot; updates become visible after the writer commits and the
  reader reloads.
- Maintain exactly one long-lived Tantivy `IndexWriter` per Git root.
- Give each Git root an `mpsc` writer task. Initialization, `notify` events, and search-time checks
  submit `Reconcile(path)` operations to this task.
- The writer task must read the current file metadata and contents when it processes an operation,
  then perform the required no-op, delete, or delete-and-insert sequence. This prevents queued
  stale observations from overwriting newer filesystem state.
- Keep the complete delete-and-insert sequence ordered within the writer task, batch commits where
  practical, and commit at the end of initialization.
- Searches read through an `IndexReader` directly; they do not pass through the writer channel.
  The first search waits for shared initialization, while later refreshes may run concurrently
  with searches.


Tests: add an integration test based on
`terminal/tests/integration-test-text-editor-viewer.spec.mjs`, named
`terminal/tests/integration-test-text-editor-search.spec.mjs`. Verify that searching for
"Maintain one cached search index per Git repository root." finds this Markdown file.
