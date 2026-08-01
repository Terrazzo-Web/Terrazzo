pub trait HasAttributes {
    fn attrs(&self) -> &[syn::Attribute];
}

impl HasAttributes for syn::Expr {
    fn attrs(&self) -> &[syn::Attribute] {
        match self {
            syn::Expr::Array(expr) => &expr.attrs,
            syn::Expr::Assign(expr) => &expr.attrs,
            syn::Expr::Async(expr) => &expr.attrs,
            syn::Expr::Await(expr) => &expr.attrs,
            syn::Expr::Binary(expr) => &expr.attrs,
            syn::Expr::Block(expr) => &expr.attrs,
            syn::Expr::Break(expr) => &expr.attrs,
            syn::Expr::Call(expr) => &expr.attrs,
            syn::Expr::Cast(expr) => &expr.attrs,
            syn::Expr::Closure(expr) => &expr.attrs,
            syn::Expr::Const(expr) => &expr.attrs,
            syn::Expr::Continue(expr) => &expr.attrs,
            syn::Expr::Field(expr) => &expr.attrs,
            syn::Expr::ForLoop(expr) => &expr.attrs,
            syn::Expr::Group(expr) => &expr.attrs,
            syn::Expr::If(expr) => &expr.attrs,
            syn::Expr::Index(expr) => &expr.attrs,
            syn::Expr::Infer(expr) => &expr.attrs,
            syn::Expr::Let(expr) => &expr.attrs,
            syn::Expr::Lit(expr) => &expr.attrs,
            syn::Expr::Loop(expr) => &expr.attrs,
            syn::Expr::Macro(expr) => &expr.attrs,
            syn::Expr::Match(expr) => &expr.attrs,
            syn::Expr::MethodCall(expr) => &expr.attrs,
            syn::Expr::Paren(expr) => &expr.attrs,
            syn::Expr::Path(expr) => &expr.attrs,
            syn::Expr::Range(expr) => &expr.attrs,
            syn::Expr::RawAddr(expr) => &expr.attrs,
            syn::Expr::Reference(expr) => &expr.attrs,
            syn::Expr::Repeat(expr) => &expr.attrs,
            syn::Expr::Return(expr) => &expr.attrs,
            syn::Expr::Struct(expr) => &expr.attrs,
            syn::Expr::Try(expr) => &expr.attrs,
            syn::Expr::TryBlock(expr) => &expr.attrs,
            syn::Expr::Tuple(expr) => &expr.attrs,
            syn::Expr::Unary(expr) => &expr.attrs,
            syn::Expr::Unsafe(expr) => &expr.attrs,
            syn::Expr::While(expr) => &expr.attrs,
            syn::Expr::Yield(expr) => &expr.attrs,
            _ => &[],
        }
    }
}
