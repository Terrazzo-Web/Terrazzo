#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleBox {
    pub h: f32,
    pub v: f32,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TexBox {
    pub h: i32,
    pub v: i32,
    pub width: i32,
    pub height: i32,
    pub depth: i32,
}
