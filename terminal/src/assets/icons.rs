#[cfg(all(feature = "server", not(feature = "client")))]
macro_rules! declare_icon {
    ($name:ident, $file:expr; $($predicate:tt)*) => {
        #[cfg($($predicate)*)]
        declare_icon!($name, $file);
    };
    ($name:ident, $file:expr) => {
        pub fn $name() -> Icon {
            terrazzo::declare_asset!(concat!("/assets", $file))
        }
    };
}

#[cfg(all(feature = "server", not(feature = "client")))]
pub type Icon = terrazzo::static_assets::AssetBuilder;

#[cfg(feature = "client")]
macro_rules! declare_icon {
    ($name:ident, $file:expr; $($predicate:tt)*) => {
        #[cfg($($predicate)*)]
        declare_icon!($name, $file);
    };
    ($name:ident, $file:expr) => {
        pub fn $name() -> Icon {
            concat!("/static", $file)
        }
    };
}

#[cfg(feature = "client")]
pub type Icon = &'static str;

declare_icon!(add_port_forward,"/icons/add-port-forward.svg"; feature = "port-forward");
declare_icon!(add_tab, "/icons/plus-square.svg");
declare_icon!(chevron_bar_down, "/icons/chevron-bar-down.svg"; feature = "logs-panel");
declare_icon!(chevron_bar_up, "/icons/chevron-bar-up.svg"; feature = "logs-panel");
declare_icon!(chevron_double_right, "/icons/chevron-double-right.svg"; feature = "text-editor");
declare_icon!(close_app, "/icons/trash3-fill.svg");
declare_icon!(close_tab, "/icons/x-lg.svg");
declare_icon!(collapse_vert, "/icons/arrows-collapse.svg"; feature = "text-editor");
declare_icon!(collection, "/icons/collection.svg");
declare_icon!(converter, "/icons/regex.svg"; feature = "converter");
declare_icon!(copy, "/icons/copy.svg"; feature = "converter");
declare_icon!(chevron_bar_contract, "/icons/chevron-bar-contract.svg"; feature = "text-editor");
declare_icon!(chevron_bar_expand, "/icons/chevron-bar-expand.svg"; feature = "text-editor");
declare_icon!(diff, "/icons/arrow-left-right.svg"; feature = "text-editor");
declare_icon!(done, "/icons/done.svg"; any(feature = "converter", feature = "text-editor"));
declare_icon!(download, "/icons/download.svg"; feature = "text-editor");
declare_icon!(drag_handle_corner, "/icons/drag-handle-corner.svg");
declare_icon!(file, "/icons/file-earmark-text.svg"; feature = "text-editor");
declare_icon!(folder, "/icons/folder2-open.svg"; feature = "text-editor");
declare_icon!(git, "/icons/git.svg"; feature = "text-editor");
declare_icon!(hdd, "/icons/hdd.svg"; feature = "text-editor");
declare_icon!(hub, "/icons/hub.svg"; feature = "port-forward");
declare_icon!(key_icon, "/icons/key.svg");
declare_icon!(loading, "/icons/loading2.svg"; feature = "text-editor");
declare_icon!(menu, "/icons/signpost-split.svg");
declare_icon!(mic_fill, "/icons/mic-fill.svg"; any(feature = "terminal", feature = "text-editor"));
declare_icon!(mic_mute_fill, "/icons/mic-mute-fill.svg"; any(feature = "terminal", feature = "text-editor"));
declare_icon!(new_file, "/icons/file-earmark-plus.svg"; feature = "text-editor");
declare_icon!(new_folder, "/icons/folder-plus.svg"; feature = "text-editor");
declare_icon!(paragraph, "/icons/paragraph.svg"; any(feature = "terminal", feature = "text-editor"));
declare_icon!(port_forward_loading,"/icons/port-forward-loading.svg"; feature = "port-forward");
declare_icon!(port_forward_pending,"/icons/port-forward-pending.svg"; feature = "port-forward");
declare_icon!(port_forward_synchronized,"/icons/port-forward-synchronized.svg"; feature = "port-forward");
declare_icon!(refresh, "/icons/arrow-counterclockwise.svg"; any(feature = "terminal", feature = "text-editor"));
declare_icon!(search, "/icons/search.svg"; feature = "text-editor");
declare_icon!(send_fill, "/icons/send-fill.svg"; any(feature = "terminal", feature = "text-editor"));
declare_icon!(slash, "/icons/slash.svg"; feature = "text-editor");
declare_icon!(split_horz, "/icons/arrows-expand-vertical.svg");
declare_icon!(split_vert, "/icons/arrows-expand.svg");
declare_icon!(terminal, "/icons/terminal-dash.svg"; feature = "terminal");
declare_icon!(text_editor, "/icons/layout-text-sidebar-reverse.svg"; feature = "text-editor");
declare_icon!(trash, "/icons/trash3.svg"; any(feature = "port-forward", feature = "text-editor"));
declare_icon!(window_stack, "/icons/window-stack.svg");

