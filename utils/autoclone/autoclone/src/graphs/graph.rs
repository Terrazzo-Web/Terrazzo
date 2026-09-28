use std::collections::BTreeMap;
use std::collections::HashSet;
use std::rc::Rc;

use quote::ToTokens as _;
use quote::quote;

use super::function::Function;
use crate::item_to_string;

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

    pub fn record_functions(&mut self) -> Result<(), syn::Error> {
        let Some((_, content)) = &mut self.module.content else {
            return Ok(());
        };
        let functions = content
            .extract_if(0.., |item| matches!(item, syn::Item::Fn { .. }))
            .collect::<Vec<_>>();
        let mut preceding = HashSet::new();
        for item in functions {
            let syn::Item::Fn(func) = item else { continue };
            // Include the current function to reject self-dependencies as well.
            preceding.insert(func.sig.ident.clone());
            for param in &func.sig.inputs {
                if let syn::FnArg::Typed(param) = param
                    && let syn::Pat::Ident(param) = &*param.pat
                    && param.by_ref.is_none()
                    && param.subpat.is_none()
                    && preceding.contains(&param.ident)
                {
                    return Err(syn::Error::new(
                        param.ident.span(),
                        format!(
                            "Graph functions must be declared in topological order: `{}` must be declared before its dependency `{}`",
                            func.sig.ident, param.ident
                        ),
                    ));
                }
            }
            let function = Function::new(&func);
            self.functions
                .insert(func.sig.ident.clone(), function.into());
        }
        for function in self.functions.values() {
            function.record_params(self)
        }
        Ok(())
    }

    pub fn process_functions(&mut self) {
        for function in &mut self.functions.values() {
            let functions = function.process_function();
            let Some((_, content)) = &mut self.module.content else {
                return;
            };
            for function in functions {
                let function = if let syn::Visibility::Inherited = &function.vis {
                    function
                } else {
                    let doc = format!(
                        r#"Implementation:
> ```ignore
> {}
> ```"#,
                        item_to_string(&syn::Item::Fn(function.clone())).replace("\n", "\n> ")
                    );
                    let doc = quote! { #[cfg_attr(debug_assertions, doc = #doc)] };
                    let mut function = function;
                    function.attrs.extend(
                        syn::parse2::<syn::ItemFn>(quote! { #doc fn x() {}})
                            .unwrap()
                            .attrs,
                    );
                    function
                };
                let named_inputs = if function.vis == syn::Visibility::Inherited {
                    vec![]
                } else {
                    super::named_inputs::generate(&function)
                };
                content.push(syn::Item::Fn(function));
                content.extend(named_inputs);
            }
        }
    }

    pub fn into_token_stream(self) -> Result<proc_macro2::TokenStream, syn::Error> {
        Ok(self.module.into_token_stream())
    }
}
