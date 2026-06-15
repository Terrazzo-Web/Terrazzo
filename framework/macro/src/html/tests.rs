#![cfg(test)]

use quote::quote;

use super::html;
use crate::item_to_string;

#[test]
fn simple_node() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(key = "root", "Root text", #[cfg(test)] "Only in test")
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        #[cfg(test)]
        __gen_children.push(XNode::from(XText(format!("Only in test").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn child() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            return html::div(
                key = "root",
                "Root text",
                span(key = "inner", "Paragraph 1", "Paragraph 2"),
            );
        }
    };
    let expected = r#"
fn sample() -> XElement {
    return {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        __gen_children
            .push(
                XNode::from({
                    let __gen_attributes = vec![];
                    let mut __gen_children = vec![];
                    __gen_children.push(XNode::from(XText(format!("Paragraph 1").into())));
                    __gen_children.push(XNode::from(XText(format!("Paragraph 2").into())));
                    XElement {
                        tag_name: Some("span".into()),
                        key: XKey::Named("inner".into()),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn children() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            let child1 = span("Child1");
            let child2 = span("Child2");
            let children = [child1, child2];
            return html::div(
                key = "root",
                "Root text",
                #[cfg(prod)]
                children..,
                #[cfg(non-prod)]
                children..,
            );
        }
    };
    let expected = r#"
fn sample() -> XElement {
    let child1 = {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Child1").into())));
        XElement {
            tag_name: Some("span".into()),
            key: XKey::default(),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
    let child2 = {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Child2").into())));
        XElement {
            tag_name: Some("span".into()),
            key: XKey::default(),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
    let children = [child1, child2];
    return {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        #[cfg(prod)] __gen_children.extend(children.into_iter().map(XNode::from));
        #[cfg(non-prod)] __gen_children.extend(children.into_iter().map(XNode::from));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn invalid_child() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            let someNode = someNode()..;
            div(key = "root", "Text", someNode)
        }
    };
    let expected = r#"
fn sample() -> XElement {
    let someNode = someNode()..;
    {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Text").into())));
        __gen_children.push(XNode::from(someNode));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn attribute() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                class = "base",
                style = format!("width: {}%", 100),
                div(
                    class = "child",
                    style = format!("width: {}%", 50),
                ),
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "base".into(),
                    }
                });
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 1usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "style".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: format!("width: {}%", 100).into(),
                    }
                });
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        __gen_children
            .push(
                XNode::from({
                    let mut __gen_attributes = vec![];
                    {
                        let mut __attribute_index = usize::MAX;
                        let mut __attribute_sub_index = 0;
                        __gen_attributes
                            .push({
                                if __attribute_index != usize::MAX {
                                    if 0usize != __attribute_index {
                                        __attribute_index += 1;
                                        __attribute_sub_index = 0;
                                    } else {
                                        __attribute_sub_index += 1;
                                    }
                                } else {
                                    __attribute_index = 0;
                                }
                                XAttribute {
                                    id: XAttributeId {
                                        name: XAttributeName {
                                            name: "class".into(),
                                            kind: XAttributeKind::Attribute,
                                        },
                                        index: __attribute_index,
                                        sub_index: __attribute_sub_index,
                                    },
                                    value: "child".into(),
                                }
                            });
                        __gen_attributes
                            .push({
                                if __attribute_index != usize::MAX {
                                    if 1usize != __attribute_index {
                                        __attribute_index += 1;
                                        __attribute_sub_index = 0;
                                    } else {
                                        __attribute_sub_index += 1;
                                    }
                                } else {
                                    __attribute_index = 0;
                                }
                                XAttribute {
                                    id: XAttributeId {
                                        name: XAttributeName {
                                            name: "style".into(),
                                            kind: XAttributeKind::Attribute,
                                        },
                                        index: __attribute_index,
                                        sub_index: __attribute_sub_index,
                                    },
                                    value: format!("width: {}%", 50).into(),
                                }
                            });
                    }
                    let __gen_children = vec![];
                    XElement {
                        tag_name: Some("div".into()),
                        key: XKey::default(),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn attribute_with_attr() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                #[cfg(feature = "prod")]
                #[cfg(not(test))]
                class = "base",
                #[cfg(feature = "prod")]
                style = format!("width: {}%", 100),
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            #[cfg(feature = "prod")] #[cfg(not(test))]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "base".into(),
                    }
                });
            #[cfg(feature = "prod")]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 1usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "style".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: format!("width: {}%", 100).into(),
                    }
                });
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn optional_attribute() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                class = "base",
                #[cfg(feature = "prod")]
                style |= Some(format!("width: {}%", 100)),
                #[cfg(feature = "prod")]
                data_custom |= Some("custom attribute"),
                #[cfg(feature = "prod")]
                data_custom |= if true { Some("y") } else { None },
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "base".into(),
                    }
                });
            #[cfg(feature = "prod")]
            if let Some(value) = Some("custom attribute") {
                __gen_attributes
                    .push({
                        if __attribute_index != usize::MAX {
                            if 1usize != __attribute_index {
                                __attribute_index += 1;
                                __attribute_sub_index = 0;
                            } else {
                                __attribute_sub_index += 1;
                            }
                        } else {
                            __attribute_index = 0;
                        }
                        XAttribute {
                            id: XAttributeId {
                                name: XAttributeName {
                                    name: "data-custom".into(),
                                    kind: XAttributeKind::Attribute,
                                },
                                index: __attribute_index,
                                sub_index: __attribute_sub_index,
                            },
                            value: value.into(),
                        }
                    });
            }
            #[cfg(feature = "prod")]
            if let Some(value) = if true { Some("y") } else { None } {
                __gen_attributes
                    .push({
                        if __attribute_index != usize::MAX {
                            if 1usize != __attribute_index {
                                __attribute_index += 1;
                                __attribute_sub_index = 0;
                            } else {
                                __attribute_sub_index += 1;
                            }
                        } else {
                            __attribute_index = 0;
                        }
                        XAttribute {
                            id: XAttributeId {
                                name: XAttributeName {
                                    name: "data-custom".into(),
                                    kind: XAttributeKind::Attribute,
                                },
                                index: __attribute_index,
                                sub_index: __attribute_sub_index,
                            },
                            value: value.into(),
                        }
                    });
            }
            #[cfg(feature = "prod")]
            if let Some(value) = Some(format!("width: {}%", 100)) {
                __gen_attributes
                    .push({
                        if __attribute_index != usize::MAX {
                            if 2usize != __attribute_index {
                                __attribute_index += 1;
                                __attribute_sub_index = 0;
                            } else {
                                __attribute_sub_index += 1;
                            }
                        } else {
                            __attribute_index = 0;
                        }
                        XAttribute {
                            id: XAttributeId {
                                name: XAttributeName {
                                    name: "style".into(),
                                    kind: XAttributeKind::Attribute,
                                },
                                index: __attribute_index,
                                sub_index: __attribute_sub_index,
                            },
                            value: value.into(),
                        }
                    });
            }
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn style_attribute() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                class = "base",
                #[cfg(style)]
                style::width = format!("{}%", 100),
                #[cfg(optional style)]
                style::height |= Some(format!("{}px", 250)),
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "base".into(),
                    }
                });
            #[cfg(optional style)]
            if let Some(value) = Some(format!("{}px", 250)) {
                __gen_attributes
                    .push({
                        if __attribute_index != usize::MAX {
                            if 1usize != __attribute_index {
                                __attribute_index += 1;
                                __attribute_sub_index = 0;
                            } else {
                                __attribute_sub_index += 1;
                            }
                        } else {
                            __attribute_index = 0;
                        }
                        XAttribute {
                            id: XAttributeId {
                                name: XAttributeName {
                                    name: "height".into(),
                                    kind: XAttributeKind::Style,
                                },
                                index: __attribute_index,
                                sub_index: __attribute_sub_index,
                            },
                            value: value.into(),
                        }
                    });
            }
            #[cfg(style)]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 2usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "width".into(),
                                kind: XAttributeKind::Style,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: format!("{}%", 100).into(),
                    }
                });
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn dynamic_attribute() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                #[cfg(dynamic)]
                class %= move |t| { make_class() },
                #[cfg(dynamic style)]
                style::width %= move |t| { make_width() },
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            #[cfg(dynamic)]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: XAttributeValue::Dynamic(
                            (move |t| { make_class() }).into(),
                        ),
                    }
                });
            #[cfg(dynamic style)]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 1usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "width".into(),
                                kind: XAttributeKind::Style,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: XAttributeValue::Dynamic(
                            (move |t| { make_width() }).into(),
                        ),
                    }
                });
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn multi_attribute() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                key = "root",
                "Root text",
                class = "base",
                #[cfg(additional class)]
                class = "additional",
                style = format!("width: {}%", 100),
                #[cfg(additional style)]
                style = format!("height: {}%", 200),
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let mut __gen_attributes = vec![];
        {
            let mut __attribute_index = usize::MAX;
            let mut __attribute_sub_index = 0;
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "base".into(),
                    }
                });
            #[cfg(additional class)]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 0usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "class".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: "additional".into(),
                    }
                });
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 1usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "style".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: format!("width: {}%", 100).into(),
                    }
                });
            #[cfg(additional style)]
            __gen_attributes
                .push({
                    if __attribute_index != usize::MAX {
                        if 1usize != __attribute_index {
                            __attribute_index += 1;
                            __attribute_sub_index = 0;
                        } else {
                            __attribute_sub_index += 1;
                        }
                    } else {
                        __attribute_index = 0;
                    }
                    XAttribute {
                        id: XAttributeId {
                            name: XAttributeName {
                                name: "style".into(),
                                kind: XAttributeKind::Attribute,
                            },
                            index: __attribute_index,
                            sub_index: __attribute_sub_index,
                        },
                        value: format!("height: {}%", 200).into(),
                    }
                });
        }
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn index_keys() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            return html::div(span("Paragraph 1"), span("Paragraph 2"));
        }
    };
    let expected = r#"
