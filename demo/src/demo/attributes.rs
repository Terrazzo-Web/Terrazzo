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
            Flavor::BoldS_ItalicS_UnderlineS_Class,
            Flavor::BoldD_ItalicS_UnderlineS_Class,
            Flavor::BoldD_ItalicD_UnderlineS_Class,
            Flavor::BoldD_ItalicD_UnderlineD_Class,
            Flavor::BoldS_ItalicS_UnderlineS_Style,
            Flavor::BoldD_ItalicS_UnderlineS_Style,
            Flavor::BoldD_ItalicD_UnderlineS_Style,
            Flavor::BoldD_ItalicD_UnderlineD_Style,
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
                class %= style_tpl::active(bold.clone()),
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
                class %= style_tpl::active(italic.clone()),
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
                class %= style_tpl::active(underline.clone()),
                u("U"),
            ),
        ),
        result(select.selected.clone(), bold, underline, italic),
        before_render = |_: &Element| info!("Before render"),
        after_render = |_: &Element| info!("After render"),
    )
}

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
        Flavor::BoldS_ItalicS_UnderlineS_Class => div(
            style = BOLD,
            style = ITALIC,
            style = UNDERLINE,
            class = style::rbox,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicS_UnderlineS_Class => div(
            style %= style_tpl::bold(bold.clone()),
            style = ITALIC,
            style = UNDERLINE,
            class = style::rbox,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicD_UnderlineS_Class => div(
            style %= style_tpl::bold(bold.clone()),
            style %= style_tpl::italic(italic.clone()),
            style = UNDERLINE,
            class = style::rbox,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicD_UnderlineD_Class => div(
            style %= style_tpl::bold(bold.clone()),
            style %= style_tpl::italic(italic.clone()),
            style %= style_tpl::underline(underline.clone()),
            class = style::rbox,
            "{flavor:?}",
        ),
        Flavor::BoldS_ItalicS_UnderlineS_Style => div(
            style = BOLD,
            style = ITALIC,
            style = UNDERLINE,
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicS_UnderlineS_Style => div(
            style %= style_tpl::bold(bold.clone()),
            style = ITALIC,
            style = UNDERLINE,
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicD_UnderlineS_Style => div(
            style %= style_tpl::bold(bold.clone()),
            style %= style_tpl::italic(italic.clone()),
            style = UNDERLINE,
            style = MARGIN,
            style = PADDING,
            style = BORDER,
            "{flavor:?}",
        ),
        Flavor::BoldD_ItalicD_UnderlineD_Style => div(
            style %= style_tpl::bold(bold.clone()),
            style %= style_tpl::italic(italic.clone()),
            style %= style_tpl::underline(underline.clone()),
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

    BoldS_ItalicS_UnderlineS_Class,
    BoldD_ItalicS_UnderlineS_Class,
    BoldD_ItalicD_UnderlineS_Class,
    BoldD_ItalicD_UnderlineD_Class,

    BoldS_ItalicS_UnderlineS_Style,
    BoldD_ItalicS_UnderlineS_Style,
    BoldD_ItalicD_UnderlineS_Style,
    BoldD_ItalicD_UnderlineD_Style,
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

    #[template(wrap = true)]
    pub fn bold(#[signal] mut bold: bool) -> XAttributeValue {
        bold.then_some(BOLD)
    }

    #[template(wrap = true)]
    pub fn italic(#[signal] mut italic: bool) -> XAttributeValue {
        italic.then_some(ITALIC)
    }

    #[template(wrap = true)]
    pub fn underline(#[signal] mut underline: bool) -> XAttributeValue {
        underline.then_some(UNDERLINE)
    }

    #[template(wrap = true)]
    pub fn active(#[signal] mut active: bool) -> XAttributeValue {
        active.then_some(style::active)
    }
}
