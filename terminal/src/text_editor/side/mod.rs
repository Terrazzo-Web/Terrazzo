use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::fsio::FileMetadata;

#[cfg(feature = "client")]
mod mutation;
#[cfg(feature = "client")]
mod ui;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "R: Default + serde::Serialize",
    deserialize = "R: Default + serde::Deserialize<'de>"
))]
#[cfg_attr(feature = "server", allow(dead_code))]
pub struct SideViewNode<R = opaque::OpaqueNotifyRegistration> {
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

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", allow(dead_code))]
pub enum SvnStatus {
    #[default]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "A"))]
    Active,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "S"))]
    Show,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "R: Default + serde::Serialize",
    deserialize = "R: Default + serde::Deserialize<'de>"
))]
#[cfg_attr(feature = "server", allow(dead_code))]
pub enum SvnItem<R = opaque::OpaqueNotifyRegistration> {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    Folder {
        folder: Arc<SideViewList<R>>,
        #[serde(skip)]
        notify: R,
    },
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    File { metadata: Arc<FileMetadata> },
}

pub mod opaque {
    #[cfg(feature = "client")]
    use std::any::Any;
    #[cfg(feature = "client")]
    use std::rc::Rc;

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    #[cfg_attr(any(feature = "server", test), derive(Default))]
    pub struct OpaqueNotifyRegistration {
        #[cfg(feature = "client")]
        #[serde(skip)]
        #[expect(unused)]
        registration: Option<Rc<dyn Any>>,
    }

    #[cfg(feature = "client")]
    mod convert {
        use terrazzo::prelude::Ptr;

        use super::OpaqueNotifyRegistration;
        use crate::text_editor::notify::ui::NotifyRegistration;

        impl From<Ptr<NotifyRegistration>> for OpaqueNotifyRegistration {
            fn from(value: Ptr<NotifyRegistration>) -> Self {
                Self {
                    registration: Some(value),
                }
            }
        }
    }

    #[cfg(feature = "client")]
    unsafe impl Send for OpaqueNotifyRegistration {}
    #[cfg(feature = "client")]
    unsafe impl Sync for OpaqueNotifyRegistration {}
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "R: Default + serde::Serialize",
    deserialize = "R: Default + serde::Deserialize<'de>"
))]
#[serde(transparent)]
pub struct SideViewList<R = opaque::OpaqueNotifyRegistration>(
    HashMap<Arc<Path>, Arc<SideViewNode<R>>>,
);

impl<R> Default for SideViewList<R> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<R> From<HashMap<Arc<Path>, Arc<SideViewNode<R>>>> for SideViewList<R> {
    fn from(value: HashMap<Arc<Path>, Arc<SideViewNode<R>>>) -> Self {
        Self(value)
    }
}

impl<R> From<SideViewList<R>> for HashMap<Arc<Path>, Arc<SideViewNode<R>>> {
    fn from(value: SideViewList<R>) -> Self {
        value.0
    }
}

impl<R> std::fmt::Debug for SideViewList<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<R> std::ops::Deref for SideViewList<R> {
    type Target = HashMap<Arc<Path>, Arc<SideViewNode<R>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R> std::ops::DerefMut for SideViewList<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R> FromIterator<(Arc<Path>, Arc<SideViewNode<R>>)> for SideViewList<R> {
    fn from_iter<T: IntoIterator<Item = (Arc<Path>, Arc<SideViewNode<R>>)>>(iter: T) -> Self {
        Self(HashMap::from_iter(iter))
    }
}

impl<R> std::fmt::Debug for SideViewNode<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.item {
            SvnItem::Folder { folder, notify: _ } => {
                let sorted_folder = (***folder)
                    .clone()
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>();
                f.debug_tuple("Folder").field(&sorted_folder).finish()
            }
            SvnItem::File { metadata } => f.debug_tuple("File").field(&metadata.name).finish(),
        }
    }
}
