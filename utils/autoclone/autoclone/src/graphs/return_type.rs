use std::rc::Rc;

use quote::quote;
use syn::punctuated::Punctuated;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReturnType {
    Unit,
    T(Rc<syn::Type>),
    Future(Rc<Self>),
    Result(Rc<Self>, Rc<Self>),
    Ref { kind: RefKind, ty: Rc<Self> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefKind {
    Ref,
    Box,
    Arc,
    Rc,
}

impl From<&syn::ReturnType> for ReturnType {
    fn from(value: &syn::ReturnType) -> Self {
        match value {
            syn::ReturnType::Default => Self::Unit,
            syn::ReturnType::Type(_, ty) => (&**ty).into(),
        }
    }
}

impl From<&syn::Type> for ReturnType {
    fn from(value: &syn::Type) -> Self {
        match value {
            syn::Type::Paren(syn::TypeParen { elem, .. }) => (&**elem).into(),
            syn::Type::Reference(syn::TypeReference { elem, .. }) => Self::Ref {
                kind: RefKind::Ref,
                ty: ReturnType::from(&**elem).into(),
            },
            syn::Type::Path(syn::TypePath {
                attrs: _,
                qself,
                path:
                    syn::Path {
                        leading_colon,
                        segments,
                    },
            }) if qself.is_none() && leading_colon.is_none() && segments.len() == 1 => {
                parse_well_known_type(segments).unwrap_or_else(|| Self::T(value.clone().into()))
            }
            syn::Type::Tuple(syn::TypeTuple { elems, .. }) if elems.is_empty() => Self::Unit,
            syn::Type::Array { .. }
            | syn::Type::FnPtr { .. }
            | syn::Type::Group { .. }
            | syn::Type::ImplTrait { .. }
            | syn::Type::Infer { .. }
            | syn::Type::Macro { .. }
            | syn::Type::Never { .. }
            | syn::Type::Path { .. }
            | syn::Type::Ptr { .. }
            | syn::Type::Slice { .. }
            | syn::Type::TraitObject { .. }
            | syn::Type::Tuple { .. }
            | syn::Type::Verbatim { .. }
            | _ => Self::T(value.clone().into()),
        }
    }
}

fn parse_well_known_type(
    segments: &Punctuated<syn::PathSegment, syn::token::PathSep>,
) -> Option<ReturnType> {
    let syn::PathSegment { ident, arguments } = segments.first().unwrap();
    let arguments = if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
        colon2_token,
        args,
        ..
    }) = arguments
        && colon2_token.is_none()
    {
        args
    } else {
        return None;
    };
    match ident.to_string().as_str() {
        "Box" => parse_ref_kind(RefKind::Box, arguments),
        "Rc" => parse_ref_kind(RefKind::Rc, arguments),
        "Arc" => parse_ref_kind(RefKind::Arc, arguments),
        "Future" => parse_future(arguments),
        "Result" => parse_result(arguments),
        _ => None,
    }
}

fn parse_ref_kind<'l>(
    kind: RefKind,
    arguments: impl IntoIterator<Item = &'l syn::GenericArgument>,
) -> Option<ReturnType> {
    match get_type_parameters(arguments).as_slice() {
        [ty] => Some(ReturnType::Ref {
            kind,
            ty: ty.clone().into(),
        }),
        _ => None,
    }
}

fn parse_future<'l>(
    arguments: impl IntoIterator<Item = &'l syn::GenericArgument>,
) -> Option<ReturnType> {
    for argument in arguments.into_iter() {
        match argument {
            syn::GenericArgument::Constraint { .. } | syn::GenericArgument::Lifetime { .. } => {
                continue;
            }
            syn::GenericArgument::AssocType(syn::AssocType {
                ident,
                generics: None,
                eq_token: syn::token::Eq { .. },
                ty,
            }) if *ident == "Output" => {
                return Some(ReturnType::Future(ReturnType::from(ty).into()));
            }
            syn::GenericArgument::Type { .. }
            | syn::GenericArgument::Const { .. }
            | syn::GenericArgument::AssocConst { .. } => break,
            _ => break,
        }
    }
    return None;
}

fn parse_result<'l>(
    arguments: impl IntoIterator<Item = &'l syn::GenericArgument>,
) -> Option<ReturnType> {
    match get_type_parameters(arguments).as_slice() {
        [output, error] => Some(ReturnType::Result(
            output.clone().into(),
            error.clone().into(),
        )),
        _ => None,
    }
}

