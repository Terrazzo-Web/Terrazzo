class TerminalJs {
    terminal;
    fitAddon;
    webLinksAddon;
    constructor() {
        this.terminal = new JsDeps.Terminal({});
        this.fitAddon = new JsDeps.FitAddon();
        this.webLinksAddon = new JsDeps.WebLinksAddon();
        this.terminal.attachCustomKeyEventHandler((event) => this.handleClipboardShortcut(event));
    }
    handleClipboardShortcut(event) {
        if (event.type !== "keydown" || !event.ctrlKey || event.altKey || event.metaKey) {
            return true;
        }
        if (this.isShortcut(event, "c", "KeyC")) {
            if (!this.terminal.hasSelection()) {
                return true;
            }
            event.preventDefault();
            void this.copySelection();
            return false;
        }
        if (this.isShortcut(event, "v", "KeyV")) {
            event.preventDefault();
            navigator.clipboard.readText().then((text) => this.terminal.paste(text));
            return false;
        }
        return true;
    }
    isShortcut(event, key, code) {
        return event.key?.toLowerCase() === key || event.code === code;
    }
    open(node) {
        this.terminal.loadAddon(this.fitAddon);
        this.terminal.loadAddon(this.webLinksAddon);
        this.terminal.open(node);
    }
    fit() {
        this.fitAddon.fit();
    }
    focus() {
        this.terminal.focus();
    }
    async copySelection() {
        try {
            if (!this.terminal.hasSelection()) {
                return false;
            }
            await navigator.clipboard.writeText(this.terminal.getSelection());
            return true;
        } finally {
            this.terminal.clearSelection();
        }
    }
    async pasteClipboard() {
        this.terminal.paste(await navigator.clipboard.readText());
    }
    rows() {
        return this.terminal.rows;
    }
    cols() {
        return this.terminal.cols;
    }
    onData(callback) {
        this.terminal.onData(callback);
    }
    onResize(callback) {
        this.terminal.onResize(callback);
    }
    onTitleChange(callback) {
        this.terminal.onTitleChange(callback);
    }
    async send(data) {
        let terminalJs = this;
        return new Promise(function (resolve, reject) {
            terminalJs.terminal.write(data, function () {
                resolve(true);
            });
        })
    }
    dispose() {
        this.terminal.dispose();
        this.webLinksAddon.dispose();
        this.fitAddon.dispose();
    }
}
export {
    TerminalJs
};
