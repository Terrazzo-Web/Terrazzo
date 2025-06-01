//! Utils

pub mod or_else_log;

pub type Ptr<T> = std::rc::Rc<T>;
pub type PtrWeak<T> = std::rc::Weak<T>;