fn get_type_parameters<'l>(
    arguments: impl IntoIterator<Item = &'l syn::GenericArgument>,
) -> Vec<ReturnType> {
    arguments
        .into_iter()
        .try_fold(vec![], |mut accu, argument| match argument {
            syn::GenericArgument::Constraint { .. } | syn::GenericArgument::Lifetime { .. } => {
                return Some(accu);
            }
            syn::GenericArgument::Type(ty) => {
                accu.push(ReturnType::from(ty));
                return Some(accu);
            }
            syn::GenericArgument::AssocConst { .. }
            | syn::GenericArgument::AssocType { .. }
            | syn::GenericArgument::Const { .. } => return None,
            _ => return None,
        })
        .unwrap_or_default()
}

impl From<&ReturnType> for syn::Type {
    fn from(return_type: &ReturnType) -> Self {
        syn::parse2(match return_type {
            ReturnType::Unit => quote! { () },
            ReturnType::T(ty) => return ty.as_ref().clone(),
            ReturnType::Future(return_type) => {
                let return_type = syn::Type::from(&**return_type);
                quote! { Future<Output = #return_type> }
            }
            ReturnType::Result(output_type, error_type) => {
                let output_type = syn::Type::from(&**output_type);
                let error_type = syn::Type::from(&**error_type);
                quote! { Result<#output_type, #error_type> }
            }
            ReturnType::Ref { kind, ty } => {
                let return_type = syn::Type::from(&**ty);
                match kind {
                    RefKind::Ref => quote! { & #return_type },
                    RefKind::Box => quote! { Box<#return_type> },
                    RefKind::Arc => quote! { Arc<#return_type> },
                    RefKind::Rc => quote! { Rc<#return_type> },
                }
            }
        })
        .unwrap()
    }
}

#[derive(Default)]
pub struct Coercion {
    pub force_async: bool,
    pub force_result: bool,
    pub expr: proc_macro2::TokenStream,
}

impl ReturnType {
    pub fn coerce(
        self: &Rc<ReturnType>,
        into: &Rc<ReturnType>,
        mut expr: proc_macro2::TokenStream,
    ) -> Coercion {
        let mut coercion = Coercion::default();
        // a: Future<Result<Box<T>>> vs b: Rc<T>
        let (_a, ta) = self.transformations();
        let (_b, tb) = into.transformations();
        // ta: [ Result<T> ; Future ] vs b: [ Rc ]
        let mut ta = ta.into_iter().rev().peekable();
        let mut tb = tb.into_iter().rev().peekable();
        while ta.peek().is_some() && ta.peek() == tb.peek() {
            ta.next();
            tb.next();
        }
        for (i, taa) in ta.rev().enumerate() {
            if i != 0 {
                expr = quote! { (#expr) };
            }
            expr = match taa {
                TypeTransformations::Future => {
                    coercion.force_async = true;
                    quote! { #expr.await }
                }
                TypeTransformations::Result => {
                    coercion.force_result = true;
                    quote! { #expr? }
                }
                TypeTransformations::Ref(_) => quote! { *#expr },
            };
        }
        for tbb in tb {
            expr = match tbb {
                TypeTransformations::Future => quote! { async move { #expr } },
                TypeTransformations::Result => quote! { Ok(#expr) },
                TypeTransformations::Ref(ref_kind) => match ref_kind {
                    RefKind::Ref => quote! { &#expr },
                    RefKind::Box => quote! { Box::new(#expr) },
                    RefKind::Arc => quote! { Arc::new(#expr) },
                    RefKind::Rc => quote! { Rc::new(#expr) },
                },
            };
        }
        coercion.expr = expr;
        return coercion;
    }

    fn transformations(mut self: &Rc<ReturnType>) -> (&Rc<Self>, Vec<TypeTransformations>) {
        let mut transformations = vec![];
        loop {
            let (transformation, next) = match self.as_ref() {
                ReturnType::Unit | ReturnType::T(_) => {
                    return (self, transformations);
                }
                ReturnType::Future(ty) => (TypeTransformations::Future, ty),
                ReturnType::Result(output, _error) => (TypeTransformations::Result, output),
                ReturnType::Ref { kind, ty } => (TypeTransformations::Ref(*kind), ty),
            };
            transformations.push(transformation);
            self = next;
        }
    }
}

#[derive(PartialEq, Eq)]
enum TypeTransformations {
    Future,
    Result,
    Ref(RefKind),
}
