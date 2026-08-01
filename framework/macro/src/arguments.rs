use std::collections::HashSet;

use darling::FromMeta;
use darling::ast::NestedMeta;

#[derive(FromMeta)]
pub struct MacroArgs {
    #[darling(default)]
    pub debug: bool,

    #[darling(default)]
    pub html_tags: HashSet<syn::Ident>,

    #[darling(default)]
    pub tag: Option<syn::Ident>,

    #[darling(
        default,
        with = darling::util::parse_expr::preserve_str_literal,
        map = Some
    )]
    pub key: Option<syn::Expr>,

    #[darling(default)]
    pub wrap: bool,
}

impl MacroArgs {
    pub fn parse2(attr: proc_macro2::TokenStream) -> syn::Result<MacroArgs> {
        let nested = NestedMeta::parse_meta_list(attr)?;
        let mut args = MacroArgs::from_list(&nested)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error))?;
        if args.html_tags.is_empty() {
            args.html_tags.extend(well_known_tags());
        }
        return Ok(args);
    }
}

fn well_known_tags() -> HashSet<syn::Ident> {
    [
        ["tag"].as_slice(),
        &["a", "abbr", "address", "area", "article", "aside", "audio"],
        &["b", "base", "bdi", "bdo", "blockquote", "body", "br"],
        &["button", "canvas", "caption", "cite", "code", "col"],
        &["colgroup", "data", "datalist", "dd", "del", "details"],
        &["dfn", "dialog", "div", "dl", "dt", "em", "embed"],
        &["fieldset", "figcaption", "figure", "footer", "form"],
        &["h1", "h2", "h3", "h4", "h5", "h6", "head", "header"],
        &["hgroup", "hr", "html", "i", "iframe", "img", "input", "ins"],
        &["kbd", "label", "legend", "li", "link", "main", "map"],
        &["mark", "meta", "meter", "nav", "noscript", "object", "ol"],
        &["optgroup", "option", "output", "p", "param", "picture"],
        &["pre", "progress", "q", "rp", "rt", "ruby", "s", "samp"],
        &["script", "section", "select", "small", "source", "span"],
        &["strong", "style", "sub", "summary", "sup", "svg", "table"],
        &["tbody", "td", "template", "textarea", "tfoot", "th"],
        &["thead", "time", "title", "tr", "track", "u", "ul", "var"],
        &["video", "wbr"],
    ]
    .into_iter()
    .flatten()
    .map(|tag| syn::Ident::new(tag, proc_macro2::Span::call_site()))
    .inspect({
        // Check for duplicates
        let mut tags = HashSet::new();
        move |tag| assert!(tags.insert(tag.clone()))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::MacroArgs;

    #[test]
    fn preserves_string_literal_expressions() -> syn::Result<()> {
        let args = MacroArgs::parse2(quote! { key = "side-view" })?;
        let key = args.key.unwrap();
        assert_eq!(
            quote! { #key }.to_string(),
            quote! { "side-view" }.to_string()
        );
        Ok(())
    }
}
