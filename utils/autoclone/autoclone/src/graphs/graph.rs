use std::collections::BTreeMap;
use std::rc::Rc;

use quote::ToTokens as _;

use super::function::Function;

pub struct Graph {
    pub module: syn::ItemMod,
    pub functions: BTreeMap<syn::Ident, Rc<Function>>,
}

impl Graph {
    pub fn new(item: proc_macro2::TokenStream) -> Result<Self, syn::Error> {
        Ok(Self {
            module: syn::parse2(item)?,
            functions: Default::default(),
        })
    }

    pub fn record_functions(&mut self) {
        let Some((_, content)) = &mut self.module.content else {
            return;
        };
        let functions = content
            .extract_if(0.., |item| matches!(item, syn::Item::Fn { .. }))
            .collect::<Vec<_>>();
        for item in functions {
            let syn::Item::Fn(func) = item else { continue };
            let function = Function::new(&func);
            self.functions
                .insert(func.sig.ident.clone(), function.into());
        }
        for function in self.functions.values() {
            self.parse_params(function)
        }
    }

    fn parse_params(&self, function: &Rc<Function>) {
        for param in &function.definition.sig.inputs {
            function.parse_param(self, param)
        }
    }

    pub fn process_functions(&mut self) {
        for function in &mut self.functions.values() {
            let functions = function.process(self);
            let Some((_, content)) = &mut self.module.content else {
                return;
            };
            for function in functions {
                content.push(syn::Item::Fn(function));
            }
        }
    }

    pub fn to_token_stream(self) -> Result<proc_macro2::TokenStream, syn::Error> {
        Ok(self.module.into_token_stream())
    }
}
