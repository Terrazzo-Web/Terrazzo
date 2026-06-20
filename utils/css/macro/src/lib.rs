use std::path::PathBuf;

use heck::ToShoutySnakeCase;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::parse_macro_input;
use terrazzo_css_shared::ScssError;
use terrazzo_css_shared::hasher::ClassNameHasher;

#[proc_macro]
pub fn import_style(input: TokenStream) -> TokenStream {
    match try_import_style(parse_macro_input!(input as syn::LitStr)) {
        Ok(ok) => ok,
        Err(error) if std::env::var("RUST_ANALYZER_INTERNALS_DO_NOT_USE").is_err() => {
            error.into_compile_error().into()
        }
        Err(_) => TokenStream::new(),
    }
}

fn try_import_style(scss_path: syn::LitStr) -> Result<TokenStream, syn::Error> {
    let current_file = current_file(&scss_path)?;

    let scss_path_span = scss_path.span();
    let scss_path = current_file
        .parent()
        .ok_or_else(|| {
            syn::Error::new(
                scss_path_span,
                format!("Failed to resolve the parent folder of the current file {current_file:?}"),
            )
        })?
        .join(scss_path.value());
    try_import_style_classes(scss_path, scss_path_span)
        .map_err(|error| syn::Error::new(scss_path_span, error.to_string()))
}

fn current_file(scss_path: &syn::LitStr) -> Result<PathBuf, syn::Error> {
    scss_path.span().unwrap().local_file().ok_or_else(|| {
        syn::Error::new(
            scss_path.span(),
            "The first token's span did not have a local file",
        )
    })
}

fn try_import_style_classes(
    file_path: PathBuf,
    identifier_span: Span,
) -> Result<TokenStream, ImportStyleError> {
    let file_content = std::fs::read_to_string(&file_path)
        .map_err(|error| ImportStyleError::ReadFileError(file_path.clone(), error))?;
    let hasher = ClassNameHasher::new(&file_path, &file_content, true);
    let hasher_debug = ClassNameHasher::new(&file_path, &file_content, true);
    let output_fields = terrazzo_css_shared::list_classes(&file_content)?.map(|class| {
        let class_ident = Ident::new(&class.to_shouty_snake_case(), identifier_span);
        let class_str = hasher.hash(class);
        let class_str_debug = hasher_debug.hash(class);
        quote_spanned!(identifier_span =>
            #[cfg(not(feature = "debug"))]
            pub const #class_ident: &str = #class_str;

            #[cfg(feature = "debug")]
            pub const #class_ident: &str = #class_str_debug;
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
    ScssError(#[from] ScssError),
}
