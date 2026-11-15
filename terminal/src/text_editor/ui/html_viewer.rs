use std::sync::Arc;

use terrazzo::html;
use terrazzo::prelude::*;

terrazzo_css::import_style!(pub(super) style, "html_viewer.scss");

#[html]
pub(super) fn html_viewer(content: Arc<str>) -> XElement {
    iframe(
        class = style::HTML_PREVIEW_PANE,
        sandbox = "",
        title = "HTML preview",
        #[cfg(not(feature = "client-prod"))]
        data_viewer = "html",
        srcdoc = content.as_ref().to_owned(),
    )
}

// An image data URL keeps SVG scripts and embedded HTML out of the app document.
fn svg_source(content: &str) -> String {
    use base64::Engine as _;
    format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(content)
    )
}

#[html]
pub(super) fn svg_viewer(content: Arc<str>) -> XElement {
    img(
        class = style::HTML_PREVIEW_PANE,
        alt = "SVG preview",
        src = svg_source(&content),
        #[cfg(not(feature = "client-prod"))]
        data_viewer = "svg",
    )
}

pub(super) fn update_preview(
    editor_type: super::editor_render::EditorType,
    preview: &web_sys::Element,
    content: &str,
) {
    use super::editor_render::EditorType;

    match editor_type {
        EditorType::Svg => {
            let _ = preview.set_attribute("src", &svg_source(content));
        }
        EditorType::Html => {
            let _ = preview.set_attribute("srcdoc", content);
        }
        EditorType::Pdf | EditorType::Text | EditorType::Markdown => {}
    }
}
