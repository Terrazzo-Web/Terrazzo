#![cfg(feature = "text-editor")]

mod autocomplete;
pub mod file_path;
mod fsio;
mod manager;
pub mod notify;
mod path_selector;
mod rust_lang;
mod search;
mod side;
mod state;
mod synchronized_state;
pub mod ui;

#[cfg(feature = "client")]
terrazzo_css::import_style!(style, "text_editor.scss");

/* TODO
Open multiple app tiles
- Review usage of global state
- Each app has an address
  - [] = root pane, current
  - ['h']

Side-by-side view of .tex and .pdf
- The logic is, when opening a file, check if there are other files with the same extension. If yes, show a dropdown of such files. If selected, split the pane in two and show two editors

Search engine
- implement tantivy

Folder view
- not just a view of previously opened files

Save cursor position
 */
