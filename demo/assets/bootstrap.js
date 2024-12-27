import init, { start } from './wasm/terrazzo_demo.js';

function run() {
    start();
}

init().then(run)
