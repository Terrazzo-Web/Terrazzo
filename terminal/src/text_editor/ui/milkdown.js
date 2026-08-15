class MilkdownJsImpl {
    crepe;
    source;
    createPromise;
    ready;
    destroyed;
    replacingMilkdown;
    replacementNotificationSeen;
    suppressedMilkdownMarkdown;
    pendingContent;
    lastFocusedPane;
    fullPath;

    constructor(
        element,
        original,
        content,
        onchange,
        oncursor,
        cursorPosition,
        basePath,
        fullPath,
        testMode,
    ) {
        this.ready = false;
        this.destroyed = false;
        this.replacingMilkdown = false;
        this.replacementNotificationSeen = false;
        this.suppressedMilkdownMarkdown = null;
        this.pendingContent = content;
        this.lastFocusedPane = "wysiwyg";
        this.fullPath = fullPath;

        const wysiwygPane = document.createElement("div");
        wysiwygPane.dataset.milkdownPane = "wysiwyg";
        const sourcePane = document.createElement("div");
        sourcePane.dataset.milkdownPane = "source";
        if (testMode) {
            wysiwygPane.classList.add("milkdown-wysiwyg-pane");
            sourcePane.classList.add("milkdown-source-pane");
        }
        wysiwygPane.addEventListener("focusin", () => {
            this.lastFocusedPane = "wysiwyg";
        });
        sourcePane.addEventListener("focusin", () => {
            this.lastFocusedPane = "source";
        });
        element.append(wysiwygPane, sourcePane);
        this.wysiwygPane = wysiwygPane;

        const sourceOnChange = (markdown) => {
            if (this.destroyed) return;
            const canonicalMarkdown = this.replaceMilkdownContent(markdown);
            this.pendingContent = canonicalMarkdown;
            this.source.set_content(canonicalMarkdown);
            onchange(canonicalMarkdown);
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

        this.crepe = new JsDeps.Crepe({
            root: wysiwygPane,
            defaultValue: content,
            features: {
                [JsDeps.Crepe.Feature.Latex]: false,
            },
        });
        this.crepe.on((listener) => {
            listener.markdownUpdated((_ctx, markdown) => {
                if (this.destroyed || !this.ready) return;
                if (this.replacingMilkdown || this.suppressedMilkdownMarkdown === markdown) {
                    this.replacementNotificationSeen ||= this.replacingMilkdown;
                    this.suppressedMilkdownMarkdown = null;
                    this.pendingContent = markdown;
                    this.source.set_content(markdown);
                    return;
                }
                this.pendingContent = markdown;
                this.source.set_content(markdown);
                onchange(markdown);
            });
        });

        this.createPromise = this.crepe.create()
            .then(() => {
                if (this.destroyed) {
                    this.crepe.destroy();
                    return;
                }
                this.ready = true;
                const canonicalMarkdown = this.replaceMilkdownContent(this.pendingContent);
                this.pendingContent = canonicalMarkdown;
                this.source.set_content(canonicalMarkdown);
                this.focus();
            })
            .catch((error) => {
                if (this.destroyed) return;
                console.error(`Failed to create Milkdown at path "${this.fullPath}".`, error);
                wysiwygPane.dataset.milkdownStatus = "error";
                wysiwygPane.textContent = `Failed to load Markdown editor: ${error}`;
            });
    }

    replaceMilkdownContent(content) {
        if (!this.ready || this.destroyed || this.crepe.getMarkdown() === content) return content;
        this.replacingMilkdown = true;
        this.replacementNotificationSeen = false;
        try {
            this.crepe.editor.action(JsDeps.replaceAll(content));
            const canonicalMarkdown = this.crepe.getMarkdown();
            this.suppressedMilkdownMarkdown = this.replacementNotificationSeen ? null : canonicalMarkdown;
            return canonicalMarkdown;
        } finally {
            this.replacingMilkdown = false;
        }
    }

    destroy() {
        if (this.destroyed) return;
        this.destroyed = true;
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
        const canonicalMarkdown = this.replaceMilkdownContent(content);
        this.pendingContent = canonicalMarkdown;
        this.source.set_content(canonicalMarkdown);
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
