//! Utils

pub mod or_else_log;
pub mod ui_thread_safe;
pub mod with_generation_id;

pub type Ptr<T> = std::rc::Rc<T>;
pub type PtrWeak<T> = std::rc::Weak<T>;
