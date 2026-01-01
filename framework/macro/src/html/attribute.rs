use quote::quote;

pub struct XAttribute {
    name: String,
    is_dynamic: bool,
    kind: XAttributeKind,
    generator: Option<Box<dyn FnOnce(Self) -> proc_macro2::TokenStream>>,
    index: usize,
    sub_index: usize,
}

impl XAttribute {
    pub fn new_static(
        name: &str,
        kind: XAttributeKind,
        generator: impl FnOnce(Self) -> proc_macro2::TokenStream + 'static,
    ) -> Self {
        Self {
            name: name.to_owned(),
            is_dynamic: false,
            kind,
            generator: Some(Box::new(generator)),
            index: usize::MAX,
            sub_index: usize::MAX,
        }
    }

    pub fn new_dynamic(
        name: &str,
        kind: XAttributeKind,
        generator: impl FnOnce(Self) -> proc_macro2::TokenStream + 'static,
    ) -> Self {
        Self {
            name: name.to_owned(),
            is_dynamic: true,
            kind,
            generator: Some(Box::new(generator)),
            index: usize::MAX,
            sub_index: usize::MAX,
        }
    }

    pub fn sort_attributes(attributes: &mut [Self]) {
        attributes.sort_by_key(|attribute| {
            (
                !attribute.is_dynamic,
                attribute.name.clone(),
                attribute.kind,
            )
        });
        for (index, chunk) in attributes
            .chunk_by_mut(|a, b| (&a.name, a.kind) == (&b.name, b.kind))
            .into_iter()
            .enumerate()
        {
            for (sub_index, attribute) in chunk.into_iter().enumerate() {
                attribute.index = index;
                attribute.sub_index = sub_index;
            }
        }
    }

    pub fn generate(mut self) -> proc_macro2::TokenStream {
        let generator = self.generator.take().expect("generator");
        generator(self)
    }

    pub fn to_tokens(&self, runtime_value: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        let XAttribute {
            name,
            is_dynamic: _,
            kind,
            generator: _,
            index,
            sub_index,
        } = self;
        let kind = kind.to_tokens();
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
