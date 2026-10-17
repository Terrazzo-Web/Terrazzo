import { expect, test } from '@playwright/test';

import { BASE_URL, SECOND } from './text-editor-helpers.mjs';

// Use the adapters and dependencies served by the application, with local
// callbacks so serialization and emissions can be observed independently of saving.
async function createEditor(page, kind, content = 'initial') {
    await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => window.JsDeps?.Milkdown && performance
        .getEntriesByType('resource').some(({ name }) => name.endsWith('/milkdown.js')));
    await page.evaluate(async ({ kind, content }) => {
        const moduleUrl = (file) => performance.getEntriesByType('resource')
            .find(({ name }) => name.endsWith(`/${file}.js`)).name;
        const { CodeMirrorJsImpl } = await import(moduleUrl('code_mirror'));
        const { MilkdownJsImpl } = await import(moduleUrl('milkdown'));
        const pane = () => document.body.appendChild(document.createElement('div'));
        const state = { calls: [], flushes: 0, serializations: 0, dirty: false };
        window.editorTest = state;
        const callbacks = [
            (text, immediate) => state.calls.push({ text, immediate }),
            () => {},
            () => state.flushes++,
            (dirty) => { state.dirty = dirty; },
        ];
        if (kind === 'markdown') {
            state.editor = new MilkdownJsImpl(pane(), pane(), null, content,
                ...callbacks, null, '/', '/test.md', false);
            await state.editor.createPromise;
            const original = state.editor.crepe.getMarkdown;
            state.editor.crepe.getMarkdown = () => {
                state.serializations++;
                return original();
            };
            state.editor.wysiwygPane.dataset.testEditor = 'wysiwyg';
        } else {
            state.editor = new CodeMirrorJsImpl(pane(), null, content,
                ...callbacks, null, '/', '/test.txt');
            let prototype = Object.getPrototypeOf(state.editor.editorView.state.doc);
            while (!Object.hasOwn(prototype, 'toString')) prototype = Object.getPrototypeOf(prototype);
            const original = prototype.toString;
            prototype.toString = function () {
                if (this === state.editor.editorView.state.doc) state.serializations++;
                return original.call(this);
            };
        }
    }, { kind, content });
}

test.describe('Editor debounce', () => {
    test.describe.configure({ retries: 5 });

    test.beforeEach(async ({ page }) => {
        page.setDefaultTimeout(10 * SECOND);
        page.setDefaultNavigationTimeout(10 * SECOND);
    });

    test('CodeMirror debounces before serialization and emits the latest document', async ({ page }) => {
        await createEditor(page, 'text');
        const result = await page.evaluate(async () => {
            const state = window.editorTest;
            const edit = (text) => state.editor.editorView.dispatch({
                changes: { from: 0, to: state.editor.editorView.state.doc.length, insert: text },
            });
            edit('first');
            await new Promise((resolve) => setTimeout(resolve, 150));
            edit('last');
            await new Promise((resolve) => setTimeout(resolve, 150));
            const before = { calls: [...state.calls], serializations: state.serializations };
            await new Promise((resolve) => setTimeout(resolve, 150));
            state.editor.destroy();
            return { before, calls: state.calls, serializations: state.serializations, flushes: state.flushes };
        });
        expect(result.before).toEqual({ calls: [], serializations: 0 });
        expect(result.calls).toEqual([{ text: 'last', immediate: false }]);
        expect(result.serializations).toBe(1);
        expect(result.flushes).toBe(1);
    });

    test('CodeMirror close flushes once, cancels the timer, and protects pending edits from reloads', async ({ page }) => {
        await createEditor(page, 'text');
        const result = await page.evaluate(async () => {
            const state = window.editorTest;
            state.editor.editorView.dispatch({ changes: { from: 0, to: 7, insert: 'local' } });
            state.editor.set_content('disk');
            const pending = state.editor.has_pending_changes();
            state.editor.destroy();
            state.editor.destroy();
            await new Promise((resolve) => setTimeout(resolve, 300));
            return { pending, calls: state.calls, serializations: state.serializations, flushes: state.flushes, dirty: state.dirty };
        });
        expect(result).toEqual({
            pending: true, calls: [{ text: 'local', immediate: true }],
            serializations: 1, flushes: 1, dirty: false,
        });
    });

    test('cursor updates and disk replacements do not save', async ({ page }) => {
        await createEditor(page, 'text');
        const calls = await page.evaluate(async () => {
            const { editor, calls } = window.editorTest;
            editor.editorView.dispatch({ selection: { anchor: 1, head: 1 } });
            editor.set_content('disk');
            await new Promise((resolve) => setTimeout(resolve, 300));
            editor.destroy();
            return calls;
        });
        expect(calls).toEqual([]);
    });

    test('Milkdown input marks pending changes without serializing and close flushes them', async ({ page }) => {
        await createEditor(page, 'markdown', '');
        await page.evaluate(() => {
            const state = window.editorTest;
            state.editor.wysiwygPane.addEventListener('input', () => {
                state.input = { dirty: state.dirty, serializations: state.serializations };
                queueMicrotask(() => {
                    state.editor.set_content('disk');
                    state.editor.destroy();
                    state.editor.destroy();
                });
            }, { once: true });
        });
        await page.locator('[data-test-editor="wysiwyg"] [contenteditable="true"]').click();
        await page.keyboard.insertText('typed edit');
        const result = await page.evaluate(() => {
            const { input, calls, flushes, dirty } = window.editorTest;
            return { input, calls, flushes, dirty };
        });
        expect(result.input).toEqual({ dirty: true, serializations: 0 });
        expect(result.calls).toHaveLength(1);
        expect(result.calls[0].text.trim()).toBe('typed edit');
        expect(result.calls[0].immediate).toBe(true);
        expect(result.flushes).toBe(1);
        expect(result.dirty).toBe(false);
    });

    test('Milkdown syncs through its debounced listener without a duplicate save on close', async ({ page }) => {
        await createEditor(page, 'markdown', '');
        await page.locator('[data-test-editor="wysiwyg"] [contenteditable="true"]').click();
        await page.keyboard.insertText('last edit');
        await expect.poll(() => page.evaluate(() => window.editorTest.calls.length)).toBe(1);
        const result = await page.evaluate(async () => {
            const state = window.editorTest;
            state.editor.destroy();
            await new Promise((resolve) => setTimeout(resolve, 300));
            return { calls: state.calls, flushes: state.flushes };
        });
        expect(result.calls).toHaveLength(1);
        expect(result.calls[0].text.trim()).toBe('last edit');
        expect(result.calls[0].immediate).toBe(false);
        expect(result.flushes).toBe(1);
    });

    test('Markdown source close preserves exact source formatting', async ({ page }) => {
        await createEditor(page, 'markdown', '---\ntheme: light\n---');
        const calls = await page.evaluate(async () => {
            const state = window.editorTest;
            const source = state.editor.source.editorView;
            source.dispatch({ changes: { from: 0, to: source.state.doc.length, insert: '---\ntheme: dark\n---' } });
            state.editor.destroy();
            await new Promise((resolve) => setTimeout(resolve, 300));
            return state.calls;
        });
        expect(calls).toEqual([{ text: '---\ntheme: dark\n---', immediate: true }]);
    });

    test('programmatic Markdown replacements do not save, including on close', async ({ page }) => {
        await createEditor(page, 'markdown');
        const calls = await page.evaluate(async () => {
            const state = window.editorTest;
            state.editor.set_content('external');
            await new Promise((resolve) => setTimeout(resolve, 300));
            state.editor.destroy();
            return state.calls;
        });
        expect(calls).toEqual([]);
    });
});
