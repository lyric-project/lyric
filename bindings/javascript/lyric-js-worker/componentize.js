/**
 * This script is used to generate the wasm file for the worker.js file.
 *
 *
 * Can't upgrade jco to 1.7.1 because of the following error:
 *
 * ```bash
 * component imports instance `wasi:http/types@0.2.0`, but a matching implementation was not found in the linker
 * ```
 */
import { componentize } from '@bytecodealliance/componentize-js';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';

const enableAot = process.env.ENABLE_AOT === '1'

const jsSource = await readFile('bundle/index.bundled.js', 'utf8');

const { component } = await componentize(jsSource, {
    witPath: resolve('./wit'),
    enableAot
});

await writeFile('javascript_worker.wasm', component);