import {nodeResolve} from '@rollup/plugin-node-resolve';
export default {input:'viewer.js',output:{file:'viewer.bundle.js',format:'iife'},plugins:[nodeResolve()]};
