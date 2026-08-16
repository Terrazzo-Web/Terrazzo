class MilkdownJsImpl {
    crepe;
    source;
    createPromise;
    ready;
    destroyed;
    updatingMilkdownFromSource;
    sourceReplacementNotificationSeen;
    suppressedMilkdownMarkdown;
    pendingContent;
    lastFocusedPane;
    fullPath;

    constructor(
        wysiwygPane,
        sourcePane,
        original,
        content,
        onchange,
        oncursor,
        cursorPosition,
        basePath,
        fullPath,
        focusSource,
    ) {
        this.ready = false;
        this.destroyed = false;
        this.updatingMilkdownFromSource = false;
        this.sourceReplacementNotificationSeen = false;
        this.suppressedMilkdownMarkdown = null;
        this.pendingContent = content;
        this.onchange = onchange;
        this.lastFocusedPane = "wysiwyg";
        this.fullPath = fullPath;
        wysiwygPane.inert = true;
        wysiwygPane.dataset.milkdownReady = "false";

        wysiwygPane.addEventListener("focusin", () => {
            this.lastFocusedPane = "wysiwyg";
        });
        sourcePane.addEventListener("focusin", () => {
            this.lastFocusedPane = "source";
        });
        this.wysiwygPane = wysiwygPane;

        const sourceOnChange = (markdown) => {
            if (this.destroyed) return;
            this.pendingContent = markdown;
            this.updateMilkdownFromSource(markdown);
            onchange(markdown);
        };
        this.source = new JsDeps.CodeMirrorJsImpl(
            sourcePane,
            original,
            content,
            sourceOnChange,
            oncursor,
            cursorPosition,
            basePath,
            fullPath,
        );
        // CodeMirror focuses itself during construction. Restore the mode's
        // intended focus target after its focusin handler has run.
        this.lastFocusedPane = focusSource ? "source" : "wysiwyg";

        this.crepe = new JsDeps.Milkdown.Crepe({
            root: wysiwygPane,
            defaultValue: content,
            features: {
                [JsDeps.Milkdown.Crepe.Feature.Latex]: false,
            },
        });
        this.crepe.on((listener) => {
            listener.markdownUpdated((_ctx, markdown) => {
                if (this.destroyed || !this.ready) return;
                if (this.updatingMilkdownFromSource) {
                    this.sourceReplacementNotificationSeen = true;
                    this.suppressedMilkdownMarkdown = null;
                    return;
                }
                if (this.suppressedMilkdownMarkdown === markdown) {
                    this.suppressedMilkdownMarkdown = null;
                    return;
                }
                this.propagateMilkdownMarkdown(markdown);
            });
        });
        this.onMilkdownInput = () => {
            if (this.destroyed || !this.ready || this.updatingMilkdownFromSource) return;
            queueMicrotask(() => {
                if (this.destroyed || !this.ready || this.updatingMilkdownFromSource) return;
                this.propagateMilkdownMarkdown(this.crepe.getMarkdown());
            });
        };
        wysiwygPane.addEventListener("input", this.onMilkdownInput);

        this.createPromise = this.crepe.create()
            .then(() => {
                if (this.destroyed) {
                    this.crepe.destroy();
                    return;
                }
                this.ready = true;
                this.updateMilkdownFromSource(this.pendingContent);
                wysiwygPane.inert = false;
                wysiwygPane.dataset.milkdownReady = "true";
                this.focus();
            })
            .catch((error) => {
                if (this.destroyed) return;
                wysiwygPane.inert = false;
                console.error(`Failed to create Milkdown at path "${this.fullPath}".`, error);
                wysiwygPane.dataset.milkdownStatus = "error";
                wysiwygPane.textContent = `Failed to load Markdown editor: ${error}`;
            });
    }

    updateMilkdownFromSource(content) {
        if (!this.ready || this.destroyed || this.crepe.getMarkdown() === content) return;
        this.updatingMilkdownFromSource = true;
        this.sourceReplacementNotificationSeen = false;
        try {
            this.crepe.editor.action(JsDeps.Milkdown.replaceAll(content));
            this.suppressedMilkdownMarkdown = this.sourceReplacementNotificationSeen
                ? null
                : this.crepe.getMarkdown();
        } finally {
            this.updatingMilkdownFromSource = false;
        }
    }

    propagateMilkdownMarkdown(markdown) {
        if (this.pendingContent === markdown) return;
        this.pendingContent = markdown;
        this.source.set_content(markdown);
        this.onchange(markdown);
    }

    destroy() {
        if (this.destroyed) return;
        this.destroyed = true;
        this.wysiwygPane.removeEventListener("input", this.onMilkdownInput);
        this.source.destroy();
        if (this.ready) {
            this.crepe.destroy();
        }
        console.debug(`Milkdown at path "${this.fullPath}" is destroyed.`);
    }

    set_content(content) {
        if (this.destroyed) return;
        this.pendingContent = content;
        this.source.set_content(content);
        this.updateMilkdownFromSource(content);
    }

    insert_text(text) {
        if (this.destroyed) return;
        this.lastFocusedPane = "source";
        this.source.insert_text(text);
    }

    focus() {
        if (this.destroyed) return;
        if (this.lastFocusedPane === "source") {
            this.source.focus();
            return;
        }
        this.wysiwygPane.querySelector('[contenteditable="true"]')?.focus();
    }

    cargo_check(diagnostics) {
        if (this.destroyed) return;
        this.source.cargo_check(diagnostics);
    }
}

export {
    MilkdownJsImpl
};
