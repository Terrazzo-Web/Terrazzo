use std::collections::HashSet;

use quote::quote;

pub struct XAttribute {
    name: String,
    is_dynamic: bool,
    kind: XAttributeKind,
    generator: Option<Box<dyn FnOnce(Self) -> proc_macro2::TokenStream>>,
    pub post_increment_index: bool,
    pub post_increment_sub_index: bool,
    pub pre_reset_sub_index: bool,
    attrs: Vec<syn::Attribute>,
}

impl XAttribute {
    pub fn new_static(
        name: &str,
        kind: XAttributeKind,
        generator: impl FnOnce(Self) -> proc_macro2::TokenStream + 'static,
        attrs: &[syn::Attribute],
    ) -> Self {
        Self {
            name: name.to_owned(),
            is_dynamic: false,
            kind,
            generator: Some(Box::new(generator)),
            post_increment_index: false,
            post_increment_sub_index: false,
            pre_reset_sub_index: false,
            attrs: attrs.to_vec(),
        }
    }

    pub fn new_dynamic(
        name: &str,
        kind: XAttributeKind,
        generator: impl FnOnce(Self) -> proc_macro2::TokenStream + 'static,
        attrs: &[syn::Attribute],
    ) -> Self {
        Self {
            name: name.to_owned(),
            is_dynamic: true,
            kind,
            generator: Some(Box::new(generator)),
            post_increment_index: false,
            post_increment_sub_index: false,
            pre_reset_sub_index: false,
            attrs: attrs.to_vec(),
        }
    }

    pub fn sort_attributes(attributes: &mut [Self]) {
        let dynamic_chunks = attributes
            .iter()
            .filter(|&attribute| attribute.is_dynamic)
            .map(|attribute| (attribute.name.clone(), attribute.kind))
            .collect::<HashSet<_>>();
        attributes.sort_by_cached_key(|attribute| {
            (
                !dynamic_chunks.contains(&(attribute.name.clone(), attribute.kind)),
                attribute.name.clone(),
                attribute.kind,
            )
        });
        let attributes_len = attributes.len();
        let mut prev_group_is_composite = false;
        for (index, chunk) in attributes
            .chunk_by_mut(|a, b| (&a.name, a.kind) == (&b.name, b.kind))
            .enumerate()
        {
            let chunk_len = chunk.len();
            assert!(chunk_len != 0, "Empty attribute group");
            for (sub_index, attribute) in chunk.iter_mut().enumerate() {
                attribute.post_increment_index =
                    index != attributes_len - 1 && sub_index == chunk_len - 1;
                attribute.post_increment_sub_index = sub_index != chunk_len - 1;
                attribute.pre_reset_sub_index = sub_index == 0 && prev_group_is_composite
            }
            prev_group_is_composite = chunk_len != 1;
        }
    }

    pub fn generate(mut self) -> proc_macro2::TokenStream {
        let generator = self.generator.take().expect("generator");
        generator(self)
    }

    pub fn has_attrs(&self) -> bool {
        !self.attrs.is_empty()
    }

    pub fn to_tokens(&self, runtime_value: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let XAttribute {
            name,
            is_dynamic: _,
            kind,
            generator: _,
            post_increment_index,
            post_increment_sub_index,
            pre_reset_sub_index,
            attrs: _,
        } = self;
        let kind = kind.to_tokens();
        let index = self.index_tokens(*post_increment_index);
        let sub_index = self.sub_index_tokens(*post_increment_sub_index, *pre_reset_sub_index);
        quote! {
            XAttribute {
                id: XAttributeId {
                    name: XAttributeName {
                        name: #name.into(),
                        kind: #kind,
                    },
                    index: #index,
                    sub_index: #sub_index,
                },
                value: #runtime_value,
            }
        }
    }

    pub fn to_guarded_tokens(
        &self,
        runtime_value: proc_macro2::TokenStream,
        push_attribute: impl FnOnce(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    ) -> proc_macro2::TokenStream {
        let XAttribute {
            name,
            is_dynamic: _,
            kind,
            generator: _,
            post_increment_index,
            post_increment_sub_index,
            pre_reset_sub_index,
            attrs,
        } = self;
        let kind = kind.to_tokens();
        let index = self.index_tokens(*post_increment_index);
        let sub_index = self.sub_index_tokens(*post_increment_sub_index, *pre_reset_sub_index);
        let generated = quote! {
            XAttribute {
                id: XAttributeId {
                    name: XAttributeName {
                        name: #name.into(),
                        kind: #kind,
                    },
                    index: attribute_id.0,
                    sub_index: attribute_id.1,
                },
                value: #runtime_value,
            }
        };
        let push_attribute = push_attribute(generated);
        quote! {
            #(#attrs)*
            {
                let attribute_id = (#index, #sub_index);
                #push_attribute
            }
        }
    }

    fn index_tokens(&self, post_increment_index: bool) -> proc_macro2::TokenStream {
        if post_increment_index {
            quote! {
                {
                    let i = attribute_index;
                    attribute_index += 1;
                    i
                }
            }
        } else {
            quote! { attribute_index }
        }
    }

    fn sub_index_tokens(
        &self,
        post_increment_sub_index: bool,
        pre_reset_sub_index: bool,
    ) -> proc_macro2::TokenStream {
        match (post_increment_sub_index, pre_reset_sub_index) {
            (true, true) => quote! {
                {
                    // post_increment_sub_index, pre_reset_sub_index
                    attribute_sub_index = 1;
                    0
                }
            },
            (true, false) => quote! {
                {
                    // post_increment_sub_index
                    let i = attribute_sub_index;
                    attribute_sub_index += 1;
                    i
                }
            },
            (false, true) => quote! {
                {
                    // pre_reset_sub_index
                    attribute_sub_index = 0;
                    0
                }
            },
            (false, false) => quote! {
                // None of post_increment_sub_index, pre_reset_sub_index
                attribute_sub_index
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum XAttributeKind {
    Attribute,
    Style,
}

impl XAttributeKind {
    fn to_tokens(self) -> proc_macro2::TokenStream {
        match self {
            XAttributeKind::Attribute => quote! { XAttributeKind::Attribute },
            XAttributeKind::Style => quote! { XAttributeKind::Style },
        }
    }
}
