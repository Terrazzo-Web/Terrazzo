import { CodeMirrorJsImpl } from "./code_mirror.js";

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
        onflush,
        ondirty,
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
        this.onflush = onflush;
        this.ondirty = ondirty;
        this.wysiwygDirty = false;
        this.lastFocusedPane = "wysiwyg";
        this.fullPath = fullPath;
        wysiwygPane.inert = true;
        wysiwygPane.dataset.milkdownReady = "false";

        wysiwygPane.addEventListener("focusin", () => {
            this.source.flush();
            this.lastFocusedPane = "wysiwyg";
        });
        sourcePane.addEventListener("focusin", () => {
            this.flushWysiwyg();
            this.lastFocusedPane = "source";
        });
        this.wysiwygPane = wysiwygPane;

        const sourceOnChange = (markdown, immediate) => {
            if (this.destroyed) return;
            this.pendingContent = markdown;
            this.updateMilkdownFromSource(markdown);
            onchange(markdown, immediate);
        };
        this.source = new CodeMirrorJsImpl(
            sourcePane,
            original,
            content,
            sourceOnChange,
            oncursor,
            () => {}, // The wrapper flushes persistence after both panes.
            (dirty) => this.ondirty(dirty || this.wysiwygDirty),
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
        // Track pending input immediately without serializing the document.
        // Milkdown's markdownUpdated listener handles the debounced sync.
        this.onMilkdownInput = () => {
            if (this.destroyed || !this.ready || this.updatingMilkdownFromSource) return;
            this.wysiwygDirty = true;
            this.suppressedMilkdownMarkdown = null;
            this.ondirty(true);
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

    propagateMilkdownMarkdown(markdown, immediate = false) {
        this.wysiwygDirty = false;
        if (this.pendingContent === markdown) {
            this.ondirty(this.source.has_pending_changes());
            return;
        }
        this.pendingContent = markdown;
        this.source.set_content(markdown);
        this.onchange(markdown, immediate);
        this.ondirty(this.source.has_pending_changes());
    }

    set_collapse_unchanged(collapse) {
        this.source.set_collapse_unchanged(collapse);
    }

    has_pending_changes() {
        return this.wysiwygDirty || this.source.has_pending_changes();
    }

    // Input marks pending edits; toolbar-only edits may miss this close flush
    // if their debounced markdownUpdated notification has not arrived yet.
    flushWysiwyg(immediate = false) {
        if (this.destroyed || !this.ready || !this.wysiwygDirty) return;
        this.propagateMilkdownMarkdown(this.crepe.getMarkdown(), immediate);
    }

    flush(immediate = false) {
        this.source.flush(immediate);
        this.flushWysiwyg(immediate);
        if (immediate) this.onflush();
    }

    destroy() {
        if (this.destroyed) return;
        this.flush(true);
        this.destroyed = true;
        this.wysiwygPane.removeEventListener("input", this.onMilkdownInput);
        this.source.destroy();
        if (this.ready) {
            this.crepe.destroy();
        }
        console.debug(`Milkdown at path "${this.fullPath}" is destroyed.`);
    }

    set_content(content) {
        if (this.destroyed || this.has_pending_changes()) return;
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
        this.wysiwygPane
            .querySelector('[contenteditable="true"]')
            ?.focus({ preventScroll: true });
    }

    cargo_check(diagnostics) {
        if (this.destroyed) return;
        this.source.cargo_check(diagnostics);
    }
}

export {
    MilkdownJsImpl
};
