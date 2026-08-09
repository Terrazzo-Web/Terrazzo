# Search

See search functionality in terminal/src/text_editor/search/service.rs

Extend it by returning two streams flattened with flatten_unordered
- One stream matches files by name
- One stream searches files with Tantivy.

The Tantivy search should
1. Maintain a cache of search indexes per root git folder. In each git repo search index cache, a file is uniquely identified by its path relative to the git root.
2. Search index gets initialized on first search. The initialization future should be a Shared<...> future so multiple searches wait on ethe same initialization future
3. The index cache should be in <git root>/<.tantivy-cache>. Make it configurable on the CLI terminal/src/backend/cli.rs and config `pub struct ServerConfig<T: ConfigTypes = RuntimeTypes>`.
4. If the cache is already there, re-initialize the cache again on first search. This should be taken care of by point (8.) below. This is cheap, just git ls files like we do today, check if the file size/last modification time matches in Tantivy, if yes ==> cheap no-op.
5. On search, check each file if it has been updated since last indexing. So that means you need to record the last modification time and size in Tantivy.
   - If the file is deleted, remove it from Tantivy
   - If the file modification or size has changed, re-index it (remove+insert again).
6. Use the notify crate to listen to file changes in the git root folder. Only care about file modifications. Re-index files when they change (go through the same routine that checks for deletion, modification time, and file size before deciding if you actually need to reindex)
7. Every hour (configurable), run the search index initialization routine
8. After a search, if the search index initialization routine hasn't run in the last 5 minutes (configurable), run it again.


Question: what if the initialization routine is running while search is running?
- I think you can modify the index while searching, this is OK
- What if search notices that a file no longer exists and needs to be deleted from the index? Do we need to pipe operations though an mpsc channel so there is only one thread modifying the index at a time?
