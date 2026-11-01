class TerminalJs {
    terminal;
    fitAddon;
    webLinksAddon;
    constructor() {
        this.terminal = new JsDeps.Terminal({});
        this.serializer = new JsDeps.SerializeAddon();
        this.terminal.loadAddon(this.serializer);
        this.ready = false;
        this.disposed = false;
        this.cursor = { event: '0', end: '0' };
        this.lines = 0;
        this.bytes = 0;
        this.lastCapture = -Infinity;
        this.pendingCapture = false;
        this.uploading = false;
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
        const size = this.fitAddon.proposeDimensions();
        if (size && this.ready && (size.rows !== this.terminal.rows || size.cols !== this.terminal.cols)) {
            this.resizeCallback?.(JSON.stringify(size));
        }
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
    session() { return this.ready && this.attachment ? JSON.stringify(this.attachment) : null; }
    rows() {
        return this.terminal.rows;
    }
    cols() {
        return this.terminal.cols;
    }
    onData(callback) {
        this.terminal.onData(data => { if (this.ready) callback(data); });
        this.terminal.onBinary(data => { if (this.ready) callback(Uint8Array.from(data, c => c.charCodeAt(0))); });
    }
    onResize(callback) {
        this.resizeCallback = callback;
    }
    onTitleChange(callback) {
        this.terminal.onTitleChange(callback);
    }
    onCheckpoint(callback) { this.checkpointCallback = callback; }
    async checkpointFinished(success) {
        this.uploading = false;
        if (success) this.terminal.element?.setAttribute('data-checkpoint-event', this.capturedEvent);
        else this.pendingCapture = true;
        this.scheduleCapture();
    }
    scheduleCapture() {
        if (this.disposed || !this.ready || this.uploading || !this.pendingCapture) return;
        clearTimeout(this.captureTimer);
        const delay = 5000 - (performance.now() - this.lastCapture);
        if (delay <= 0) this.captureNow();
        else this.captureTimer = setTimeout(() => this.captureNow(), delay);
    }
    captureNow() {
        if (this.disposed || !this.ready || this.uploading || this.applying || !this.pendingCapture) return;
        const payload = JsDeps.SnapshotAdapter.capture(this.terminal, this.serializer, this.capacity);
        if (!payload) return; // Retry at the next safe output boundary.
        this.lastCapture = performance.now();
        this.lines = this.bytes = 0;
        this.pendingCapture = false;
        this.uploading = true;
        this.capturedEvent = this.cursor.event;
        this.checkpointCallback?.(JSON.stringify({
            version: 1, attachment: this.attachment, cursor: this.cursor,
            size: { r: this.terminal.rows, c: this.terminal.cols }, payload,
        }));
    }
    async send(json) {
        this.applying = true;
        try {
            const parsed = JSON.parse(json);
            const messages = Array.isArray(parsed) ? parsed : [parsed];
            let outputs = [];
            const flush = async () => {
                if (!outputs.length) return;
                const chunks = outputs.map(output => Uint8Array.from(atob(output.base64), c => c.charCodeAt(0)));
                const bytes = new Uint8Array(chunks.reduce((n, chunk) => n + chunk.length, 0));
                let offset = 0;
                for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
                await new Promise(resolve => this.terminal.write(bytes, resolve));
                this.cursor = outputs.at(-1).cursor;
                this.bytes += bytes.length;
                for (const byte of bytes) if (byte === 10) this.lines++;
                if (this.lines >= 1000 || this.bytes >= Math.min(256 * 1024, Math.max(1, Math.floor(this.capacity / 2)))) this.pendingCapture = true;
                outputs = [];
            };
            for (const message of messages) {
                if (message.Output) outputs.push(message.Output);
                else { await flush(); await this.applyMessage(JSON.stringify(message)); }
            }
            await flush();
        } finally { this.applying = false; this.scheduleCapture(); }
    }
    async applyMessage(json) {
        const message = JSON.parse(json);
        if (message.Init) {
            const init = message.Init;
            if (init.version !== 1) throw new Error('Unsupported terminal protocol');
            this.ready = false;
            this.uploading = false;
            clearTimeout(this.captureTimer);
            this.terminal.element?.setAttribute('data-ready', 'false');
            this.attachment = init.attachment;
            this.capacity = init.capacity;
            this.restoring = null;
            this.snapshotChunks = [];
            if (init.fresh) { this.terminal.reset(); this.terminal.resize(80, 24); this.cursor = { event: "0", end: "0" }; }
            if (init.checkpoint) {
                const c = init.checkpoint;
                this.terminal.reset();
                this.terminal.resize(c.size.c ?? c.size.cols, c.size.r ?? c.size.rows);
                this.restoring = c;
                this.snapshotChunks = [];
                this.snapshotBytes = 0;
            }
        } else if (message.SnapshotChunk !== undefined) {
            if (!this.restoring) throw new Error('Unexpected snapshot chunk');
            const bytes = Uint8Array.from(atob(message.SnapshotChunk), c => c.charCodeAt(0));
            this.snapshotBytes += bytes.length;
            if (this.snapshotBytes > this.capacity) throw new Error('Snapshot exceeds capacity');
            this.snapshotChunks.push(bytes);
        } else if (message === 'SnapshotEnd') {
            if (!this.restoring) throw new Error('Unexpected snapshot end');
            const bytes = new Uint8Array(this.snapshotBytes);
            let offset = 0;
            for (const chunk of this.snapshotChunks) { bytes.set(chunk, offset); offset += chunk.length; }
            await JsDeps.SnapshotAdapter.restore(this.terminal, new TextDecoder('utf-8', { fatal: true }).decode(bytes));
            this.cursor = this.restoring.cursor;
            this.restoring = null;
            this.snapshotChunks = [];
        } else if (message.Resize || message.Gap) {
            const event = message.Resize ?? message.Gap;
            if (message.Gap) {
                this.pendingCapture = true;
                JsDeps.SnapshotAdapter.cancelPartialInput(this.terminal);
                this.terminal.element?.setAttribute('data-output-dropped', 'true');
                if (this.terminal.element) this.terminal.element.title = 'Some terminal output was dropped';
            }
            this.terminal.resize(event.size.c ?? event.size.cols, event.size.r ?? event.size.rows);
            this.cursor = { event: event.cursor.event, end: event.cursor.end || this.cursor.end };
        } else if (message === 'Ready') {
            this.ready = true;
            this.terminal.element?.setAttribute('data-ready', 'true');
            this.pendingCapture = true;
            this.fit();
        } else if (message === 'Revoked' || message === 'Eos' || message === 'Disconnected') {
            this.ready = false;
            this.terminal.element?.setAttribute('data-ready', 'false');
            clearTimeout(this.captureTimer);
        }
        this.scheduleCapture();
    }
    dispose() {
        this.disposed = true;
        clearTimeout(this.captureTimer);
        this.terminal.dispose();
        this.webLinksAddon.dispose();
        this.fitAddon.dispose();
    }
}

export {
    TerminalJs
};
