/*
#[graph]
mod make_app {
  pub fn run(name: String, comp1: Comp1, comp2: Comp2) -> App {
    App { name, comp1, comp2 }
  }

  fn comp1() -> Comp1 {
    Comp1::new()
  }

  fn comp2() -> Comp2 {
    Comp2::new()
  }
}

mod make_app {
  pub fn run(name: String) -> App {
    let comp1 = comp1();
    let comp2 = comp2();
    return run_impl(
      name.into(),
      comp1.into(),
      comp2.into(),
    );
  }

  fn run_impl(name: String, comp1: Comp1, comp2: Comp2) -> App {
    App { name, comp1, comp2 }
  }

  fn comp1() -> Box<Comp1> {
    Comp1::new()
  }

  fn comp2() -> Comp2 {
    Comp2::new()
  }
}



#[graph]
mod make_app {
  pub fn run(name: String, c1: C1, c2: C2) -> App {
    App { name, c1, c2 }
  }

  pub async fn c1(c3: C3) -> C1 {
    C1::new(c3)
  }

  async fn c2(c3: C3) -> C2 {
    C2::new(c3)
  }

  async fn c3() -> C3 {
    C3::new()
  }
}

mod make_app {
  pub fn run(name: String) -> App {
    let c3 = c3();
    let c3 = c3.await; // Converts Future<C3> -> C3
    let c3 = c3.into(); // convert because T in Future<T> returned by c3() != c3 from first parameter.
    let c1 = c1_impl(c3.clone()); // c3.clone() because it is referenced later
    let c2 = c2(&c3); // Ref because c2 takes a ref. Doesn't matter that types don't match
    let (c1, c2) = tokio::join!(c1, c2); // Converts multiple parameters Future<T> -> T
    return run_impl(
      name.into(),
      c1.into(),
      c2.into(),
    );
  }

  // Preserve the original impl
  fn run_impl(name: String, c1: C1, 2: C2) -> App {
    App { name, c1, c2 }
  }

  pub async fn c1() -> C1 {
    let c3 = c3();
    C1::new(c3)
  }

  // Preserve the original impl
  async fn c1_impl(c3: Arc<C3>) -> C1 {
    C1::new(c3)
  }

  // Impl and derived are the same because there is no need to generate pub c2()
  async fn c2(c3: Arc<C3>) -> C2 {
    C2::new(c3)
  }

  // Impl and derived are the same because there is no need to generate pub c3()
  async fn c3() -> C3 {
    C3::new()
  }
}

Conversions
1. Future<T> -> T using '.await'
2. Result<T, E> -> T using '?'
3. From<T>
T -> Arc<T>
T -> Rc<T>
T -> Box<T>
...
T -> From<T>
4. T -> &T
5. &T -> T
*/

use std::collections::HashSet;

use quote::ToTokens;
use syn::Ident;
use syn::Item;
use syn::Visibility;
use syn::punctuated::Punctuated;

pub fn graph2(
    _attr: proc_macro2::TokenStream,
    item: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream, syn::Error> {
    let mut graph = Graph::new(item)?;
    graph.record_pub_fns();
    graph.record_types();
    graph.to_token_stream()
}

struct Graph {
    module: syn::ItemMod,
    pub_fns: HashSet<Ident>,
}

impl Graph {
    fn new(item: proc_macro2::TokenStream) -> Result<Self, syn::Error> {
        Ok(Self {
            module: syn::parse2(item)?,
            pub_fns: Default::default(),
        })
    }

    fn record_pub_fns(&mut self) {
        let Some((_, content)) = &self.module.content else {
            return;
        };
        for item in content {
            if let Item::Fn(func) = item
                && let Visibility::Public { .. } = &func.vis
            {
                self.pub_fns.insert(func.sig.ident.clone());
            }
        }
    }

    fn record_types(&mut self) {
        let Some((_, content)) = &self.module.content else {
            return;
        };
        for item in content {
            let Item::Fn(func) = item else { continue };
            todo!()
        }
    }

    fn to_token_stream(self) -> Result<proc_macro2::TokenStream, syn::Error> {
        Ok(self.module.into_token_stream())
    }
}

enum ReturnType {
    Unit,
    T(syn::Type),
    Future(Box<Self>),
    Result(Box<Self>),
    Ref { kind: RefKind, ty: Box<Self> },
}

enum RefKind {
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
            syn::Type::Group(syn::TypeGroup { elem, .. })
            | syn::Type::Paren(syn::TypeParen { elem, .. })
            | syn::Type::Reference(syn::TypeReference { elem, .. }) => (&**elem).into(),
            syn::Type::Path(syn::TypePath {
                attrs: _,
                qself,
                path:
                    syn::Path {
                        leading_colon,
                        segments,
                    },
            }) if qself.is_none() && leading_colon.is_none() && segments.len() == 1 => {
                parse_well_known_type(segments).unwrap_or_else(|| Self::T(value.clone()))
            }
            syn::Type::Tuple(syn::TypeTuple { elems, .. }) if elems.is_empty() => Self::Unit,
            syn::Type::Array { .. }
            | syn::Type::FnPtr { .. }
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
            | _ => Self::T(value.clone()),
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
    return get_first_type_parameter(arguments).map(|ty| ReturnType::Ref {
        kind,
        ty: ty.into(),
    });
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
            }) if ident.to_string() == "Output" => {
                return Some(ReturnType::Future(Box::new(ReturnType::T(ty.clone()))));
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
    return get_first_type_parameter(arguments).map(|ty| ReturnType::Result(ty.into()));
}

fn get_first_type_parameter<'l>(
    arguments: impl IntoIterator<Item = &'l syn::GenericArgument>,
) -> Option<ReturnType> {
    for argument in arguments.into_iter() {
        match argument {
            syn::GenericArgument::Constraint { .. } | syn::GenericArgument::Lifetime { .. } => {
                continue;
            }
            syn::GenericArgument::Type(ty) => {
                return Some(ReturnType::T(ty.clone()));
            }
            syn::GenericArgument::AssocConst { .. }
            | syn::GenericArgument::AssocType { .. }
            | syn::GenericArgument::Const { .. } => break,
            _ => break,
        }
    }
    return None;
}
