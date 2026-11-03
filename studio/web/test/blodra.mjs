// node studio/web/test/blodra.mjs: examples/blodra.s in the JS machine against the JS blodra's 300 walks.
import { Rt } from '../s.js'
import { LIB } from '../std.js'
import fs from 'node:fs'
const root = new URL('../../../', import.meta.url).pathname
const cube = fs.readFileSync(root + 'examples/blodra.s', 'utf8')
const walks = fs.readFileSync(root + 'core/tests/blodra.walks', 'utf8')
const want = fs.readFileSync(root + 'core/tests/blodra.expect', 'utf8').trim().split('\n')
let out = ''
const rt = new Rt({ lib: LIB, write: (s) => (out += s) })
const st = rt.runText(cube + '\n' + walks)
if (st && st.error) console.log('error:', st.error)
const got = out.split('\n').map((l) => l.trim()).filter(Boolean)
let ok = 0, bad = 0
want.forEach((w, i) => { if (got[i] === w) ok++; else { bad++; if (bad < 5) console.log(`--- walk ${i + 1}\n  want: ${w}\n  got:  ${got[i]}`) } })
console.log(`${ok} match, ${bad} differ`)
process.exit(bad ? 1 : 0)
