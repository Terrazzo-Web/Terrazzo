use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use quote::ToTokens as _;
use quote::format_ident;
use quote::quote;

use super::graph::Graph;
use super::return_type::ReturnType;

pub struct Function {
    /// Public functions are converted to public graph entry points (same visibility).
    /// The public name is the original name of the function
    pub is_public: bool,
    pub public_name: syn::Ident,

    /// The definition of the function implementation.
    /// The name of the function is prefixed with '_impl'
    pub implementation: syn::ItemFn,

    /// The type of the function, parsed to [ReturnType].
    pub return_type: ReturnType,

    pub params: RefCell<Vec<Rc<Parameter>>>,
    pub used_by: RefCell<Vec<Rc<Function>>>,

    pub errors: RefCell<Vec<String>>,
}

pub enum Parameter {
    Callee(CalleeParameter),
    Input(InputParameter),
}

pub struct CalleeParameter {
    callee: Rc<Function>,
    ty: ReturnType,
}

pub struct InputParameter {
    name: syn::Ident,
    ty: ReturnType,
}

impl Function {
    pub fn new(func: &syn::ItemFn) -> Self {
        let implementation = {
            let mut func = func.clone();
            func.sig.ident = format_ident!("{}_impl", func.sig.ident);
            func
        };
        Function {
            is_public: matches!(func.vis, syn::Visibility::Public { .. }),
            public_name: func.sig.ident.clone(),
            implementation,
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
                    self.params.borrow_mut().push(Rc::from(InputParameter {
                        name: ident.clone(),
                        ty: (&**ty).into(),
                    }));
                    return;
                };
                self.params.borrow_mut().push(Rc::from(CalleeParameter {
                    callee: callee.clone(),
                    ty: (&**ty).into(),
                }));
                callee.used_by.borrow_mut().push(self.clone());
            }
        }
    }

    pub fn process(&self) -> Vec<syn::ItemFn> {
        let mut result = vec![];
        result.push(self.implementation.clone());
        if self.is_public {
            result.push(self.create_pub_fn());
        }
        return result;
    }

    fn create_pub_fn(&self) -> syn::ItemFn {
        let mut state = self.create_generation_state();

        // TODO: Support for generics needs more work.
        let generics = self.implementation.sig.generics.clone();

        for param in &self.params.borrow().clone() {
            self.process_param(&mut state, param)
        }

        syn::ItemFn {
            attrs: vec![],
            vis: self.implementation.vis.clone(),
            modifiers: Default::default(),
            sig: syn::Signature {
                constness: None,
                asyncness: state.asyncness,
                safety: Default::default(),
                abi: None,
                fn_token: self.implementation.sig.fn_token.clone(),
                ident: self.public_name.clone(),
                generics,
                paren_token: self.implementation.sig.paren_token.clone(),
                inputs: state.get_inputs(),
                variadic: None,
                output: self.implementation.sig.output.clone(),
            },
            block: syn::Block {
                brace_token: self.implementation.block.brace_token.clone(),
                stmts,
            }
            .into(),
        }
    }

    fn process_param(&self, state: &mut GenerationState, param: &Parameter) {
        match param {
            Parameter::Callee(CalleeParameter { callee, ty }) => todo!(),
            Parameter::Input(InputParameter { name, ty }) => todo!(),
        }
    }

    fn create_generation_state(&self) -> GenerationState {
        GenerationState {
            asyncness: todo!(),
            inputs: todo!(),
            nodes: todo!(),
            statements: todo!(),
        }
    }

    pub fn add_error(&self, error: String) {
        self.errors.borrow_mut().push(error);
    }
}

impl Parameter {
    pub fn name(&self) -> &syn::Ident {
        match self {
            Parameter::Callee(CalleeParameter { callee, .. }) => &callee.public_name,
            Parameter::Input(InputParameter { name, .. }) => name,
        }
    }
}

impl From<InputParameter> for Parameter {
    fn from(value: InputParameter) -> Self {
        Self::Input(value)
    }
}

impl From<CalleeParameter> for Parameter {
    fn from(value: CalleeParameter) -> Self {
        Self::Callee(value)
    }
}

impl From<InputParameter> for Rc<Parameter> {
    fn from(value: InputParameter) -> Self {
        Parameter::from(value).into()
    }
}

impl From<CalleeParameter> for Rc<Parameter> {
    fn from(value: CalleeParameter) -> Self {
        Parameter::from(value).into()
    }
}

struct GenerationState {
    /// Whether the generated public method is async
    asyncness: Option<syn::token::Async>,

    /// The list of inputs that are not graph nodes
    inputs: Vec<InputParameter>,

    /// The map from node -> type that are assigned in earlier statements
    nodes: HashMap<syn::Ident, ReturnType>,

    /// The body of the public graph implementation
    statements: Vec<syn::Stmt>,
}

impl GenerationState {
    fn get_inputs_iter(&self) -> impl Iterator<Item = syn::FnArg> {
        self.inputs.iter().map(|InputParameter { name, ty }| {
            let ty = syn::Type::from(ty);
            syn::parse2(quote! {#name: #ty}).unwrap()
        })
    }

    fn get_inputs<B: FromIterator<syn::FnArg>>(&self) -> B {
        self.get_inputs_iter().collect()
    }
}
