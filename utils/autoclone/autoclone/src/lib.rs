use crate::autoclone::autoclone2;
use crate::envelope::envelope2;
use crate::graphs::graph2;

mod autoclone;
mod envelope;
mod graphs;

/// A simple macro to cloning variable before passing them into a `move` closure or async block.
///
/// See [crate] documentations for details.
#[proc_macro_attribute]
pub fn autoclone(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    autoclone2(attr.into(), item.into()).into()
}

/// A simple macro to wrap a type in a shared pointer.
///
/// See [crate] documentations for details.
#[proc_macro_attribute]
pub fn envelope(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    envelope2(attr.into(), item.into()).into()
}

/// A macro to write algorithms as graphs.
///
/// Declare each function before the graph functions it depends on (parameters
/// whose names match functions in the module), including private helper chains.
///
/// The module documentation must contain a fenced `graph` block showing every
/// function and every direct dependency (external inputs are not nodes).
/// Arrows point from a dependency to its consumer; plain lines allow either
/// direction.
///
/// Draw an intuitive visual layout that makes the flow, branches, and shared
/// dependencies visible at a glance. Arrange dependencies on the left and their
/// consumers on the right so data flows left to right. Write each node once
/// unless doing so is impractical; use connectors to show branches and joins.
/// For AI-generated
/// diagrams, prefer Unicode box drawing symbols and arrows for a polished,
/// readable layout. For example:
///
/// ````text
/// /// ```graph
/// /// ┌──────── left ──→ run
/// /// │                   │
/// /// shared ──→ right ──→┘
/// /// ```
/// ````
///
/// Diagrams use character columns, continuous vertical/diagonal strokes, and
/// optional horizontal spaces. Supported connectors include `-`, `|`, `/`,
/// `\`, `+`, Unicode box drawing lines/corners/junctions, and `→ ← ↑ ↓`.
/// Junctions connect all their branches; use separate strokes for crossings.
#[proc_macro_attribute]
pub fn graph(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    graph2(attr.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn item_to_string(item: &syn::Item) -> String {
    prettyplease::unparse(&syn::File {
        frontmatter: None,
        shebang: None,
        attrs: vec![],
        items: vec![item.clone()],
    })
}

#[cfg(test)]
use clone_macro as _;

fn ident_to_snake_case(name: impl std::fmt::Display + Copy) -> String {
    let name = name.to_string();
    let mut result = String::default();
    let mut last_is_upper = false;
    for c in name.chars() {
        if !last_is_upper && c.is_uppercase() {
            last_is_upper = true;
            if !result.is_empty() {
                result.push('_');
            }
            result.push_str(&c.to_lowercase().to_string());
        } else {
            last_is_upper = false;
            result.push_str(&c.to_lowercase().to_string());
        }
    }
    return result;
}

#[cfg(test)]
mod tests {
    #[test]
    fn ident_to_snake_case() {
        assert_eq!("file_system_io", super::ident_to_snake_case("FileSystemIO"));
    }
}
