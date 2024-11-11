import commonjs from '@rollup/plugin-commonjs';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import { getBabelOutputPlugin } from '@rollup/plugin-babel';

export default {
    input: 'worker.js',
    output: {
        file: 'bundle/index.bundled.js',
        format: 'esm',
        plugins: [
            getBabelOutputPlugin({
                plugins: ['@babel/plugin-transform-unicode-property-regex'],
            }),
        ],
    },
    plugins: [
        commonjs(),
        nodeResolve(),
    ],
}