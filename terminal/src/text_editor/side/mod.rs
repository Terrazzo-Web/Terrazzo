use std::collections::BTreeMap;
use std::sync::Arc;

use super::fsio::FileMetadata;

pub mod mutation;
pub mod ui;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
#[serde(bound(serialize = "R: opqaue::T", deserialize = "R: opqaue::T"))]
pub struct SideViewNode<R: opqaue::T = opqaue::OpaqueNotifyRegistration> {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub properties: SvnProperties,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "i"))]
    pub item: SvnItem<R>,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub struct SvnProperties {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "s"))]
    pub status: SvnStatus,
}

#[derive(Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub enum SvnStatus {
    #[default]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "O"))]
    Opened,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    Displayed,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
#[serde(bound(serialize = "R: opqaue::T", deserialize = "R: opqaue::T"))]
pub enum SvnItem<R: opqaue::T = opqaue::OpaqueNotifyRegistration> {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    Folder(Arc<SideViewList<R>>),
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    File {
        metadata: Arc<FileMetadata>,
        #[serde(skip)]
        notify_registration: R,
    },
}

pub mod opqaue {
    use std::any::Any;
    use std::rc::Rc;

    use serde::de::DeserializeOwned;

    pub trait T: Clone + Default + serde::Serialize + DeserializeOwned {}
    impl<TT: Clone + Default + serde::Serialize + DeserializeOwned> T for TT {}

    #[derive(Clone, Default)]
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    impl OpaqueNotifyRegistration {
        pub fn is_set(&self) -> bool {
            self.0.is_some()
        }
    }

    impl serde::Serialize for OpaqueNotifyRegistration {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            serializer.serialize_unit()
        }
    }

    impl<'de> serde::Deserialize<'de> for OpaqueNotifyRegistration {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            <()>::deserialize(deserializer)?;
            Ok(Self::default())
        }
    }

    unsafe impl Send for OpaqueNotifyRegistration {}
    unsafe impl Sync for OpaqueNotifyRegistration {}
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
#[serde(transparent)]
#[serde(bound(serialize = "R: opqaue::T", deserialize = "R: opqaue::T"))]
pub struct SideViewList<R: opqaue::T = opqaue::OpaqueNotifyRegistration>(
    BTreeMap<Arc<str>, Arc<SideViewNode<R>>>,
);

impl<R: opqaue::T> std::fmt::Debug for SideViewList<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SideViewList").field(&self.0).finish()
    }
}

impl<R: opqaue::T> std::ops::Deref for SideViewList<R> {
    type Target = BTreeMap<Arc<str>, Arc<SideViewNode<R>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: opqaue::T> std::ops::DerefMut for SideViewList<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R: opqaue::T> FromIterator<(Arc<str>, Arc<SideViewNode<R>>)> for SideViewList<R> {
    fn from_iter<T: IntoIterator<Item = (Arc<str>, Arc<SideViewNode<R>>)>>(iter: T) -> Self {
        Self(BTreeMap::from_iter(iter))
    }
}

impl<R: opqaue::T> std::fmt::Debug for SideViewNode<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.item {
            SvnItem::Folder(children) => f.debug_tuple("Folder").field(children).finish(),
            SvnItem::File {
                metadata,
                notify_registration: _,
            } => f.debug_tuple("File").field(&metadata.name).finish(),
        }
    }
}
