#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum App {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "Z"))]
    Default,

    #[cfg(feature = "terminal")]
    #[default]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "T"))]
    Terminal,

    #[cfg(feature = "text-editor")]
    #[cfg_attr(not(feature = "terminal"), default)]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    TextEditor,

    #[cfg(feature = "converter")]
    #[cfg_attr(not(any(feature = "terminal", feature = "text-editor")), default)]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "C"))]
    Converter,

    #[cfg(feature = "port-forward")]
    #[cfg_attr(
        not(any(feature = "terminal", feature = "text-editor", feature = "converter")),
        default
    )]
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "P"))]
    PortForward,
}

impl std::fmt::Display for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            App::Default => "Default",
            #[cfg(feature = "terminal")]
            App::Terminal => "Terminal",
            #[cfg(feature = "text-editor")]
            App::TextEditor => "Text editor",
            #[cfg(feature = "converter")]
            App::Converter => "Converter",
            #[cfg(feature = "port-forward")]
            App::PortForward => "Port forward",
        }
        .fmt(f)
    }
}

/// Persisted settings for the apps hosted by a tile.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppConfig {
    #[cfg(feature = "text-editor")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_path:
        Option<crate::text_editor::file_path::FilePath<std::sync::Arc<std::path::Path>>>,
}
