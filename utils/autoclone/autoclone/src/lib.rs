use crate::autoclone::autoclone2;
use crate::envelope::envelope2;

mod autoclone;
mod envelope;

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

fn item_to_string(item: &syn::Item) -> String {
    prettyplease::unparse(&syn::File {
        shebang: None,
        attrs: vec![],
        items: vec![item.clone()],
    })
}

#[cfg(test)]
use clone_macro as _;
