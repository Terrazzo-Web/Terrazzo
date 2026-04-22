use std::path::PathBuf;

use heck::ToShoutySnakeCase;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use proc_macro2::Span;
use quote::ToTokens;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;
use terrazzo_css_shared::ScssError;

#[proc_macro]
pub fn import_style(input: TokenStream) -> TokenStream {
    match try_import_style(input) {
        Ok(ok) => ok,
        Err(error) if std::env::var("RUST_ANALYZER_INTERNALS_DO_NOT_USE").is_err() => {
            error.into_compile_error().into()
        }
        Err(_) => TokenStream::new(),
    }
}

fn try_import_style(input: TokenStream) -> Result<TokenStream, syn::Error> {
    let current_file = current_file(&input)?;
    let input = proc_macro2::TokenStream::from(input);
    let input: syn::ExprTuple = syn::parse2(quote! ( (#input) )).unwrap();

    let ident = &input.elems[0];
    let ident = if let syn::Expr::Path(path) = ident
        && let Some(ident) = path.path.get_ident()
    {
        ident
    } else {
        return Err(syn::Error::new(
            ident.span(),
            format!("Expected a module name, got {}", ident.into_token_stream()),
        ));
    };

    let scss_path = &input.elems[1];
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(scss_path),
        ..
    }) = scss_path
    else {
        return Err(syn::Error::new(
            scss_path.span(),
            format!(
                "Expected a string literal for the SCSS file path, got {}",
                scss_path.into_token_stream()
            ),
        ));
    };

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
    try_import_style_classes(ident, scss_path, scss_path_span)
        .map_err(|error| syn::Error::new_spanned(&input, error.to_string()))
}

fn current_file(input: &TokenStream) -> Result<PathBuf, syn::Error> {
    let first_token = input.clone().into_iter().next().ok_or_else(|| {
        syn::Error::new_spanned(
            proc_macro2::TokenStream::from(input.clone()),
            "Failed to get the first token to resolve the current file",
        )
    })?;
    first_token.span().local_file().ok_or_else(|| {
        syn::Error::new_spanned(
            proc_macro2::TokenStream::from(input.clone()),
            "The first token's span did not have a local file",
        )
    })
}

fn try_import_style_classes(
    ident: &syn::Ident,
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

    Ok(quote! {
        mod #ident {
            #(#output_fields)*
        }
    }
    .into())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
enum ImportStyleError {
    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    ReadFileError(PathBuf, std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    ScssError(#[from] ScssError),
}
