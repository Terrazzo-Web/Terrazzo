use syn::punctuated::Punctuated;

pub enum ReturnType {
    Unit,
    T(syn::Type),
    Future(Box<Self>),
    Result(Box<Self>),
    Ref { kind: RefKind, ty: Box<Self> },
}

pub enum RefKind {
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
