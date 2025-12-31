use diagnostics::info;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::element_capture::ElementCapture;
use web_sys::HtmlSelectElement;
use web_sys::MouseEvent;

stylance::import_style!(style, "attributes.scss");

#[autoclone]
#[template(tag = div)]
#[html]
pub fn attributes_demo() -> XElement {
    let flavor = XSignal::new("flavor", Flavor::Zero);
    let underline = XSignal::new("underline", false);
    let italic = XSignal::new("bold", false);
    let bold = XSignal::new("bold", false);
    let flavor_dom: ElementCapture<HtmlSelectElement> = ElementCapture::default();
    div(
        key = "attributes",
        h1("Attributes"),
        select(
            before_render = flavor_dom.capture(),
            option(value = "Zero", "Zero"),
            option(value = "DynamicAndStatic", "DynamicAndStatic"),
            option(value = "DynamicOnly", "DynamicOnly"),
            change = move |_| {
                autoclone!(flavor_dom, flavor);
                match flavor_dom.get().value().as_str() {
                    "Zero" => flavor.set(Flavor::Zero),
                    "DynamicAndStatic" => flavor.set(Flavor::DynamicAndStatic),
                    "DynamicOnly" => flavor.set(Flavor::DynamicOnly),
                    _ => unreachable!(),
                }
            },
        ),
        span(
            button(
                click = move |_ev: MouseEvent| {
                    autoclone!(bold);
                    bold.update(|b| {
                        diagnostics::info!("Toggle bold to {}", !b);
                        Some(!b)
                    });
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
                u("U"),
            ),
        ),
        result(flavor, underline, italic, bold),
        before_render = |_: &Element| info!("Before render"),
        after_render = |_: &Element| info!("After render"),
    )
}

#[template(tag = div)]
#[html]
fn result(
    #[signal] flavor: Flavor,
    underline: XSignal<bool>,
    italic: XSignal<bool>,
    bold: XSignal<bool>,
) -> XElement {
    let value = match flavor {
        Flavor::DynamicAndStatic => dynamic_and_static(underline, italic, bold),
        Flavor::DynamicOnly => dynamic_only(underline, italic, bold),
        Flavor::Zero => zero(),
    };
    tag([value]..)
}

#[autoclone]
#[html]
fn dynamic_and_static(
    underline: XSignal<bool>,
    italic: XSignal<bool>,
    bold: XSignal<bool>,
) -> XElement {
    div(
        style = "margin: 5px 0 5px 0;",
        style %= move |t: XAttributeTemplate| {
            autoclone!(bold);
            style_tpl::bold(t, bold.clone())
        },
        style = "padding: 5px;",
        style %= move |t: XAttributeTemplate| {
            autoclone!(italic);
            style_tpl::italic(t, italic.clone())
        },
        style = "border: 1px solid green;",
        style %= move |t: XAttributeTemplate| {
            autoclone!(underline);
            style_tpl::underline(t, underline.clone())
        },
        "Hello, world! dynamic and static",
    )
}

#[autoclone]
#[html]
fn dynamic_only(underline: XSignal<bool>, italic: XSignal<bool>, bold: XSignal<bool>) -> XElement {
    div(
        class = style::dynamic_only,
        style %= move |t: XAttributeTemplate| {
            autoclone!(underline);
            style_tpl::underline(t, underline.clone())
        },
        style %= move |t: XAttributeTemplate| {
            autoclone!(bold);
            style_tpl::bold(t, bold.clone())
        },
        style %= move |t: XAttributeTemplate| {
            autoclone!(italic);
            style_tpl::italic(t, italic.clone())
        },
        "Hello, world! - dynamic only",
    )
}

#[html]
fn zero() -> XElement {
    div(class = style::dynamic_only, "Hello, world! - zero")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flavor {
    DynamicAndStatic,
    DynamicOnly,
    Zero,
}

mod style_tpl {
    use terrazzo::prelude::*;
    use terrazzo::template;

    #[template]
    pub fn bold(#[signal] mut bold: bool) -> XAttributeValue {
        bold.then_some("font-weight: bold;")
    }

    #[template]
    pub fn italic(#[signal] mut italic: bool) -> XAttributeValue {
        italic.then_some("font-style: italic;")
    }

    #[template]
    pub fn underline(#[signal] mut underline: bool) -> XAttributeValue {
        underline.then_some("text-decoration: underline;")
    }
}
