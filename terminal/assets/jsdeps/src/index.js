// JsDeps

import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';

import { Crepe } from '@milkdown/crepe';
import { replaceAll } from '@milkdown/kit/utils';
import '@milkdown/crepe/theme/common/reset.css';
import '@milkdown/crepe/theme/common/prosemirror.css';
import '@milkdown/crepe/theme/common/block-edit.css';
import '@milkdown/crepe/theme/common/code-mirror.css';
import '@milkdown/crepe/theme/common/cursor.css';
import '@milkdown/crepe/theme/common/image-block.css';
import '@milkdown/crepe/theme/common/link-tooltip.css';
import '@milkdown/crepe/theme/common/list-item.css';
import '@milkdown/crepe/theme/common/placeholder.css';
import '@milkdown/crepe/theme/common/toolbar.css';
import '@milkdown/crepe/theme/common/table.css';
import '@milkdown/crepe/theme/common/top-bar.css';
import '@milkdown/crepe/theme/common/diff.css';
import '@milkdown/crepe/theme/frame-dark.css';

import { basicSetup } from "codemirror";
import { EditorState } from '@codemirror/state';
import { EditorView, tooltips } from "@codemirror/view";
import { MergeView } from "@codemirror/merge";
import { search } from "@codemirror/search";

import { oneDark } from '@codemirror/theme-one-dark';
import { lintGutter, setDiagnostics } from '@codemirror/lint';

import { cpp } from "@codemirror/lang-cpp"
import { css } from "@codemirror/lang-css"
import { go } from "@codemirror/lang-go"
import { html } from "@codemirror/lang-html"
import { java } from "@codemirror/lang-java"
import { json } from "@codemirror/lang-json"
import { markdown } from "@codemirror/lang-markdown"
import { python } from "@codemirror/lang-python"
import { sass } from "@codemirror/lang-sass"
import { xml } from "@codemirror/lang-xml"
import { yaml } from "@codemirror/lang-yaml"
import { rust } from "@codemirror/lang-rust"

import { CodeMirrorJsImpl } from "../../../src/text_editor/ui/code_mirror.js";

const languages = {
    cpp, "c++": cpp, "h": cpp, "hpp": cpp,
    css,
    go,
    html,
    java,
    json,
    md: markdown,
    py: python,
    sass,
    xml,
    yml: yaml, yaml,
    rs: rust,
};

// Export them for Webpack to expose as globals
export {
    Terminal,
    FitAddon,
    WebLinksAddon,

    Crepe,
    replaceAll,

    CodeMirrorJsImpl,

    basicSetup,
    EditorState,
    EditorView,
    MergeView,
    search,
    tooltips,

    oneDark,

    lintGutter,
    setDiagnostics,

    languages,
};
