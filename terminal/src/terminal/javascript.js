class TerminalJs {
    terminal;
    fitAddon;
    webLinksAddon;
    constructor() {
        this.terminal = new JsDeps.Terminal({});
        this.serializer = new JsDeps.SerializeAddon();
        this.terminal.loadAddon(this.serializer);
        this.position = this.snapshotPosition = 0;
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
    async initialize(process, capacity, snapshot) {
        this.process = process;
        this.capacity = capacity;
        snapshot = JSON.parse(snapshot);
        if (snapshot) {
            await JsDeps.SnapshotAdapter.restore(this.terminal, snapshot.data);
            this.position = this.snapshotPosition = snapshot.position;
        }
    }
    onSnapshot(callback) {
        const capture = async () => {
            try {
                if (this.process && !this.writing && this.position !== this.snapshotPosition) {
                    const position = this.position;
                    const data = JsDeps.SnapshotAdapter.capture(this.terminal, this.serializer, this.capacity);
                    if (data) {
                        await callback(JSON.stringify({ process: this.process, position, data }));
                        this.snapshotPosition = position;
                    }
                }
            } catch (error) { console.warn("Snapshot upload failed", error); }
            finally { if (!this.disposed) this.snapshotTimer = setTimeout(capture, 5000); }
        };
        this.snapshotTimer = setTimeout(capture, 5000);
    }
    async send(data, position) {
        this.writing = true;
        await new Promise(resolve => this.terminal.write(data, resolve));
        this.position = position;
        this.writing = false;
    }
    dispose() {
        this.disposed = true;
        clearTimeout(this.snapshotTimer);
        this.terminal.dispose();
        this.webLinksAddon.dispose();
        this.fitAddon.dispose();
    }
}

export {
    TerminalJs
};
