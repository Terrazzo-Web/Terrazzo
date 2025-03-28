use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo_client::prelude::XString;
use wasm_bindgen::JsCast;
use web_sys::EventTarget;

pub trait MoreEvent {
    /// Shortcut to get the target of a DOM event cast to a certain type.
    fn current_target_element<T: JsCast>(
        &self,
        name: impl Into<XString>,
    ) -> Result<T, CurrentTargetElementError>;
}

impl MoreEvent for web_sys::Event {
    fn current_target_element<T: JsCast>(
        &self,
        name: impl Into<XString>,
    ) -> Result<T, CurrentTargetElementError> {
        let Some(current_target) = self.current_target() else {
            return Err(CurrentTargetElementError::Missing(name.into()));
        };
        current_target
            .dyn_into()
            .map_err(|wrong_type| CurrentTargetElementError::WrongType(name.into(), wrong_type))
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CurrentTargetElementError {
    #[error("[{n}] [{0}] Missing current target", n = self.name())]
    Missing(XString),

    #[error("[{n}] [{0}] Wrong type: {1:?}", n = self.name())]
    WrongType(XString, EventTarget),
}
