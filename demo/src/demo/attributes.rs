use diagnostics::info;
use nameth::nameth;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::select;
use terrazzo::widgets::select::SelectPtr;
use web_sys::MouseEvent;

stylance::import_style!(style, "attributes.scss");

#[autoclone]
#[template(tag = div)]
#[html]
pub fn attributes_demo() -> XElement {
    let underline = XSignal::new("underline", false);
    let italic = XSignal::new("bold", false);
    let bold = XSignal::new("bold", false);
    let select = SelectPtr::new(
        vec![
            Flavor::Zero,
            Flavor::BoldS_UnderlineS_ItalicS_Style,
            Flavor::BoldD_UnderlineD_ItalicD_Class,
            Flavor::BoldS_UnderlineS_ItalicS_Class,
            Flavor::BoldD_UnderlineS_ItalicS_Class,
            Flavor::BoldD_UnderlineD_ItalicS_Class,
            Flavor::BoldD_UnderlineD_ItalicD_Style,
            Flavor::BoldS_UnderlineD_ItalicD_Style,
            Flavor::BoldS_UnderlineS_ItalicD_Style,
        ],
        None,
    );
    tag(
        key = "attributes",
        h1("Attributes"),
        select.show(),
        span(
            button(
                click = move |_ev: MouseEvent| {
                    autoclone!(bold);
                    bold.update(|b| {
                        diagnostics::info!("Toggle bold to {}", !b);
                        Some(!b)
                    });
                },
                class %= move |t: XAttributeTemplate| {
                    autoclone!(bold);
                    style_tpl::active(t, bold.clone())
                },
                b("B"),
            ),
            button(
                click = move |_ev: MouseEvent| {
                    autoclone!(italic);
                    italic.update(|i| {
                        diagnostics::info!("Toggle italic to {}", !i);
                        Some(!i)
                    });
                },
                class %= move |t: XAttributeTemplate| {
                    autoclone!(italic);
                    style_tpl::active(t, italic.clone())
                },
                i("I"),
            ),
            button(
                click = move |_ev: MouseEvent| {
                    autoclone!(underline);
                    underline.update(|u| {
                        diagnostics::info!("Toggle underline to {}", !u);
                        Some(!u)
                    });
                },
                class %= move |t: XAttributeTemplate| {
                    autoclone!(underline);
                    style_tpl::active(t, underline.clone())
                },
                u("U"),
            ),
        ),
        result(select.selected.clone(), bold, underline, italic),
        before_render = |_: &Element| info!("Before render"),
        after_render = |_: &Element| info!("After render"),
    )
}

#[autoclone]
#[template(tag = div)]
#[html]
fn result(
    #[signal] flavor: Flavor,
    bold: XSignal<bool>,
    underline: XSignal<bool>,
    italic: XSignal<bool>,
) -> XElement {
    let value = match flavor {
        Flavor::Zero => div(class = style::rbox, "Hello, world! - zero"),
        Flavor::BoldS_UnderlineS_ItalicS_Style => div(
            style = BOLD,
            style = UNDERLINE,
            style = ITALIC,
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldD_UnderlineD_ItalicD_Class => div(
            class = style::rbox,
            style %= move |t: XAttributeTemplate| {
                autoclone!(bold);
                style_tpl::bold(t, bold.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(underline);
                style_tpl::underline(t, underline.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(italic);
                style_tpl::italic(t, italic.clone())
            },
            "{flavor:?}",
        ),
        Flavor::BoldS_UnderlineS_ItalicS_Class => div(
            class = style::rbox,
            style = BOLD,
            style = UNDERLINE,
            style = ITALIC,
            "{flavor:?}",
        ),
        Flavor::BoldD_UnderlineS_ItalicS_Class => div(
            class = style::rbox,
            style %= move |t: XAttributeTemplate| {
                autoclone!(bold);
                style_tpl::bold(t, bold.clone())
            },
            style = UNDERLINE,
            style = ITALIC,
            "{flavor:?}",
        ),
        Flavor::BoldD_UnderlineD_ItalicS_Class => div(
            class = style::rbox,
            style %= move |t: XAttributeTemplate| {
                autoclone!(bold);
                style_tpl::bold(t, bold.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(underline);
                style_tpl::underline(t, underline.clone())
            },
            style = ITALIC,
            "{flavor:?}",
        ),
        Flavor::BoldD_UnderlineD_ItalicD_Style => div(
            class = style::rbox,
            style %= move |t: XAttributeTemplate| {
                autoclone!(bold);
                style_tpl::bold(t, bold.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(underline);
                style_tpl::underline(t, underline.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(italic);
                style_tpl::italic(t, italic.clone())
            },
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldS_UnderlineD_ItalicD_Style => div(
            class = style::rbox,
            style = BOLD,
            style %= move |t: XAttributeTemplate| {
                autoclone!(underline);
                style_tpl::underline(t, underline.clone())
            },
            style %= move |t: XAttributeTemplate| {
                autoclone!(italic);
                style_tpl::italic(t, italic.clone())
            },
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldS_UnderlineS_ItalicD_Style => div(
            class = style::rbox,
            style = BOLD,
            style = UNDERLINE,
            style %= move |t: XAttributeTemplate| {
                autoclone!(italic);
                style_tpl::italic(t, italic.clone())
            },
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
    };
    tag([value]..)
}

static MARGIN: &str = "margin: 5px 0 5px 0;";
static PADDING: &str = "padding: 5px;";
static BORDER: &str = "border: 1px solid green;";
static BOLD: &str = "font-weight: bold;";
static ITALIC: &str = "font-style: italic;";
static UNDERLINE: &str = "text-decoration: underline;";

#[nameth]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types)]
enum Flavor {
    Zero,

    BoldS_UnderlineS_ItalicS_Style,
    BoldD_UnderlineD_ItalicD_Class,

    BoldS_UnderlineS_ItalicS_Class,
    BoldD_UnderlineS_ItalicS_Class,
    BoldD_UnderlineD_ItalicS_Class,

    BoldD_UnderlineD_ItalicD_Style,
    BoldS_UnderlineD_ItalicD_Style,
    BoldS_UnderlineS_ItalicD_Style,
}

impl select::Option for Flavor {
    #[html]
    fn show(&self) -> XElement {
        let name = nameth::NamedEnumValues::name(self);
        span("{name}")
    }

    fn name(&self) -> XString {
        nameth::NamedEnumValues::name(self).into()
    }
}

mod style_tpl {
    use terrazzo::prelude::*;
    use terrazzo::template;

    use super::BOLD;
    use super::ITALIC;
    use super::UNDERLINE;
    use super::style;

    #[template]
    pub fn bold(#[signal] mut bold: bool) -> XAttributeValue {
        bold.then_some(BOLD)
    }

    #[template]
    pub fn italic(#[signal] mut italic: bool) -> XAttributeValue {
        italic.then_some(ITALIC)
    }

    #[template]
    pub fn underline(#[signal] mut underline: bool) -> XAttributeValue {
        underline.then_some(UNDERLINE)
    }

    #[template]
    pub fn active(#[signal] mut active: bool) -> XAttributeValue {
        active.then_some(style::active)
    }
}
