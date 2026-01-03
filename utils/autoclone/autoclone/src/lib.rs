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
        assert_eq!(
            "file_system_iO",
            super::ident_to_snake_case(&"FileSystemIO")
        );
    }
}
