use std::path::PathBuf;

use heck::ToShoutySnakeCase;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use proc_macro::TokenStream;
use proc_macro2::Ident;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use syn::spanned::Spanned;
use terrazzo_css_shared::CssError;

#[proc_macro]
pub fn import_style(input: TokenStream) -> TokenStream {
    let local_file = input
        .clone()
        .into_iter()
        .next()
        .expect("first token")
        .span()
        .local_file()
        .expect("local_file");
    let input = proc_macro2::TokenStream::from(input);
    let input: syn::ExprTuple = syn::parse2(quote! ( (#input) )).unwrap();

    let ident = if let syn::Expr::Path(path) = &input.elems[0]
        && let Some(ident) = path.path.get_ident()
    {
        ident
    } else {
        let span = input.elems[0].span();
        return quote_spanned! {span=> compile_error!("Expected a module name") }.into();
    };

    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(file_path),
        ..
    }) = &input.elems[1]
    else {
        let span = input.elems[1].span();
        return quote_spanned! {span=> compile_error!("Expected a string literal") }.into();
    };

    let identifier_span = file_path.span();
    let file_path = local_file.parent().unwrap().join(file_path.value());
    match try_import_style_classes(ident, file_path, identifier_span) {
        Ok(ts) => ts,
        Err(err) => syn::Error::new_spanned(&input, err.to_string())
            .to_compile_error()
            .into(),
    }
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
    CssError(#[from] CssError),
}
