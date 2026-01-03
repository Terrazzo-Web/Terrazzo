//! Attributes of generated HTML elements

use nameth::nameth;

use self::id::XAttributeId;
use self::value::XAttributeValue;

pub mod builder;
pub mod diff_store;
pub mod id;
pub mod merge;
pub mod name;
pub mod template;
pub mod value;

/// Represents an attribute of an HTML node.
///
/// Example: the HTML tag `<input type="text" name="username" value="LamparoS@Pavy.one" />`
/// would have an attribute
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XAttribute {
///     id: XAttributeId {
///         name: XAttributeKind::Attribute.make("name"),
///         index: 0,
///         sub_index: 0,
///     },
///     value: "username".into(),
/// }
/// # ;
/// ```
///
/// and an attribute
/// ```
/// # use terrazzo_client::prelude::*;
/// # let _ =
/// XAttribute {
///     id: XAttributeId {
///         name: XAttributeKind::Attribute.make("value"),
///         index: 1,
///         sub_index: 0,
///     },
///     value: "LamparoS@Pavy.one".into(),
/// }
/// # ;
/// ```
#[nameth]
pub struct XAttribute {
    /// ID of the attribute
    pub id: XAttributeId,

    /// Value of the attribute
    pub value: XAttributeValue,
}
