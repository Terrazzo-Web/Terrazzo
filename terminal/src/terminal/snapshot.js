// Private XTermJS 6.0.0 state is isolated here. Update round-trip tests on upgrades.
export function parserSafe(terminal) {
    const input = terminal._core._inputHandler;
    return input._parser.currentState === 0 && !input._utf8Decoder.interim.some(Boolean);
}

function bufferState(buffer, keptScrollback) {
    return {
        x: buffer.x, y: buffer.y,
        tabs: buffer.tabs, scrollTop: buffer.scrollTop, scrollBottom: buffer.scrollBottom,
        savedX: buffer.savedX,
        savedY: Math.max(0, buffer.savedY - Math.max(0, buffer.ybase - keptScrollback)),
        savedCurAttrData: buffer.savedCurAttrData,
        savedCharset: buffer.savedCharset,
    };
}

export function capture(terminal, serializer, capacity) {
    if (!parserSafe(terminal)) return null;
    const core = terminal._core;
    for (let scrollback = 1000; ; scrollback = Math.floor(scrollback / 2)) {
        const payload = JSON.stringify({
            rows: terminal.rows, cols: terminal.cols,
            vt: serializer.serialize({ scrollback }),
            extra: {
                encoding: core.coreMouseService.activeEncoding,
                cursorHidden: core.coreService.isCursorHidden,
                normal: bufferState(core.buffers.normal, scrollback),
                alternate: bufferState(core.buffers.alt, 0),
                charset: core._charsetService.charset,
                charsets: core._charsetService._charsets,
                glevel: core._charsetService.glevel,
            },
        });
        if (new TextEncoder().encode(payload).length <= capacity) return payload;
        if (scrollback === 0) return null;
    }
}

export async function restore(terminal, payload) {
    const state = JSON.parse(payload);
    terminal.resize(state.cols, state.rows);
    await new Promise(resolve => terminal.write(state.vt, resolve));
    const core = terminal._core;
    core.coreMouseService.activeEncoding = state.extra.encoding;
    core.coreService.isCursorHidden = state.extra.cursorHidden;
    for (const [buffer, extra] of [[core.buffers.normal, state.extra.normal], [core.buffers.alt, state.extra.alternate]]) {
        const { savedCurAttrData, ...fields } = extra;
        Object.assign(buffer, fields);
        buffer.savedCurAttrData.fg = savedCurAttrData.fg;
        buffer.savedCurAttrData.bg = savedCurAttrData.bg;
        Object.assign(buffer.savedCurAttrData.extended, savedCurAttrData.extended);
    }
    core._charsetService.charset = state.extra.charset;
    core._charsetService._charsets = state.extra.charsets;
    core._charsetService.glevel = state.extra.glevel;
}
