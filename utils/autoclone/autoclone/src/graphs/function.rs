use std::cell::RefCell;
use std::rc::Rc;

use quote::ToTokens as _;
use quote::format_ident;

use super::graph::Graph;
use super::return_type::ReturnType;

pub struct Function {
    pub is_pub: bool,
    pub definition: syn::ItemFn,
    pub impl_name: syn::Ident,
    pub return_type: ReturnType,
    pub params: RefCell<Vec<Rc<Parameter>>>,
    pub used_by: RefCell<Vec<Rc<Function>>>,
    pub errors: RefCell<Vec<String>>,
}

pub enum Parameter {
    Callee {
        callee: Rc<Function>,
        ty: ReturnType,
    },
    Input {
        name: syn::Ident,
        ty: ReturnType,
    },
}

impl Function {
    pub fn new(func: &syn::ItemFn) -> Self {
        let is_pub = matches!(func.vis, syn::Visibility::Public { .. });
        Function {
            is_pub,
            definition: func.clone(),
            impl_name: format_ident!("{}_impl", func.sig.ident),
            return_type: (&func.sig.output).into(),
            params: Default::default(),
            used_by: Default::default(),
            errors: Default::default(),
        }
    }

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
                    self.params.borrow_mut().push(Rc::new(Parameter::Input {
                        name: ident.clone(),
                        ty: (&**ty).into(),
                    }));
                    return;
                };
                self.params.borrow_mut().push(Rc::new(Parameter::Callee {
                    callee: callee.clone(),
                    ty: (&**ty).into(),
                }));
                callee.used_by.borrow_mut().push(self.clone());
            }
        }
    }

    pub fn process(&self) -> Vec<syn::ItemFn> {
        let mut result = vec![];
        result.push(self.get_impl_item_fn());
        if self.is_pub {
            result.push(self.create_pub_fn());
        }
        return result;
    }

    fn get_impl_item_fn(&self) -> syn::ItemFn {
        let mut impl_item_fn = self.definition.clone();
        impl_item_fn.sig.ident = self.impl_name.clone();
        return impl_item_fn;
    }

    fn create_pub_fn(&self) -> syn::ItemFn {
        let mut asyncness = self.definition.sig.asyncness.clone();
        let mut inputs: Vec<syn::FnArg> = vec![];
        let mut stmts = vec![];

        // TODO: Support for generics needs more work.
        let generics = self.definition.sig.generics.clone();

        for param in &self.params.borrow().clone() {
            self.process_param(&mut asyncness, &mut inputs, &mut stmts, param)
        }

        syn::ItemFn {
            attrs: vec![],
            vis: self.definition.vis.clone(),
            modifiers: Default::default(),
            sig: syn::Signature {
                constness: None,
                asyncness,
                safety: Default::default(),
                abi: None,
                fn_token: self.definition.sig.fn_token.clone(),
                ident: self.definition.sig.ident.clone(),
                generics,
                paren_token: self.definition.sig.paren_token.clone(),
                inputs: inputs.into_iter().collect(),
                variadic: None,
                output: self.definition.sig.output.clone(),
            },
            block: syn::Block {
                brace_token: self.definition.block.brace_token.clone(),
                stmts,
            }
            .into(),
        }
    }

    fn process_param(
        &self,
        asyncness: &mut Option<syn::token::Async>,
        inputs: &mut Vec<syn::PatType>,
        stmts: &mut Vec<syn::Stmt>,
        param: &Parameter,
    ) {
        match param {
            Parameter::Callee { callee, ty } => todo!(),
            Parameter::Input { name, ty } =>{
                if inputs.iter().any(|input| input.)
            },
        }
    }

    pub fn add_error(&self, error: String) {
        self.errors.borrow_mut().push(error);
    }
}
