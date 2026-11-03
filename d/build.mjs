// node d/build.mjs: d.min.js, one classic script that puts $ on window. Uses esbuild from
// studio/web (npm i there).
import { build } from '../studio/web/node_modules/esbuild/lib/main.js'
await build({ entryPoints: [new URL('d.js', import.meta.url).pathname], bundle: true, minify: true, format: 'iife', globalName: '__d', outfile: new URL('d.min.js', import.meta.url).pathname, target: 'es2020', footer: { js: 'window.$=__d.$;' } })
