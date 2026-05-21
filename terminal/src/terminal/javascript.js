class TerminalJs {
    terminal;
    fitAddon;
    webLinksAddon;
    disposables;
    constructor() {
        this.terminal = new JsDeps.Terminal({});
        this.fitAddon = new JsDeps.FitAddon();
        this.webLinksAddon = new JsDeps.WebLinksAddon();
        this.disposables = [];
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
    rows() {
        return this.terminal.rows;
    }
    cols() {
        return this.terminal.cols;
    }
    onData(callback) {
        this.disposables.push(this.terminal.onData(callback));
    }
    onResize(callback) {
        this.disposables.push(this.terminal.onResize(callback));
    }
    onTitleChange(callback) {
        this.disposables.push(this.terminal.onTitleChange(callback));
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
        for (const disposable of this.disposables.splice(0)) {
            disposable.dispose();
        }
        this.terminal.dispose();
        this.webLinksAddon.dispose();
        this.fitAddon.dispose();
    }
}
export {
    TerminalJs
};
