use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::stack::Stack;
use openssl::stack::Stackable;

pub fn make_stack<T: Stackable>(
    items: impl Iterator<Item = T>,
) -> Result<Stack<T>, MakeStackError> {
    let mut stack = Stack::new().map_err(MakeStackError::New)?;
    for item in items {
        stack.push(item).map_err(MakeStackError::Push)?;
    }
    Ok(stack)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeStackError {
    #[error("[{n}] {0}", n = self.name())]
    New(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    Push(ErrorStack),
}
