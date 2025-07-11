use terrazzo_client::prelude::*;
use terrazzo_macro::html;
use terrazzo_macro::template;
use web_sys::MouseEvent;

stylance::import_crate_style!(style, "src/client_impl/widgets/link.scss");

/// A clickable link with a styled underline effect.
#[html]
#[template(tag = span)]
pub fn link<C, CI>(
    click: impl Fn(MouseEvent) + Clone + 'static,
    content: impl FnOnce() -> CI + Clone + 'static,
) -> XElement
where
    XNode: From<C>,
    CI: IntoIterator<Item = C>,
{
    tag! {
        class = style::link,
        click = click,
        content()..
    }
}
