import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const source = await readFile(new URL('../src/text_editor/ui/code_mirror.js', import.meta.url), 'utf8');
const { ChangeNotifier } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

class Clock {
    time = 0;
    next = 0;
    timers = new Map();
    now = () => this.time;
    setTimeout = (callback, delay) => {
        const id = this.next++;
        this.timers.set(id, { callback, due: this.time + delay });
        return id;
    };
    clearTimeout = (id) => this.timers.delete(id);
    tick(duration) {
        const end = this.time + duration;
        for (;;) {
            const next = [...this.timers].sort((a, b) => a[1].due - b[1].due)[0];
            if (!next || next[1].due > end) break;
            this.time = next[1].due;
            this.timers.delete(next[0]);
            next[1].callback();
        }
        this.time = end;
    }
}

test('continuous typing emits once per second and serializes only the latest body', () => {
    const clock = new Clock();
    const events = [];
    let serializations = 0;
    let body = '';
    const notifier = new ChangeNotifier(value => events.push([clock.now(), value]), clock);
    const getContent = () => { serializations++; return body; };
    for (let i = 0; i < 25; i++) {
        body = String(i);
        notifier.schedule(getContent);
        clock.tick(100);
    }
    clock.tick(500);
    assert.deepEqual(events, [[0, '0'], [1000, '9'], [2000, '19'], [3000, '24']]);
    assert.equal(serializations, 4);
    assert.equal(clock.timers.size, 0);
});

test('teardown flush delivers the pending edit exactly once and cancels the timer', () => {
    const clock = new Clock();
    const events = [];
    const notifier = new ChangeNotifier(value => events.push(value), clock);
    notifier.schedule(() => 'first');
    clock.tick(100);
    notifier.schedule(() => 'last');
    notifier.flush();
    notifier.flush();
    clock.tick(2000);
    assert.deepEqual(events, ['first', 'last']);
    assert.equal(clock.timers.size, 0);
});

test('a timer firing early cannot emit before one second', () => {
    const clock = new Clock();
    const events = [];
    const notifier = new ChangeNotifier(value => events.push([clock.now(), value]), clock);
    notifier.schedule(() => 'first');
    clock.tick(100);
    notifier.schedule(() => 'last');
    const timer = [...clock.timers.values()][0];
    clock.timers.clear();
    clock.tick(400);
    timer.callback();
    assert.equal(events.length, 1);
    clock.tick(500);
    assert.deepEqual(events, [[0, 'first'], [1000, 'last']]);
});