fn sample() -> XElement {
    return {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children
            .push(
                XNode::from({
                    let __gen_attributes = vec![];
                    let mut __gen_children = vec![];
                    __gen_children.push(XNode::from(XText(format!("Paragraph 1").into())));
                    XElement {
                        tag_name: Some("span".into()),
                        key: XKey::default(),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        __gen_children
            .push(
                XNode::from({
                    let __gen_attributes = vec![];
                    let mut __gen_children = vec![];
                    __gen_children.push(XNode::from(XText(format!("Paragraph 2").into())));
                    XElement {
                        tag_name: Some("span".into()),
                        key: XKey::default(),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        XElement {
            tag_name: Some("div".into()),
            key: XKey::default(),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn dynamic() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(|element| do_template(element))
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        XElement {
            tag_name: Some("div".into()),
            key: XKey::default(),
            value: XElementValue::Dynamic((|element| do_template(element)).into()),
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn dynamic_duplicate_callback() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(
                |element| do_template(element),
                |element| do_template(element),
            )
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        XElement {
            tag_name: Some("div".into()),
            key: XKey::default(),
            value: compile_error!("Dynamic nodes have a single callback"),
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn dynamic_invalid() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div(class = "stylish", |element| do_template(element))
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        compile_error!("Properties of dynamic nodes cannot be defined at the call site");
        XElement {
            tag_name: Some("div".into()),
            key: XKey::default(),
            value: XElementValue::Dynamic((|element| do_template(element)).into()),
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn child_macros() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            return div! {
                key = "root",
                "Root text",
                span! {
                    key = "inner",
                    "Paragraph 1",
                    "Paragraph 2",
                },
            };
        }
    };
    let expected = r#"
fn sample() -> XElement {
    return {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        __gen_children
            .push(
                XNode::from({
                    let __gen_attributes = vec![];
                    let mut __gen_children = vec![];
                    __gen_children.push(XNode::from(XText(format!("Paragraph 1").into())));
                    __gen_children.push(XNode::from(XText(format!("Paragraph 2").into())));
                    XElement {
                        tag_name: Some("span".into()),
                        key: XKey::Named("inner".into()),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    };
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn child_macros2() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            div! {
                key = "root",
                "Root text",
                span! {
                    key = "inner",
                    "Paragraph 1",
                    "Paragraph 2",
                },
            }
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        __gen_children
            .push(
                XNode::from({
                    let __gen_attributes = vec![];
                    let mut __gen_children = vec![];
                    __gen_children.push(XNode::from(XText(format!("Paragraph 1").into())));
                    __gen_children.push(XNode::from(XText(format!("Paragraph 2").into())));
                    XElement {
                        tag_name: Some("span".into()),
                        key: XKey::Named("inner".into()),
                        value: XElementValue::Static {
                            attributes: __gen_attributes,
                            children: __gen_children,
                            events: vec![],
                        },
                        before_render: None,
                        after_render: None,
                    }
                }),
            );
        XElement {
            tag_name: Some("div".into()),
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}

#[test]
fn tag() -> syn::Result<()> {
    let sample = quote! {
        fn sample() -> XElement {
            tag(key = "root", "Root text")
        }
    };
    let expected = r#"
fn sample() -> XElement {
    {
        let __gen_attributes = vec![];
        let mut __gen_children = vec![];
        __gen_children.push(XNode::from(XText(format!("Root text").into())));
        XElement {
            tag_name: None,
            key: XKey::Named("root".into()),
            value: XElementValue::Static {
                attributes: __gen_attributes,
                children: __gen_children,
                events: vec![],
            },
            before_render: None,
            after_render: None,
        }
    }
}"#;
    let actual = html(quote! {}, sample)?;
    let actual = item_to_string(&syn::parse2(actual)?);
    if expected.trim() != actual.trim() {
        println!("{}", actual);
        panic!();
    }
    Ok(())
}
