// node studio/web/test/corpus.mjs: runs the s 1.0 corpus through the JS machine and
// compares with the recorded output; the shell is node's, and cases that need threads or TCP are named.
import { Rt } from '../s.js'
import { LIB } from '../std.js'
import fs from 'node:fs'
import os from 'node:os'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
const dir = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../../core/tests/cases')
const skip = new Set(['threads.s', 'threads_worker.s', 'rs_tcp.s', 'rs_tcp_client.s', 'par_a.s', 'par_b.s'])
let ok = 0, bad = 0
for (const f of fs.readdirSync(dir).filter((f) => f.endsWith('.s')).sort()) {
  const outp = path.join(dir, f.replace(/\.s$/, '.out'))
  if (!fs.existsSync(outp) || skip.has(f)) continue
  let out = ''
  const vfs = new Map()
  for (const g of fs.readdirSync(dir)) if (!g.endsWith('.out')) vfs.set(g, fs.readFileSync(path.join(dir, g), 'utf8'))
  // the shell runs in a folder of its own
  const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 's-corpus-'))
  const shell = (k) => { const r = spawnSync('sh', ['-c', k], { cwd, encoding: 'utf8' }); if (r.error) throw r.error; return { out: r.stdout, code: r.status ?? -1 } }
  const rt = new Rt({ lib: LIB, vfs, write: (s) => (out += s), shell })
  rt.setArgv([])
  const st = rt.runText(fs.readFileSync(path.join(dir, f), 'utf8'))
  const want = fs.readFileSync(outp, 'utf8')
  if (out === want) ok++
  else {
    bad++
    console.log('--- ' + f + (st && st.error ? '  error: ' + st.error : ''))
    const a = want.split('\n'), b = out.split('\n')
    for (let i = 0; i < Math.max(a.length, b.length); i++) if (a[i] !== b[i]) { console.log('  line ' + (i + 1) + '\n  want: ' + JSON.stringify(a[i]) + '\n  got:  ' + JSON.stringify(b[i])); break }
  }
}
console.log(ok + ' match, ' + bad + ' differ')
