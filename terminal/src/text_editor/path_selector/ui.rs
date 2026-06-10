use std::path::Path;
use std::sync::Arc;

use nameth::NamedEnumValues as _;
use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::element_capture::ElementCapture;
use web_sys::HtmlInputElement;

use self::diagnostics::debug;
use self::diagnostics::info;
use super::schema::PathSelector;
use crate::assets::icons;
use crate::text_editor::autocomplete::server_fn::AutocompleteItem;
use crate::text_editor::autocomplete::ui::do_autocomplete;
use crate::text_editor::autocomplete::ui::show_autocomplete;
use crate::text_editor::autocomplete::ui::start_autocomplete;
use crate::text_editor::autocomplete::ui::stop_autocomplete;
use crate::text_editor::manager::TextEditorManager;
use crate::text_editor::style;
use crate::utils::more_path::MorePath as _;

impl TextEditorManager {
    pub fn base_path_selector(self: &Ptr<Self>) -> XElement {
        path_selector_impl(
            self.clone(),
            PathSelector::BasePath,
            None,
            self.path.base.clone(),
            self.force_edit_path.clone(),
        )
    }

    pub fn file_path_selector(self: &Ptr<Self>) -> XElement {
        path_selector_impl(
            self.clone(),
            PathSelector::FilePath,
            Some(self.path.base.clone()),
            self.path.file.clone(),
            XSignal::new("unused-force-edit-path", false),
        )
    }
}

#[html]
#[template(tag = div)]
fn path_selector_impl(
    manager: Ptr<TextEditorManager>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
    path: XSignal<Arc<Path>>,
    #[signal] mut force_edit_path: bool,
) -> XElement {
    let show_input = kind == PathSelector::FilePath || force_edit_path;
    tag(
        class = style::PATH_SELECTOR,
        style = (!show_input).then_some("width: auto;"),
        img(class = style::PATH_SELECTOR_ICON, src = kind.icon()),
        if show_input {
            path_selector_input(manager, kind, prefix, path)
        } else {
            path_selector_display(kind, path, force_edit_path_mut)
        },
    )
}

#[autoclone]
#[html]
fn path_selector_input(
    manager: Ptr<TextEditorManager>,
    kind: PathSelector,
    prefix: Option<XSignal<Arc<Path>>>,
    path: XSignal<Arc<Path>>,
) -> XElement {
    info!("Show autocomplete input {kind:?}");
    let autocomplete: XSignal<Option<Vec<AutocompleteItem>>> = XSignal::new(kind.name(), None);
    let input: ElementCapture<HtmlInputElement> = ElementCapture::default();
    let do_autocomplete = Ptr::new(do_autocomplete(
        manager.clone(),
        input.clone(),
        autocomplete.clone(),
        kind,
        prefix.clone(),
    ));
    let input_capture = input.capture();
    let onchange = path.add_subscriber(move |new| {
        autoclone!(input);
        let () = input
            .try_with(|i| i.set_value(&new.display().to_string()))
            .unwrap_or_else(|| debug!("input was not set"));
    });
    div(
        class = style::PATH_SELECTOR_WIDGET,
        key = "input",
        input(
            before_render = move |element| {
                let _ = &onchange;
                input_capture(element);
            },
            r#type = "text",
            class = style::PATH_SELECTOR_FIELD,
            #[cfg(not(feature = "client-prod"))]
            class = match kind {
                PathSelector::BasePath => "base-path-selector-field",
                PathSelector::FilePath => "file-path-selector-field",
            },
            focus = start_autocomplete(
                manager.clone(),
                kind,
                prefix.clone(),
                path.clone(),
                input.clone(),
                autocomplete.clone(),
            ),
            blur = stop_autocomplete(kind, path.clone(), input.clone(), autocomplete.clone()),
            keydown = move |_| {
                autoclone!(do_autocomplete);
                do_autocomplete(())
            },
            click = move |_| {
                autoclone!(do_autocomplete);
                do_autocomplete(())
            },
            value = path.get_value_untracked().to_owned_string(),
        ),
        show_autocomplete(
            manager,
            kind,
            prefix.clone(),
            input,
            autocomplete.clone(),
            autocomplete,
            path,
        ),
    )
}

#[html]
#[template(tag = div)]
fn path_selector_display(
    kind: PathSelector,
    #[signal] path: Arc<Path>,
    force_edit_path_mut: MutableSignal<bool>,
) -> XElement {
    #[cfg(feature = "client-prod")]
    let _ = kind;
    let display_path = path.display();
    div(
        class = style::PATH_SELECTOR_WIDGET,
        key = "display",
        span(
            class = style::PATH_SELECTOR_FIELD,
            #[cfg(not(feature = "client-prod"))]
            class = match kind {
                PathSelector::BasePath => "base-path-selector-display",
                PathSelector::FilePath => "file-path-selector-display",
            },
            dblclick = move |_ev| force_edit_path_mut.set(true),
            "{display_path}",
        ),
    )
}

impl PathSelector {
    pub fn icon(self) -> icons::Icon {
        match self {
            Self::BasePath => icons::slash(),
            Self::FilePath => icons::chevron_double_right(),
        }
    }
}
