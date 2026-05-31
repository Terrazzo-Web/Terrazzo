use std::collections::BTreeMap;
use std::sync::Arc;

use super::fsio::FileMetadata;

pub mod mutation;
pub mod ui;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub struct SideViewNode {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub properties: SideViewNodeProps,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "i"))]
    pub item: SideViewNodeItem,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub struct SideViewNodeProps {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "s"))]
    pub status: UiStatus,
}

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub enum UiStatus {
    #[default]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "O"))]
    Opened,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    Displayed,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub enum SideViewNodeItem {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    Folder(Arc<SideViewList>),
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    File {
        metadata: Arc<FileMetadata>,
        #[serde(skip)]
        notify_registration: opqaue::OpaqueNotifyRegistration,
    },
}

pub mod opqaue {
    use std::any::Any;
    use std::rc::Rc;

    #[derive(Clone, Default)]
    pub struct OpaqueNotifyRegistration(Option<Rc<dyn Any>>);

    #[cfg(feature = "client")]
    mod convert {
        use terrazzo::prelude::Ptr;

        use crate::text_editor::notify::ui::NotifyRegistration;
        use crate::text_editor::side::opqaue::OpaqueNotifyRegistration;
        impl From<Ptr<NotifyRegistration>> for OpaqueNotifyRegistration {
            fn from(value: Ptr<NotifyRegistration>) -> Self {
                Self(Some(value))
            }
        }
    }

    impl OpaqueNotifyRegistration {
        pub fn is_set(&self) -> bool {
            self.0.is_some()
        }
    }

    unsafe impl Send for OpaqueNotifyRegistration {}
    unsafe impl Sync for OpaqueNotifyRegistration {}
}

pub type SideViewList = BTreeMap<Arc<str>, Arc<SideViewNode>>;

impl std::fmt::Debug for SideViewNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.item {
            SideViewNodeItem::Folder(children) => f.debug_tuple("Folder").field(children).finish(),
            SideViewNodeItem::File {
                metadata,
                notify_registration,
            } => {
                let _ = notify_registration.is_set();
                f.debug_tuple("File").field(&metadata.name).finish()
            }
        }
    }
}
