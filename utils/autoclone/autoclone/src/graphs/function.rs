use std::cell::RefCell;
use std::rc::Rc;

use quote::ToTokens as _;

use super::graph::Graph;
use super::return_type::ReturnType;

pub struct Function {
    pub is_pub: bool,
    pub definition: syn::ItemFn,
    pub impl_name: syn::Ident,
    pub return_type: ReturnType,
    pub params: RefCell<Vec<Rc<Function>>>,
    pub used_by: RefCell<Vec<Rc<Function>>>,
    pub errors: RefCell<Vec<String>>,
}

impl Function {
    pub fn parse_param(self: &Rc<Self>, graph: &Graph, param: &syn::FnArg) {
        match param {
            syn::FnArg::Receiver { .. } => {
                self.add_error(format!("Graph functions cannot be associated methods"))
            }
            syn::FnArg::Typed(syn::PatType {
                attrs: _,
                pat,
                colon_token: syn::token::Colon { .. },
                ty,
            }) => {
                let syn::Pat::Ident(syn::PatIdent {
                    attrs: _,
                    by_ref: None,
                    mutability: _,
                    ident,
                    subpat: None,
                }) = &**pat
                else {
                    self.add_error(format!(
                        "Only ident parameters are supported, got '{}'",
                        pat.to_token_stream()
                    ));
                    return;
                };
                let Some(callee) = graph.functions.get(ident) else {
                    return;
                };
                self.params.borrow_mut().push(callee.clone());
                callee.used_by.borrow_mut().push(self.clone());
            }
        }
    }

    pub fn add_error(&self, error: String) {
        self.errors.borrow_mut().push(error);
    }
}
