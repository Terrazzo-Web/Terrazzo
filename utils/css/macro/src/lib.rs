use std::path::PathBuf;

use heck::ToShoutySnakeCase;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::LitStr;
use syn::parse_macro_input;
use terrazzo_css_shared::CssError;

#[proc_macro]
pub fn import_style(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);

    match try_import_style_classes(&input) {
        Ok(ts) => ts,
        Err(err) => syn::Error::new_spanned(&input, err.to_string())
            .to_compile_error()
            .into(),
    }
}

fn try_import_style_classes(input: &LitStr) -> Result<TokenStream, ImportStyleError> {
    try_import_style_classes_with_path(input.value().into(), input.span())
}

fn try_import_style_classes_with_path(
    file_path: PathBuf,
    identifier_span: Span,
) -> Result<TokenStream, ImportStyleError> {
    let file_content = std::fs::read_to_string(&file_path)
        .map_err(|error| ImportStyleError::ReadFileError(file_path, error))?;
    let hasher = terrazzo_css_shared::hasher::ClassNameHasher::new(&file_content);
    let output_fields = terrazzo_css_shared::list_classes(&file_content)?.map(|class| {
        let class_ident = Ident::new(&class.to_shouty_snake_case(), identifier_span);
        let class_str = hasher.hash(class);
        quote_spanned!(identifier_span =>
            pub const #class_ident: &str = #class_str;
        )
    });

    Ok(quote! { #(#output_fields)* }.into())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
enum ImportStyleError {
    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    ReadFileError(PathBuf, std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    CssError(#[from] CssError),
}