// Material Icon Theme; see assets/icons/MATERIAL-ICONS.md.
declare_icon!(filetype_rust, "/icons/material-rust.svg"; feature = "text-editor");
declare_icon!(filetype_javascript, "/icons/material-javascript.svg"; feature = "text-editor");
declare_icon!(filetype_typescript, "/icons/material-typescript.svg"; feature = "text-editor");
declare_icon!(filetype_react, "/icons/material-react.svg"; feature = "text-editor");
declare_icon!(filetype_python, "/icons/material-python.svg"; feature = "text-editor");
declare_icon!(filetype_java, "/icons/material-java.svg"; feature = "text-editor");
declare_icon!(filetype_c, "/icons/material-c.svg"; feature = "text-editor");
declare_icon!(filetype_cpp, "/icons/material-cpp.svg"; feature = "text-editor");
declare_icon!(filetype_csharp, "/icons/material-csharp.svg"; feature = "text-editor");
declare_icon!(filetype_go, "/icons/material-go.svg"; feature = "text-editor");
declare_icon!(filetype_ruby, "/icons/material-ruby.svg"; feature = "text-editor");
declare_icon!(filetype_php, "/icons/material-php.svg"; feature = "text-editor");
declare_icon!(filetype_swift, "/icons/material-swift.svg"; feature = "text-editor");
declare_icon!(filetype_kotlin, "/icons/material-kotlin.svg"; feature = "text-editor");
declare_icon!(filetype_scala, "/icons/material-scala.svg"; feature = "text-editor");
declare_icon!(filetype_lua, "/icons/material-lua.svg"; feature = "text-editor");
declare_icon!(filetype_console, "/icons/material-console.svg"; feature = "text-editor");
declare_icon!(filetype_powershell, "/icons/material-powershell.svg"; feature = "text-editor");
declare_icon!(filetype_html, "/icons/material-html.svg"; feature = "text-editor");
declare_icon!(filetype_css, "/icons/material-css.svg"; feature = "text-editor");
declare_icon!(filetype_sass, "/icons/material-sass.svg"; feature = "text-editor");
declare_icon!(filetype_less, "/icons/material-less.svg"; feature = "text-editor");
declare_icon!(filetype_json, "/icons/material-json.svg"; feature = "text-editor");
declare_icon!(filetype_yaml, "/icons/material-yaml.svg"; feature = "text-editor");
declare_icon!(filetype_toml, "/icons/material-toml.svg"; feature = "text-editor");
declare_icon!(filetype_xml, "/icons/material-xml.svg"; feature = "text-editor");
declare_icon!(filetype_markdown, "/icons/material-markdown.svg"; feature = "text-editor");
declare_icon!(filetype_pdf, "/icons/material-pdf.svg"; feature = "text-editor");
declare_icon!(filetype_word, "/icons/material-word.svg"; feature = "text-editor");
declare_icon!(filetype_table, "/icons/material-table.svg"; feature = "text-editor");
declare_icon!(filetype_powerpoint, "/icons/material-powerpoint.svg"; feature = "text-editor");
declare_icon!(filetype_image, "/icons/material-image.svg"; feature = "text-editor");
declare_icon!(filetype_audio, "/icons/material-audio.svg"; feature = "text-editor");
declare_icon!(filetype_video, "/icons/material-video.svg"; feature = "text-editor");
declare_icon!(filetype_zip, "/icons/material-zip.svg"; feature = "text-editor");
declare_icon!(filetype_font, "/icons/material-font.svg"; feature = "text-editor");
declare_icon!(filetype_database, "/icons/material-database.svg"; feature = "text-editor");
declare_icon!(filetype_docker, "/icons/material-docker.svg"; feature = "text-editor");
declare_icon!(filetype_git, "/icons/material-git.svg"; feature = "text-editor");
declare_icon!(filetype_lock, "/icons/material-lock.svg"; feature = "text-editor");
declare_icon!(filetype_settings, "/icons/material-settings.svg"; feature = "text-editor");
declare_icon!(filetype_exe, "/icons/material-exe.svg"; feature = "text-editor");
declare_icon!(filetype_document, "/icons/material-document.svg"; feature = "text-editor");
declare_icon!(filetype_folder_base, "/icons/material-folder-base.svg"; feature = "text-editor");
