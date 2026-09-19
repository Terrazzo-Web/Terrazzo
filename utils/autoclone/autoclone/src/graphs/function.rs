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
    pub return_type: Rc<ReturnType>,

    pub params: RefCell<Vec<Rc<Parameter>>>,
    pub used_by: RefCell<Vec<Rc<Function>>>,

    pub errors: RefCell<Vec<String>>,
}

pub enum Parameter {
    Callee(Rc<CalleeParameter>),
    Input(Rc<InputParameter>),
}

pub struct CalleeParameter {
    function: Rc<Function>,
    #[expect(
        unused,
        reason = "// TODO: handle coersions when callee.function.return_type != callee.ty"
    )]
    ty: Rc<ReturnType>,
}

pub struct InputParameter {
    name: syn::Ident,
    ty: Rc<ReturnType>,
}

impl Function {
    pub fn new(func: &syn::ItemFn) -> Self {
        let implementation = {
            let mut func = func.clone();
            let attr = {
                let mut module: syn::ItemMod =
                    syn::parse2(quote! { #[doc(hidden)] mod x; }).unwrap();
                module.attrs.remove(0)
            };
            func.attrs.push(attr);
            func.sig.ident = format_ident!("{}_impl", func.sig.ident);
            func
        };
        Function {
            is_public: matches!(func.vis, syn::Visibility::Public { .. }),
            public_name: func.sig.ident.clone(),
            implementation,
            return_type: ReturnType::from(&func.sig.output).into(),
            params: Default::default(),
            used_by: Default::default(),
            errors: Default::default(),
        }
    }

    pub fn record_params(self: &Rc<Self>, graph: &Graph) {
        for param in &self.implementation.sig.inputs {
            self.record_param(graph, param)
        }
    }

    fn record_param(self: &Rc<Self>, graph: &Graph, param: &syn::FnArg) {
        match param {
            syn::FnArg::Receiver { .. } => {
                self.add_error("Graph functions cannot be associated methods")
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
                        ty: ReturnType::from(ty.as_ref()).into(),
                    }));
                    return;
                };
                self.params.borrow_mut().push(Rc::from(CalleeParameter {
                    function: callee.clone(),
                    ty: ReturnType::from(ty.as_ref()).into(),
                }));
                callee.used_by.borrow_mut().push(self.clone());
            }
        }
    }

    pub fn process_function(self: &Rc<Self>) -> Vec<syn::ItemFn> {
        let mut results = vec![];
        results.push(self.implementation.clone());
        if self.is_public {
            results.push(self.create_public_fn());
        }
        return results;
    }

    fn create_public_fn(self: &Rc<Self>) -> syn::ItemFn {
        let mut state = self.create_generation_state();

        // TODO: Support for generics needs more work.
        let generics = self.implementation.sig.generics.clone();

        self.process_params(&mut state);
        self.return_call_implementation(&mut state);

        syn::ItemFn {
            attrs: vec![],
            vis: self.implementation.vis.clone(),
            modifiers: Default::default(),
            sig: syn::Signature {
                constness: None,
                asyncness: state.asyncness,
                safety: Default::default(),
                abi: None,
                fn_token: self.implementation.sig.fn_token,
                ident: self.public_name.clone(),
                generics,
                paren_token: self.implementation.sig.paren_token,
                inputs: state.get_inputs(),
                variadic: None,
                output: self.implementation.sig.output.clone(),
            },
            block: syn::Block {
                brace_token: self.implementation.block.brace_token,
                stmts: state.statements,
            }
            .into(),
        }
    }

    fn process_params(&self, state: &mut GenerationState) {
        for param in &self.params.borrow().clone() {
            self.process_param(state, param)
        }
    }

    fn process_param(&self, state: &mut GenerationState, param: &Parameter) {
        match param {
            Parameter::Callee(callee) => {
                if let Some(_return_type) = state.nodes.get(param.name()) {
                    return;
                }
                state
                    .nodes
                    .insert(param.name().clone(), callee.function.return_type.clone());
                // TODO: handle coersions when callee.function.return_type != callee.ty
                let callee_name = callee.function.public_name.clone();
                let call_implementation = callee.function.call_implementation(state);
                let statement =
                    match syn::parse2(quote! { let #callee_name = #call_implementation; }) {
                        Ok(statement) => statement,
                        Err(error) => {
                            let error = format!(
                                "Failed to parse into statement: {error} -- {call_implementation}"
                            );
                            syn::parse2(quote! { compile_error!(#error); }).unwrap()
                        }
                    };
                state.statements.push(statement);
            }
            Parameter::Input(input) => {
                if let Some(_return_type) = state.nodes.get(param.name()) {
                    // TODO: coerce types if they don't match
                    return;
                }
                state.nodes.insert(param.name().clone(), input.ty.clone());
                state.inputs.push(input.clone());
            }
        }
    }

    fn call_implementation(
        self: &Rc<Self>,
        state: &mut GenerationState,
    ) -> proc_macro2::TokenStream {
        let callee_impl = self.implementation.sig.ident.clone();
        self.process_params(state);
        let callee_parameters = self
            .params
            .borrow()
            .iter()
            .map(|param| param.name().clone())
            .collect::<Vec<_>>();
        quote! { #callee_impl( #(#callee_parameters),* ) }
    }

    fn return_call_implementation(self: &Rc<Self>, state: &mut GenerationState) {
        let call_implementation = self.call_implementation(state);
        state
            .statements
            .push(syn::parse2(quote! { return #call_implementation; }).unwrap());
    }

    fn create_generation_state(&self) -> GenerationState {
        GenerationState {
            asyncness: self.implementation.sig.asyncness,
            ..Default::default()
        }
    }

    pub fn add_error(&self, error: impl Into<String>) {
        self.errors.borrow_mut().push(error.into());
    }
}

impl Parameter {
    pub fn name(&self) -> &syn::Ident {
        match self {
            Parameter::Callee(callee_parameter) => &callee_parameter.function.public_name,
            Parameter::Input(input_parameter) => &input_parameter.name,
        }
    }
}

impl From<InputParameter> for Parameter {
    fn from(value: InputParameter) -> Self {
        Self::Input(value.into())
    }
}

impl From<CalleeParameter> for Parameter {
    fn from(value: CalleeParameter) -> Self {
        Self::Callee(value.into())
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

#[derive(Default)]
struct GenerationState {
    /// Whether the generated public method is async
    asyncness: Option<syn::token::Async>,

    /// The list of inputs that are not graph nodes
    inputs: Vec<Rc<InputParameter>>,

    /// The map from node -> type that are assigned in earlier statements
    nodes: HashMap<syn::Ident, Rc<ReturnType>>,

    /// The body of the public graph implementation
    statements: Vec<syn::Stmt>,
}

impl GenerationState {
    fn get_inputs_iter(&self) -> impl Iterator<Item = syn::FnArg> {
        self.inputs
            .iter()
            .map(Rc::as_ref)
            .map(|InputParameter { name, ty }| {
                let ty = syn::Type::from(ty.as_ref());
                syn::parse2(quote! {#name: #ty}).unwrap()
            })
    }

    fn get_inputs<B: FromIterator<syn::FnArg>>(&self) -> B {
        self.get_inputs_iter().collect()
    }
}
