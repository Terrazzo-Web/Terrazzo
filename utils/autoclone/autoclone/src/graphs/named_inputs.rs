use std::collections::HashSet;

use quote::format_ident;
use quote::quote;
use syn::visit_mut::VisitMut;

/// Generate the named-input counterpart of a public graph entry point.
pub fn generate(function: &syn::ItemFn) -> Vec<syn::Item> {
    let function_name = &function.sig.ident;
    let name = function_name.to_string();
    let name = name
        .trim_start_matches("r#")
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().unwrap().to_uppercase().collect::<String>() + chars.as_str()
        })
        .collect::<String>();
    let name = format_ident!("{name}");
    let visibility = &function.vis;
    let mut generics = function.sig.generics.clone();
    let mut field_types = FieldTypes {
        reserved: generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Lifetime(param) => param.lifetime.ident.to_string(),
                syn::GenericParam::Type(param) => param.ident.to_string(),
                syn::GenericParam::Const(param) => param.ident.to_string(),
            })
            .collect(),
        parameters: Vec::new(),
    };
    let mut names = Vec::new();
    let mut types = Vec::new();
    for input in &function.sig.inputs {
        let syn::FnArg::Typed(input) = input else {
            continue;
        };
        let syn::Pat::Ident(pat) = input.pat.as_ref() else {
            continue;
        };
        names.push(pat.ident.clone());
        let mut ty = *input.ty.clone();
        field_types.visit_type_mut(&mut ty);
        types.push(ty);
    }
    // Rust requires lifetime parameters before type and const parameters.
    for parameter in field_types.parameters {
        if matches!(parameter, syn::GenericParam::Lifetime(_)) {
            generics.params.insert(0, parameter);
        } else {
            generics.params.push(parameter);
        }
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    let mut output = function.sig.output.clone();
    let lifetimes = types
        .iter()
        .filter_map(|ty| match ty {
            syn::Type::Reference(reference) => reference.lifetime.clone(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if let [lifetime] = lifetimes.as_slice() {
        // A by-value `self` cannot supply the free function's lifetime elision.
        // Carry the sole borrowed input's lifetime into its borrowed output.
        OutputLifetime(lifetime.clone()).visit_return_type_mut(&mut output);
    }
    let asyncness = &function.sig.asyncness;
    let await_call = asyncness.map(|_| quote! { .await });
    let file: syn::File = syn::parse2(quote! {
        #visibility struct #name #impl_generics #where_clause {
            #(pub #names: #types),*
        }
        impl #impl_generics #name #type_generics #where_clause {
            #visibility #asyncness fn run(self) #output {
                let Self { #(#names),* } = self;
                #function_name(#(#names),*) #await_call
            }
        }
    })
    .unwrap();
    file.items
}

struct FieldTypes {
    reserved: HashSet<String>,
    parameters: Vec<syn::GenericParam>,
}

struct OutputLifetime(syn::Lifetime);

impl VisitMut for OutputLifetime {
    fn visit_type_reference_mut(&mut self, reference: &mut syn::TypeReference) {
        if reference.lifetime.as_ref().is_none_or(|l| l.ident == "_") {
            reference.lifetime = Some(self.0.clone());
        }
        syn::visit_mut::visit_type_reference_mut(self, reference);
    }
}

impl FieldTypes {
    fn fresh_name(&mut self, prefix: &str) -> syn::Ident {
        for index in 0.. {
            let name = format!("{prefix}{index}");
            if self.reserved.insert(name.clone()) {
                return format_ident!("{name}");
            }
        }
        unreachable!()
    }
}

impl VisitMut for FieldTypes {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        syn::visit_mut::visit_type_mut(self, ty);
        match ty {
            syn::Type::ImplTrait(opaque) => {
                let name = self.fresh_name("GraphInput");
                let bounds = &opaque.bounds;
                self.parameters.push(syn::parse_quote! { #name: #bounds });
                *ty = syn::parse_quote! { #name };
            }
            syn::Type::Reference(reference)
                if reference.lifetime.as_ref().is_none_or(|l| l.ident == "_") =>
            {
                let name = self.fresh_name("graph_input");
                let lifetime = syn::Lifetime::new(&format!("'{name}"), name.span());
                self.parameters.push(syn::parse_quote! { #lifetime });
                reference.lifetime = Some(lifetime);
            }
            _ => {}
        }
    }
}

/// Drop lifetimes belonging only to internal nodes from the generated entry point.
pub fn prune_internal_lifetimes(signature: &mut syn::Signature) {
    #[derive(Default)]
    struct UsedLifetimes(HashSet<String>);
    impl VisitMut for UsedLifetimes {
        fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
            self.0.insert(lifetime.ident.to_string());
        }
    }
    let mut used = UsedLifetimes::default();
    for input in &mut signature.inputs {
        used.visit_fn_arg_mut(input);
    }
    used.visit_return_type_mut(&mut signature.output);
    for parameter in &mut signature.generics.params {
        match parameter {
            syn::GenericParam::Lifetime(parameter) => {
                for bound in &mut parameter.bounds {
                    used.visit_lifetime_mut(bound);
                }
            }
            parameter => used.visit_generic_param_mut(parameter),
        }
    }
    if let Some(clause) = &mut signature.generics.where_clause {
        used.visit_where_clause_mut(clause);
    }
    signature.generics.params = signature
        .generics
        .params
        .clone()
        .into_iter()
        .filter(|parameter| match parameter {
            syn::GenericParam::Lifetime(parameter) => {
                used.0.contains(&parameter.lifetime.ident.to_string())
            }
            _ => true,
        })
        .collect();
}
