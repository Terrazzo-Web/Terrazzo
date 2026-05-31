class CodeMirrorJsImpl {
    rootView;
    editorView;
    reloadFromDisk; // Set to true when the file is updated from disk
    basePath;
    fullPath;
    constructor(
        element,
        original,
        content,
        onchange,
        basePath,
        fullPath,
    ) {
        this.basePath = basePath;
        this.fullPath = fullPath;
        this.reloadFromDisk = true;
        const updateListener = JsDeps.EditorView.updateListener.of((update) => {
            if (!this.reloadFromDisk && update.docChanged) {
                const content = update.state.doc.toString();
                onchange(content);
            }
        });

        let extensions = [
            JsDeps.basicSetup,
            JsDeps.search({ top: true }),
            JsDeps.lintGutter(),
            JsDeps.oneDark,
            updateListener,
        ];
        const language = getLanguage(fullPath);
        if (language) {
            extensions.push(language());
        }

        if (original) {
            const mergePaneExtensions = [
                JsDeps.EditorView.lineWrapping,
                JsDeps.EditorView.theme({
                    "&": {
                        minWidth: "0",
                        width: "100%",
                    },
                    ".cm-scroller": {
                        overflowX: "auto",
                    },
                    ".cm-content": {
                        minWidth: "0",
                    },
                }),
            ];
            this.rootView = new JsDeps.MergeView({
                a: {
                    doc: original,
                    extensions: [
                        JsDeps.basicSetup,
                        JsDeps.lintGutter(),
                        JsDeps.oneDark,
                        JsDeps.EditorView.editable.of(false),
                        ...mergePaneExtensions,
                    ],
                },
                b: {
                    doc: content,
                    tooltips: JsDeps.tooltips({
                        position: "absolute",
                    }),
                    extensions: [
                        ...extensions,
                        ...mergePaneExtensions,
                    ],
                },
                parent: element,
            });
            this.rootView.dom.style.overflowX = "auto";
            this.rootView.dom
                .querySelectorAll(".cm-mergeViewEditor")
                .forEach((editor) => {
                    editor.style.flex = "1 1 0";
                    editor.style.minWidth = "0";
                    editor.style.width = "0";
                });
            this.editorView = this.rootView.b;
        } else {
            this.rootView = new JsDeps.EditorView({
                state: JsDeps.EditorState.create({
                    doc: content,
                    tooltips: JsDeps.tooltips({
                        position: "absolute",
                    }),
                    extensions,
                }),
                parent: element,
            });
            this.editorView = this.rootView;
        }
        this.reloadFromDisk = false;
    }

    destroy() {
        this.editorView.destroy();
        console.debug(`CodeMirror at path "${this.fullPath}" is destroyed.`);
    }

    set_content(content) {
        const current = this.editorView.state.doc;
        if (current == content) return;
        this.reloadFromDisk = true;
        try {
            const changes = {
                from: 0,
                to: current.length,
                insert: content
            };
            const selection = {
                anchor: Math.min(
                    this.editorView.state.selection.main.anchor,
                    content.length)
            };
            this.editorView.dispatch({
                changes,
                selection
            });
        } finally {
            this.reloadFromDisk = false;
        }
    }

    cargo_check(diagnostics) {
        const lints = [];
        let docLength = this.editorView.state.doc.length;
        for (const diagnostic of diagnostics) {
            if (diagnostic.file_path != this.fullPath)
                continue;
            for (const span of diagnostic.spans) {
                if (!this.fullPath.endsWith(span.file_name))
                    continue;
                if (span.byte_end >= docLength)
                    continue;
                const severity = diagnostic.level == "error" || diagnostic.level == "warning"
                    ? diagnostic.level
                    : "info";
                const lint = {
                    from: span.byte_start,
                    to: span.byte_end,
                    severity,
                    source: "cargo-check",
                    message: diagnostic.message,
                };
                lints.push(lint);
            }
        }
        const setLintsTransaction = JsDeps.setDiagnostics(this.editorView.state, lints);
        this.editorView.dispatch(setLintsTransaction);
    }
}

function getLanguage(fileName) {
    const lastDotIndex = fileName.lastIndexOf('.');
    if (lastDotIndex === -1 || lastDotIndex === fileName.length - 1) {
        return null;
    }

    const ext = fileName.slice(lastDotIndex + 1).toLowerCase();
    return JsDeps.languages[ext] || null;
}

export {
    CodeMirrorJsImpl
};
