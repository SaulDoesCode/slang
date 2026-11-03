// The s machine in JavaScript, ported from core/src/rt.rs: the same parser, line, sigils,
// replacements with generations, frames, scripts, lists and commands. Files live in a
// virtual file system; the shell, threads and TCP are not part of a browser. Bead lines
// (BEADS.md §3) fork the line into lanes and join them; try catches an error.
import { MARKS, ASCII, beadTree, mint, BeadError } from './beads.js'

export const SUFFIX = '?!~$`#;:.@'
const RESERVED = ['=', 'n', 'r', 'rc', 'rn', '$', 'err', 'eof', 'out', 'threads', '_', 'args', 'argc', '?']
const S_EQ = 0, S_N = 1, S_R = 2, S_RC = 3, S_RN = 4, S_DOLLAR = 5, S_ERR = 6, S_EOF = 7, S_OUT = 8, S_UNDER = 10, S_ARGS = 11, S_ARGC = 12, S_Q = 13
const UNBOUND = Infinity

// ---------- values ----------
// A value is a number, a text (string), or a word read from code ({ w, k }).

export function fmtNum(x) {
  // shake: weak .repeat
  if (x === 0) return '0'
  if (Number.isNaN(x)) return 'NaN'
  if (x === Infinity) return 'inf'
  if (x === -Infinity) return '-inf'
  const s = String(x)
  if (!s.includes('e')) return s
  // the digits without an exponent, as Rust prints them
  const m = s.match(/^(-?)(\d)(?:\.(\d+))?e([+-]\d+)$/)
  const sign = m[1], digits = m[2] + (m[3] || ''), e = +m[4]
  if (e >= 0) return sign + (e >= digits.length - 1 ? digits + '0'.repeat(e - (digits.length - 1)) : digits.slice(0, e + 1) + '.' + digits.slice(e + 1))
  return sign + '0.' + '0'.repeat(-e - 1) + digits
}
function rustParse(s) {
  if (!/^[+-]?((\d+\.?\d*|\.\d+)([eE][+-]?\d+)?|inf|infinity|nan)$/i.test(s)) return null
  const l = s.toLowerCase().replace(/^\+/, '')
  if (l === 'inf' || l === 'infinity') return Infinity
  if (l === '-inf' || l === '-infinity') return -Infinity
  if (l === 'nan' || l === '-nan') return NaN
  return Number(s)
}
export function parseNum(s) {
  const x = rustParse(s.trim())
  return x === null ? 0 : x
}
export function canonNum(s) {
  const x = rustParse(s)
  if (x === null) return null
  return fmtNum(x) === s ? x : null
}
export const valOf = (s) => { const x = canonNum(s); return x === null ? s : x }
export const text = (v) => (typeof v === 'number' ? fmtNum(v) : typeof v === 'string' ? v : v.w)
export const num = (v) => (typeof v === 'number' ? v : parseNum(text(v)))
function valEq(a, b) {
  if (typeof a === 'number' && typeof b === 'number' && !Number.isNaN(a) && !Number.isNaN(b)) return a === b
  return text(a) === text(b)
}
const isWs = (c) => c === ' ' || c === '\n' || c === '\t' || c === '\r'
const hasWs = (t) => /[ \n\t\r]/.test(t)
export function activeForm(t) {
  const cs = Array.from(t)
  return cs.length > 1 && (cs[0] === '\\' || SUFFIX.includes(cs[cs.length - 1]))
}

// ---------- commands ----------

const CMDS = ['>', '>_', '<', '<_', 'w', '+w', 'mv', 'r', 'r@', '?)', '&&&', '***', 'n', '+', '-', '*', '/', '^', '%', '!!', '=', '!=', ';', '&', '==', '<<', '>>', '<<=', '>>=', 'l', '-l', '[l]', 'l=', 'l-', '#l', '/l', 'l/', '#s', 'f', '+f', '@f', 'f-', 'f_', '_f', '?eof', 't', '-t', '<t', '>t', '#prng', '`prng', '><', '//', '<-', '<--', '(', ')', '~', '##', 'tcp']
const CMD_SET = new Set(CMDS)
export const cmdOf = (w) => (CMD_SET.has(w) ? w : null)
export const CMD_WORDS = CMDS

// ---------- symbols ----------

class Interner {
  constructor() { this.map = new Map(); this.names = []; this.local = []; for (const n of RESERVED) this.sym(n) }
  sym(s) {
    let k = this.map.get(s)
    if (k !== undefined) return k
    k = this.names.length
    this.names.push(s)
    this.local.push(s.length > 1 && s[0] === ':')
    this.map.set(s, k)
    return k
  }
  name(k) { return this.names[k] }
}

// ---------- parsing ----------
// ops: ['push', v] ['interp', parts] ['word', t, k, n] ['cmd', c] ['native', c] ['lookup', k]
// ['pop', k] ['def', k] ['setmem', k] ['setnum', k] ['inc', k] ['dec', k] ['call', k] ['callpass', k] ['cond', bool, op]

export function classify(w, it) {
  const c = cmdOf(w)
  if (c) return ['cmd', c]
  const cs = Array.from(w)
  if (cs.length > 1 && cs[0] === '\\') {
    const rest = cs.slice(1).join('')
    const c2 = cmdOf(rest)
    if (c2) return ['native', c2]
    const op = classify(rest, it)
    if (op[0] === 'word') return ['push', valOf(op[1])]
    return op
  }
  const last = cs[cs.length - 1]
  const stem = cs.slice(0, -1).join('')
  if (stem) {
    switch (last) {
      case '?': return ['cond', true, classify(stem, it)]
      case '!': return ['cond', false, classify(stem, it)]
      case '~': return ['lookup', it.sym(stem)]
      case '@': return ['pop', it.sym(stem)]
      case '$': return ['def', it.sym(stem)]
      case '`': return ['setmem', it.sym(stem)]
      case '#': return ['setnum', it.sym(stem)]
      case ';': return ['inc', it.sym(stem)]
      case ':': return ['dec', it.sym(stem)]
      case '.': {
        const sc = Array.from(stem)
        if (sc.length > 1 && sc[sc.length - 1] === '.') return ['callpass', it.sym(sc.slice(0, -1).join(''))]
        return ['call', it.sym(stem)]
      }
    }
  }
  return ['word', w, it.sym(w), canonNum(w)]
}

// <reader>
function classifyAngle(stem, suf, it) {
  if (!suf) return ['word', stem, it.sym(stem), null]
  if (suf === '..') return ['callpass', it.sym(stem)]
  const ss = Array.from(suf)
  const last = ss[ss.length - 1], head = ss.slice(0, -1).join('')
  if (last === '?' || last === '!') return ['cond', last === '?', classifyAngle(stem, head, it)]
  if (head) throw new Error(`⟨${stem}⟩${suf}: unknown suffix`)
  const m = { '~': 'lookup', '@': 'pop', '$': 'def', '`': 'setmem', '#': 'setnum', ';': 'inc', ':': 'dec', '.': 'call' }[last]
  if (!m) throw new Error(`⟨${stem}⟩${suf}: unknown suffix`)
  return [m, it.sym(stem)]
}

function brace(cs, i, it) {
  const n = cs.length
  let depth = 1, j = i + 1, cur = ''
  const parts = []
  while (j < n) {
    const c = cs[j]
    if (c === '{') { depth++; cur += c; j++ }
    else if (c === '}') {
      depth--
      if (depth === 0) { if (cur) parts.push(['T', cur]); return [parts, j + 1] }
      cur += c; j++
    } else if (c === '~' && depth === 1) {
      let k = j + 1
      while (k < n && cs[k] !== '~' && cs[k] !== '{' && cs[k] !== '}' && !isWs(cs[k])) k++
      if (k < n && cs[k] === '~') {
        if (k === j + 1) cur += '~'
        else {
          if (cur) { parts.push(['T', cur]); cur = '' }
          parts.push(['V', it.sym(cs.slice(j + 1, k).join(''))])
        }
        j = k + 1
      } else { cur += '~'; j++ }
    } else { cur += c; j++ }
  }
  throw new Error('unclosed {')
}

function litOp(prefix, parts) {
  const ps = []
  for (const p of [['T', prefix], ...parts]) {
    if (p[0] === 'T' && !p[1]) continue
    const last = ps[ps.length - 1]
    if (p[0] === 'T' && last && last[0] === 'T') last[1] += p[1]
    else ps.push([...p])
  }
  if (ps.every((p) => p[0] === 'T')) return ['push', valOf(ps.length ? ps[ps.length - 1][1] : '')]
  return ['interp', ps]
}

/** Parsed text: ops and the character offset (in code points) where each starts. */
export function parse(src, it) {
  const cs = Array.from(src)
  const n = cs.length
  const ops = [], spans = []
  let i = 0
  while (i < n) {
    if (isWs(cs[i])) { i++; continue }
    const start = i
    if (cs[i] === '⟨') {
      let j = i + 1
      while (j < n && cs[j] !== '⟩') j++
      if (j >= n) throw new Error('unclosed ⟨')
      const stem = cs.slice(i + 1, j).join('')
      let k = j + 1
      while (k < n && !isWs(cs[k])) k++
      ops.push(classifyAngle(stem, cs.slice(j + 1, k).join(''), it))
      spans.push(start)
      i = k
      continue
    }
    let word = '', done = false, wstart = start
    while (i < n && !isWs(cs[i])) {
      const c = cs[i]
      if (MARKS[c]) {
        if (word) { ops.push(classify(word, it)); spans.push(wstart); word = '' }
        let label = ''
        if (MARKS[c] === 'fork') {
          const r = /^([A-Za-z_][\w-]*):(?=[^\s⟪¦⟫⨝])/.exec(cs.slice(i + 1, i + 40).join(''))
          if (r) { label = r[1]; i += Array.from(r[0]).length }
        }
        ops.push(['mark', MARKS[c], label]); spans.push(i)
        i++; wstart = i
        continue
      }
      if (c === '{') {
        const [parts, j] = brace(cs, i, it)
        ops.push(litOp(word, parts)); word = ''
        spans.push(wstart)
        i = j; done = true
        break
      } else if (c === "'") {
        let j = i + 1
        while (j < n && cs[j] !== "'") j++
        if (j >= n) throw new Error("unclosed '")
        ops.push(['push', valOf(word + cs.slice(i + 1, j).join(''))]); word = ''
        spans.push(wstart)
        i = j + 1; done = true
        break
      } else { word += c; i++ }
    }
    if (!done && word) {
      const am = ASCII[word] ? [ASCII[word], ''] : /^\[\[([A-Za-z_][\w-]*):$/.test(word) ? ['fork', word.slice(2, -1)] : null
      ops.push(am ? ['mark', am[0], am[1]] : classify(word, it)); spans.push(wstart)
    }
  }
  return groupLanes({ ops, spans, src, unit: -1 }, cs, it)
}

// A region from a fork to its join (or the end of its text line) becomes one 'lanes' op
// holding one unit of code per lane.
function groupLanes(u, cs, it) {
  if (!u.ops.some((o) => o[0] === 'mark')) return u
  const lineAt = []
  let ln = 0
  for (let i = 0; i <= cs.length; i++) { lineAt.push(ln); if (cs[i] === '\n') ln++ }
  const toks = u.ops.map((op, i) => op[0] === 'mark'
    ? { m: op[1], label: op[2], at: u.spans[i], line: lineAt[u.spans[i]] }
    : { v: op, at: u.spans[i], line: lineAt[u.spans[i]] })
  let tree
  try { tree = beadTree(toks) } catch (e) { if (e instanceof BeadError) throw new Error(`at ${e.at}: ${e.message}`); throw e }
  const unitOf = (items) => {
    const ops = [], spans = []
    for (const it of items) {
      if (it.region) { ops.push(regionOp(it)); spans.push(it.at) } else { ops.push(it.v); spans.push(it.at) }
    }
    return { ops, spans, src: u.src, unit: u.unit }
  }
  const regionOp = (r) => {
    const forks = r.region.filter((it) => it.fork)
    // ⟪xs…⟫: one lane per item of list xs, the item on the lane's line
    if (forks.length === 1 && forks[0].fork.length === 1 && forks[0].fork[0].length === 1 && forks[0].fork[0][0].v) {
      const op = forks[0].fork[0][0].v
      const name = op[0] === 'word' && op[1].endsWith('…') && op[1].length > 1 ? op[1].slice(0, -1)
        : op[0] === 'callpass' && it.name(op[1]).endsWith('.') ? it.name(op[1]).slice(0, -1) : null
      if (name) return ['lanes', { spread: it.sym(name), shared: unitOf(r.region.filter((x) => !x.fork)), lanes: [], at: r.at }]
    }
    let lanes
    try { lanes = mint(r.region) } catch (e) { if (e instanceof BeadError) throw new Error(`at ${e.at}: ${e.message}`); throw e }
    const simd = forks.every((f) => f.fork.every((lane) => lane.every((it) => it.v && it.v[0] === 'push')))
    return ['lanes', { lanes: lanes.map(unitOf), simd, at: r.at }]
  }
  const out = unitOf(tree)
  out.unit = u.unit
  return out
}
// </reader>

// ---------- random numbers: xoshiro256** seeded by splitmix64, as in rt.rs ----------

const M64 = (1n << 64n) - 1n
const rotl = (x, k) => ((x << BigInt(k)) | (x >> BigInt(64 - k))) & M64
function splitmix(st) {
  st.z = (st.z + 0x9E3779B97F4A7C15n) & M64
  let x = st.z
  x = ((x ^ (x >> 30n)) * 0xBF58476D1CE4E5B9n) & M64
  x = ((x ^ (x >> 27n)) * 0x94D049BB133111EBn) & M64
  return x ^ (x >> 31n)
}
class Rng {
  constructor(a, b) {
    const st = { z: (a ^ rotl(b, 32)) & M64 }
    this.s = [splitmix(st), splitmix(st), splitmix(st), splitmix(st)]
  }
  next() {
    const s = this.s
    const r = (rotl((s[1] * 5n) & M64, 7) * 9n) & M64
    const t = (s[1] << 17n) & M64
    s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3]; s[2] ^= t
    s[3] = rotl(s[3], 45)
    return r
  }
  f64() { return Number(this.next() >> 11n) / 2 ** 53 }
}
const u64 = (x) => BigInt.asUintN(64, BigInt(Math.trunc(x)))
const osU64 = () => BigInt(Math.floor(Math.random() * 2 ** 32)) << 32n | BigInt(Math.floor(Math.random() * 2 ** 32))

// ---------- errors ----------

export class SErr extends Error {}
export class SExit extends Error { constructor(code) { super('exit ' + code); this.code = code } }

// ---------- the machine ----------

/** Counts how often each name is read inside one scope. */
export class Reads extends Map { add(k) { this.set(k, (this.get(k) || 0) + 1); return this } sub(k) { const n = (this.get(k) || 0) - 1; if (n > 0) this.set(k, n); else this.delete(k) } }

export class Rt {
  constructor(opts = {}) {
    this.it = new Interner()
    this.args = []
    this.ow = false
    this.mem = []
    this.nums = []
    this.scripts = []
    this.lists = []
    this.repl = new Map()
    this.replNum = false
    this.gen = 1
    this.bound = UNBOUND
    this.frames = []
    this.units = new Map()
    this.unitIds = new Map()
    this.rng = null // made when #prng or `prng first runs
    this.fs = []
    this.touched = []
    this.lmap = new Map()
    this.site = [-1, 0]
    this.ops = 0
    this.lib = opts.lib || {}
    this.vfs = opts.vfs || new Map()
    this.write = opts.write || ((s) => { if (typeof process !== 'undefined') process.stdout.write(s) })
    this.handles = new Map()
    this.hostWord = null // (rt, text) => true when the host handled the word
    this.stdin = opts.stdin || null
    this.shell = opts.shell || null // (command) => { out, code }, where the host has a shell
    this.natives = new Map()
    this.reads = null    // a set or Reads: the names read in the current scope
    this.writes = null   // a set: names written while a handler runs
    this.tracking = false
    this.lastInterp = null
    this.laneAt = null   // the index of the lane running, for errors
    this.laneStack = []  // forks not yet joined
    this.lanesJoined = null   // the lanes of the last join
    this.laneView = null
    this.onJoin = null   // (record) => void, for an editor that shows lanes
    this.B = { n: Infinity }
    laneWords(this)
  }

  setArgv(argv) {
    this.lists[S_ARGS] = argv.map(valOf)
    this.nset(S_ARGC, argv.length)
  }

  // natives: JS functions that work on the line like library words, overridable with ><
  native(name, f) {
    this.natives.set(name, f)
    const m = { val: name, code: { n: f }, gen: 0, nat: name }
    if (!this.repl.has(name)) this.repl.set(name, [])
    this.repl.get(name).unshift(m)
  }

  fail(msg) { throw new SErr(this.laneAt !== null ? `lane ${this.laneAt}: ${msg}` : msg) }
  pop(who) { if (!this.args.length) this.fail(`${who}: stack is empty`); return this.args.pop() }
  name(k) { const n = this.it.name(k); const i = n.indexOf('\u0001'); return i < 0 ? n : n.slice(0, i) }

  // locals
  l(k) { return this.it.local[k] ? this.localSym(k) : k }
  localSym(k) {
    const d = Math.max(0, this.fs.length - 1)
    const key = k + ',' + d
    let s = this.lmap.get(key)
    if (s === undefined) {
      s = this.it.sym(this.it.name(k) + '\u0001' + d)
      this.it.local[s] = false
      this.lmap.set(key, s)
    }
    this.touch(d, s)
    return s
  }
  touch(d, s) {
    while (this.touched.length <= d) this.touched.push([])
    if (!this.touched[d].includes(s)) this.touched[d].push(s)
  }
  clearLocals(d) {
    if (d < this.touched.length && this.touched[d].length) {
      for (const s of this.touched[d]) { this.mem[s] = undefined; this.nums[s] = undefined; this.lists[s] = undefined; this.scripts[s] = undefined }
      this.touched[d] = []
    }
  }

  // words and replacements
  visible(t) {
    const v = this.repl.get(t)
    if (!v) return null
    for (let i = v.length - 1; i >= 0; i--) if (v[i].gen < this.bound) return v[i]
    return null
  }
  tryExpand(t) {
    const m = this.visible(t)
    if (!m) return false
    if (!m.code) this.args.push(m.val)
    else if (m.code.n) {
      const save = this.bound
      this.bound = m.gen
      m.code.n(this)
      this.bound = save
    } else this.fs.push({ body: { ops: m.code.u }, pc: 0, bound: m.gen, lib: m.lib })
    return true
  }
  wordRc(t, k, n) {
    if (n !== null) {
      if (this.replNum && this.tryExpand(t)) return
      this.args.push(n)
      return
    }
    if (this.preWord && this.preWord(t)) return
    if (this.repl.size && this.tryExpand(t)) return
    if (this.hostWord && this.hostWord(this, t)) return
    this.args.push({ w: t, k })
  }
  cmdSite(c) {
    if (this.repl.size && this.tryExpand(c)) return false
    return this.cmd(c)
  }
  popTo(k) {
    k = this.l(k)
    if (!this.args.length) this.fail(`${this.name(k)}@: stack is empty`)
    this.mset(k, this.args.pop())
  }
  need(n, c) { if (this.args.length < n) this.fail(`${c}: needs ${n} args, stack has ${this.args.length}`) }
  t(i) { return text(this.args[i]) }
  optT(i) { return i < this.args.length ? text(this.args[i]) : null }
  x(i) { return num(this.args[i]) }
  argSym(i) {
    const v = this.args[i]
    const k = typeof v === 'object' ? v.k : this.it.sym(text(v))
    return this.l(k)
  }
  dest(i, def) { return this.args.length > i ? this.argSym(i) : def }

  // variables
  nset(s, x) { this.nums[s] = x; if (this.writes) this.writes.add(s) }
  mset(s, v) { this.mem[s] = v; if (this.writes) this.writes.add(s) }
  wrote(s) { if (this.writes) this.writes.add(s) }
  read(s) { if (this.reads) this.reads.add(s) }
  getVal(s) {
    if (this.reads) this.reads.add(s)
    if (this.mem[s] !== undefined) return this.mem[s]
    if (this.nums[s] !== undefined) return this.nums[s]
    if (s === S_Q) return String(osU64())
    return '404'
  }
  interp(k) { return text(this.getVal(this.l(k))) }
  lookup(k) {
    k = this.l(k)
    if (!this.replNum && this.mem[k] === undefined && this.nums[k] !== undefined) { this.read(k); this.args.push(this.nums[k]); return false }
    return this.dispatch(this.getVal(k))
  }
  /** Runs a looked-up value as a word: one word is executed, anything else is pushed. */
  dispatch(v) {
    let t = null
    if (typeof v === 'number') {
      if (this.replNum && this.tryExpand(fmtNum(v))) return false
    } else if (typeof v === 'object') {
      if (this.repl.size && this.tryExpand(v.w)) return false
      if (this.hostWord && this.hostWord(this, v.w, true)) return false
    } else t = v
    if (t !== null && t && !hasWs(t)) {
      const c = cmdOf(t)
      if (c) return this.cmdSite(c)
      if (activeForm(t) || t[0] === '⟨') {
        let op
        if (t[0] === '⟨') {
          let c2
          // shake: weak parse
          try { c2 = parse(t, this.it) } catch { c2 = null }
          if (!c2 || c2.ops.length !== 1) { this.args.push(v); return false }
          op = c2.ops[0]
        } else op = classify(t, this.it)
        const f = this.step(op, false)
        return f === 'done'
      }
      if (this.repl.size && this.tryExpand(t)) return false
      if (this.hostWord && this.hostWord(this, t, true)) return false
    }
    this.args.push(v)
    return false
  }
  joined() { return this.args.map(text).join(' ') }
  setMem(k) {
    k = this.l(k)
    const v = this.args.length === 1 ? this.args.pop() : valOf(this.joined())
    this.args = []
    this.mset(k, v)
  }
  setNum(k) {
    k = this.l(k)
    const x = this.args.length ? num(this.args[0]) : 0
    this.args = []
    this.nset(k, x)
  }
  incBy(k, sign) {
    k = this.l(k)
    let d = 1
    if (this.args.length) { d = num(this.args[0]); this.args = [] }
    const x = this.nums[k] ?? 0
    this.nset(k, x + sign * d)
  }

  // scripts and units
  codeOf(src) {
    let u = this.units.get(src)
    if (u) return u
    // shake: weak parse
    u = this.reader ? this.reader(src) : parse(src, this.it)
    u.unit = this.unitIds.has(src) ? this.unitIds.get(src) : -1
    if (this.units.size > 4096) this.units.clear()
    this.units.set(src, u)
    return u
  }
  defScript(k) {
    k = this.l(k)
    const body = this.joined().replace(/\|/g, '~')
    this.args = []
    let u
    try { u = this.codeOf(body) } catch (e) { this.fail(`${this.name(k)}$: ${e.message}`) }
    this.scripts[k] = { u, b: this.gen }
  }
  libTexts() { return this._libTexts || (this._libTexts = new Set(Object.values(this.lib))) }
  /** Whether the running code is the body of a word a library file defined. */
  inLib() { const f = this.fs[this.fs.length - 1]; return !!(f && f.lib) }
  libSrc(path) { const p = path.replace(/^\.\//, ''); return this.lib[p] ?? null }
  fileText(path) {
    const p = path.replace(/^\.\//, '')
    if (this.vfs.has(p)) return this.vfs.get(p)
    return this.libSrc(p)
  }
  frameFor(k) {
    const sc = this.scripts[k]
    if (sc) {
      if (sc.n) { sc.n(this); return null }
      return { body: { ops: sc.u }, pc: 0, bound: sc.b }
    }
    const name = this.name(k)
    // shake: weak .fileText .fileFrame
    if (this.fileFrame && this.fileText(name) !== null) return this.fileFrame(name)
    this.fail(`${name}.: no script or file named '${name}'`)
  }
  call(k) { const f = this.frameFor(k); if (f) this.fs.push(f) }
  count(i, w) {
    const v = this.args[i]
    const x = typeof v === 'number' ? v : (rustParse(text(v).trim()) ?? NaN)
    if (Number.isFinite(x) && x >= 0 && Number.isInteger(x)) return x
    this.fail(`${w}: repeat count must be a whole number >= 0, got '${text(v)}'`)
  }
  repeat(k) {
    // shake: uses .repStep
    const n = this.count(0, this.name(k) + '.')
    this.fs.push({ body: { rep: { k, i: 0, n, amp: false } }, pc: 0, bound: this.bound })
  }
  callSite(k) { k = this.l(k); if (this.args.length === 1) this.repeat(k); else this.call(k) }
  srcFrame(src) {
    let u
    try { u = this.codeOf(src) } catch (e) { this.fail('parse: ' + e.message) }
    return { body: { ops: u }, pc: 0, bound: UNBOUND }
  }
  runSrc(src) { this.fs.push(this.srcFrame(src)) }
  fileFrame(path) {
    const t = this.fileText(path)
    if (t === null) this.fail(`${path}: No such file or directory (os error 2)`)
    const f = this.srcFrame(t)
    if (this.libTexts().has(t)) f.lib = true
    return f
  }
  /** A frame that calls a JS function once when it is reached: the host's own steps. */
  hostFrame(fn) { return { body: { host: fn }, pc: 0, bound: this.bound } }

  // running
  guarded(f) {
    try { f(this); return null } catch (e) { return this.statusOf(e) }
  }
  statusOf(e) {
    let st
    if (e instanceof SErr) st = { error: e.message }
    else if (e instanceof SExit) st = { exit: e.code }
    else if (e instanceof RangeError) st = { error: 'too deep: ' + e.message }
    else throw e
    this.resetFrames()
    // shake: weak .endView .useState
    if (this.laneView) this.endView()
    while (this.laneStack.length) this.useState(this.laneStack.pop().fork)
    this.laneAt = null
    return st
  }
  resetFrames() {
    while (this.fs.length) { this.fs.pop(); this.clearLocals(this.fs.length) }
  }
  /** Runs text to the end (or the budget): 'done', 'ran', { error }, { exit }. */
  runText(src, budget = Infinity) {
    const r = this.guarded((rt) => rt.runSrc(src))
    if (r) return r
    return this.run(budget)
  }
  run(budget = Infinity) {
    if (budget > Number.MAX_SAFE_INTEGER) budget = Number.MAX_SAFE_INTEGER
    const left = { n: budget }
    const saved = this.B
    this.B = left
    try {
      const st = this.runTo(0)
      this.ops += budget - left.n
      return st
    } catch (e) {
      this.ops += budget - left.n
      return this.statusOf(e)
    } finally { this.B = saved }
  }
  /** Runs frames until the stack is back to depth stop; an error goes to the nearest try above stop. */
  runTo(stop) {
    for (;;) {
      // shake: weak .recover
      try { return this.runInner(this.B, stop) } catch (e) {
        if (!(e instanceof SErr) || !this.recover || !this.recover(e, stop)) throw e
      }
    }
  }
  recover(e, stop) {
    let i = this.fs.length - 1
    while (i >= stop && !this.fs[i].catch) i--
    if (i < stop) return false
    const c = this.fs[i].catch
    while (this.fs.length > i) { this.fs.pop(); this.clearLocals(this.fs.length) }
    this.frames.length = Math.min(this.frames.length, c.frames)
    // shake: weak .useState
    while (this.laneStack.length > c.lanes) this.useState(this.laneStack.pop().fork)
    this.laneAt = c.laneAt
    this.args = []
    this.mset(S_ERR, valOf(e.message))
    this.runSrc(c.h)
    return true
  }
  /** After an op of compiled code: runs what it pushed, then restores the frame's bound. */
  st(d) {
    if (this.fs.length > d + 1) this.runTo(d + 1)
    this.bound = this.fs[d].bound
  }
  endFrame(idx) {
    this.clearLocals(idx)
    this.fs.splice(idx, 1)
    if (idx < this.touched.length) this.touched.splice(idx, 1)
  }
  tailInto(idx, k) {
    this.clearLocals(idx)
    const f = this.frameFor(k)
    if (f) {
      if (this.fs.length > idx + 1) {
        const rest = this.fs.splice(idx + 1)
        this.fs[idx] = f
        this.fs.push(...rest)
      } else this.fs[idx] = f
    } else this.endFrame(idx)
  }
  canFastTail(idx) { return this.fs.length === idx + 1 && !(this.touched[idx] && this.touched[idx].length) }
  runInner(budget, stop = 0) {
    for (;;) {
      const n = this.fs.length
      if (n <= stop) return 'done'
      if (budget.n <= 0) return 'ran'
      const idx = n - 1
      const fr = this.fs[idx]
      this.bound = fr.bound
      const body = fr.body
      // shake: weak .interpret .repStep .tailInto
      if (body.ops && body.ops.fn) {
        budget.n--
        const r = body.ops.fn(this, idx)
        if (r && r.tail !== undefined) this.tailInto(idx, r.tail)
        else this.endFrame(idx)
      } else if (body.ops) {
        if (this.interpret(budget, idx, fr)) return 'ran'
      } else if (body.rep) {
        budget.n--
        this.repStep(idx, body.rep)
      } else if (body.host) {
        budget.n--
        this.endFrame(idx)
        body.host(this)
      }
    }
  }
  /** Runs the ops of frame idx one at a time, until the frame ends or pushes another; true
   *  when the budget ran out. */
  interpret(budget, idx, fr) {
    let code = fr.body.ops
    let len = code.ops.length
    let pc = fr.pc
    for (;;) {
      let out = 'end', tailK = 0
      while (pc < len) {
        if (budget.n <= 0) { this.fs[idx].pc = pc; return true }
        budget.n--
        this.site = [code.unit, pc]
        this.siteCode = code
        const op = code.ops[pc]
        pc++
        const f = this.step(op, pc === len)
        if (f === 'done') { this.fs.length = idx + 1; out = 'end'; break }
        if (f && f.tail !== undefined) { out = 'tail'; tailK = f.tail; break }
        if (this.fs.length !== idx + 1) { out = 'pushed'; break }
      }
      if (out === 'end') this.endFrame(idx)
      else if (out === 'pushed') {
        if (pc === len) this.endFrame(idx)
        else this.fs[idx].pc = pc
      } else {
        if (this.canFastTail(idx)) {
          const sc = this.scripts[tailK]
          if (sc && sc.u) {
            this.fs[idx].body = { ops: sc.u }
            this.fs[idx].bound = sc.b
            this.fs[idx].pc = 0
            this.bound = sc.b
            code = sc.u
            len = code.ops.length
            pc = 0
            continue
          }
        }
        this.tailInto(idx, tailK)
      }
      return false
    }
  }
  /** One turn of a repeat frame: the script runs again, or the frame ends. */
  repStep(idx, r) {
    if (r.i < r.n) {
      const cur = r.i
      r.i++
      if (r.amp) this.nset(S_N, cur)
      this.args = []
      this.call(r.k)
    } else {
      this.args = []
      this.endFrame(idx)
    }
  }

  step(op, tail) {
    // shake: weak .lanesOp
    switch (op[0]) {
      case 'push': this.args.push(op[1]); break
      case 'interp': this.interpOp(op[1]); break
      case 'lanes': this.lanesOp(op[1]); break
      case 'word': this.wordRc(op[1], op[2], op[3]); break
      case 'cmd': if (this.cmdSite(op[1])) return 'done'; break
      case 'native': if (this.cmd(op[1])) return 'done'; break
      case 'pop': this.popTo(op[1]); break
      case 'lookup': if (this.lookup(op[1])) return 'done'; break
      case 'def': this.defScript(op[1]); break
      case 'setmem': this.setMem(op[1]); break
      case 'setnum': this.setNum(op[1]); break
      case 'inc': this.incBy(op[1], 1); break
      case 'dec': this.incBy(op[1], -1); break
      case 'call': {
        const k = this.l(op[1])
        if (tail) {
          if (this.args.length === 1) { this.repeat(k); return null }
          return { tail: k }
        }
        if (this.args.length === 1) this.repeat(k)
        else this.call(k)
        break
      }
      case 'callpass': {
        const k = this.l(op[1])
        if (tail) return { tail: k }
        this.call(k)
        break
      }
      case 'cond': if (this.ow === op[1]) return this.step(op[2], tail); break
    }
    return null
  }

  interpOp(parts) {
    const outer = this.reads
    const mine = this.tracking ? new Set() : null
    if (mine) this.reads = mine
    let s = ''
    for (const p of parts) s += p[0] === 'T' ? p[1] : this.interp(p[1])
    this.reads = outer
    const v = valOf(s)
    this.args.push(v)
    if (mine) {
      if (outer) for (const x of mine) outer.add(x)
      this.lastInterp = { v, parts, reads: mine, at: this.args.length }
    }
  }

  // ---------- lanes ----------
  state() { return { mem: this.mem, nums: this.nums, lists: this.lists, scripts: this.scripts, repl: this.repl } }
  useState(s) { this.mem = s.mem; this.nums = s.nums; this.lists = s.lists; this.scripts = s.scripts; this.repl = s.repl }
  copyState(s) {
    return {
      mem: s.mem.slice(), nums: s.nums.slice(), lists: s.lists.map((l) => l && l.slice()), scripts: s.scripts.slice(),
      repl: new Map([...s.repl].map(([k, v]) => [k, v.slice()])),
    }
  }
  /** The names a lane wrote: what differs from the state at the fork. */
  laneWrites(st, fork) {
    const w = new Set(), r = new Set()
    const n = Math.max(st.mem.length, st.nums.length, st.lists.length, st.scripts.length)
    for (let s = 0; s < n; s++) {
      const a = st.lists[s], b = fork.lists[s]
      const listDiff = a !== b && (!a || !b || a.length !== b.length || a.some((x, i) => x !== b[i]))
      if (st.mem[s] !== fork.mem[s] || st.nums[s] !== fork.nums[s] || st.scripts[s] !== fork.scripts[s] || listDiff) w.add(s)
    }
    for (const [k, v] of st.repl) { const f = fork.repl.get(k); if (!f || f.length !== v.length || v.some((x, i) => x !== f[i])) r.add(k) }
    return { names: w, repl: r }
  }
  lanesOp(L) {
    const base = this.args
    const rec = { L, fork: this.state(), lanes: [], laneAt: this.laneAt, view: this.laneView, ow0: this.ow }
    this.endView()
    this.laneStack.push(rec)
    const items = L.spread != null ? (this.lists[this.l(L.spread)] || []).slice() : null
    const n = items ? items.length : L.lanes.length
    const bound = this.bound
    let i = -1
    // one lane at a time: this frame ends the lane before it and starts the next
    const drive = () => {
      if (i >= 0) {
        const st = this.state()
        rec.lanes[i] = { args: this.args, st, w: this.laneWrites(st, rec.fork) }
        this.useState(rec.fork)
      }
      i++
      if (i >= n) { this.joinLanes(rec, base); return }
      this.fs.push(this.hostFrame(drive))
      this.args = items ? [...base, items[i]] : base.slice()
      this.useState(this.copyState(rec.fork))
      this.laneAt = i
      this.ow = rec.ow0
      this.fs.push({ body: { ops: items ? L.shared : L.lanes[i] }, pc: 0, bound })
    }
    this.fs.push(this.hostFrame(drive))
    this.args = []
  }
  joinLanes(rec, base) {
    this.useState(rec.fork)
    this.ow = rec.ow0
    this.laneStack.pop()
    this.laneAt = rec.laneAt
    this.args = base
    this.lanesJoined = rec
    if (this.onJoin) this.onJoin(rec)
  }
  joinedLanes(who) {
    if (!this.lanesJoined) this.fail(`${who}: no joined lanes`)
    return this.lanesJoined.lanes
  }
  /** Lane i's writes are read first until endlane. */
  viewLane(i) {
    this.endView()
    const lane = this.joinedLanes('lane')[i]
    if (!lane) this.fail(`lane: no lane ${i}, there are ${this.lanesJoined.lanes.length}`)
    const saved = []
    for (const s of lane.w.names) {
      saved.push([s, this.mem[s], this.nums[s], this.lists[s], this.scripts[s]])
      this.mem[s] = lane.st.mem[s]; this.nums[s] = lane.st.nums[s]; this.lists[s] = lane.st.lists[s]; this.scripts[s] = lane.st.scripts[s]
    }
    const savedR = []
    for (const k of lane.w.repl) { savedR.push([k, this.repl.get(k)]); this.repl.set(k, lane.st.repl.get(k)) }
    this.laneView = { lane, saved, savedR }
  }
  endView() {
    const v = this.laneView
    if (!v) return
    const st = v.lane.st
    for (const [s, m, x, l, sc] of v.saved) {
      if (this.mem[s] === st.mem[s]) this.mem[s] = m
      if (this.nums[s] === st.nums[s]) this.nums[s] = x
      if (this.lists[s] === st.lists[s]) this.lists[s] = l
      if (this.scripts[s] === st.scripts[s]) this.scripts[s] = sc
    }
    for (const [k, r] of v.savedR) if (this.repl.get(k) === st.repl.get(k)) { if (r) this.repl.set(k, r); else this.repl.delete(k) }
    this.laneView = null
  }

  onErr(i, msg) {
    if (this.args.length > i) {
      const script = this.t(i)
      this.args = []
      this.mset(S_ERR, valOf(msg))
      this.runSrc(script)
    } else this.fail(msg)
  }
  list(k) { if (!this.lists[k]) this.lists[k] = []; return this.lists[k] }
  seedOf() {
    if (!this.args.length) return null
    if (this.args.length === 1) return [u64(this.x(0)), 0n]
    return [u64(this.x(0)), u64(this.x(1))]
  }
  handle(k) {
    const h = this.handles.get(k)
    if (!h) throw new Error('no open file')
    return h
  }

  /** Runs command c on the line. A command returns true when the frame ends, false when the
   *  line is left as it is; otherwise the line is cleared. */
  cmd(c) {
    const f = CMD[c]
    if (!f) this.fail(`${c}: not part of this page`)
    const r = f.call(this, c)
    if (r !== undefined) return r
    this.args = []
    return false
  }
}

// ---------- the commands, one function each (a compiled page keeps the ones it reaches) ----------

function arith(c) {
  this.need(2, c)
  const a = this.x(0), b = this.x(1)
  let r = c === '+' ? a + b : c === '-' ? a - b : c === '*' ? a * b : c === '/' ? a / b : c === '^' ? Math.pow(a, b) : a % b
  if (c === '%' && r < 0) r += Math.abs(b)
  this.nset(this.dest(2, S_EQ), r)
}
function compare(c) {
  this.need(2, c)
  const a = this.args[0], b = this.args[1]
  this.ow = c === '=' ? valEq(a, b) : c === '!=' ? !valEq(a, b) : c === ';' ? this.ow && !valEq(a, b) : c === '&' ? this.ow && valEq(a, b)
    : c === '==' ? num(a) === num(b) : c === '<<' ? num(a) < num(b) : c === '>>' ? num(a) > num(b) : c === '<<=' ? num(a) <= num(b) : num(a) >= num(b)
}
function noThreads(c) { this.fail(`${c}: threads and the network are not part of the browser machine`) }
function prng(rt) {
  const s = rt.seedOf()
  if (s) rt.rng = new Rng(s[0], s[1])
  else if (!rt.rng) rt.rng = new Rng(osU64(), osU64())
  return rt.rng
}

const CMD = {
  '>'() { this.write('\n' + this.args.map((a) => ' ' + text(a)).join('')) },
  '>_'() { this.write(this.args.map(text).join('')) },
  '<'(c) {
    this.need(1, c)
    const k = this.t(0)
    if (!this.shell) this.fail('<: no shell in the browser')
    let o
    try { o = this.shell(k) } catch (e) { this.fail(`<: ${k}: ${e.message}`) }
    this.mset(S_R, valOf(o.out))
    this.nset(S_RC, o.code)
  },
  '<_'() {
    const d = this.dest(0, S_UNDER)
    const line = this.stdin ? this.stdin() : null
    this.mset(d, valOf(line === null ? '' : line))
    this.mset(S_EOF, line === null ? 1 : 0)
  },
  'w'(c) { this.need(2, c); this.vfs.set(this.t(0), this.t(1)) },
  '+w'(c) { this.need(2, c); this.vfs.set(this.t(0), (this.vfs.get(this.t(0)) || '') + this.t(1)) },
  'mv'(c) {
    this.need(2, c)
    const a = this.t(0), b = this.t(1)
    if (!this.vfs.has(a)) this.fail(`mv: ${a} ${b}: No such file or directory (os error 2)`)
    this.vfs.set(b, this.vfs.get(a)); this.vfs.delete(a)
  },
  'r'(c) {
    this.need(1, c)
    const p = this.t(0)
    const k = this.args.length > 1 ? this.argSym(1) : this.argSym(0)
    const s = this.fileText(p)
    if (s === null) this.fail(`r: ${p}: No such file or directory (os error 2)`)
    this.mset(k, valOf(s))
  },
  'r@'(c) {
    this.need(4, c)
    const p = this.t(0), off = this.x(1), len = this.x(2), d = this.argSym(3)
    const s = this.fileText(p)
    if (s === null) this.fail(`r@: ${p}: No such file or directory (os error 2)`)
    const b = new TextEncoder().encode(s).slice(off, off + len)
    this.mset(d, valOf(new TextDecoder().decode(b)))
  },
  '?)'(c) {
    this.need(2, c)
    const table = this.t(0), key = this.t(1), def = this.optT(2)
    this.args = []
    const ws = table.split(/\s+/).filter(Boolean)
    let hit = null
    for (let i = 0; i + 1 < ws.length; i += 2) if (ws[i] === key) { hit = ws[i + 1]; break }
    const code = hit ?? def
    if (code !== null) this.runSrc(code)
    return false
  },
  '&&&'(c) {
    // shake: uses .repStep
    this.need(2, c)
    const n = this.count(0, '&&&')
    const k = this.argSym(1)
    this.fs.push({ body: { rep: { k, i: 0, n, amp: true } }, pc: 0, bound: this.bound })
  },
  '***': noThreads, 't': noThreads, '-t': noThreads, '<t': noThreads, '>t': noThreads, 'tcp': noThreads,
  'n'(c) { this.need(2, c); const x = this.x(0); this.nset(this.argSym(1), x) },
  '+': arith, '-': arith, '*': arith, '/': arith, '^': arith, '%': arith,
  '!!'() { this.ow = !this.ow },
  '=': compare, '!=': compare, ';': compare, '&': compare, '==': compare, '<<': compare, '>>': compare, '<<=': compare, '>>=': compare,
  'l'(c) {
    // every value before the list name is an entry, appended in order
    this.need(2, c)
    const n = this.args.length, k = this.argSym(n - 1), l = this.list(k)
    for (let i = 0; i < n - 1; i++) l.push(this.args[i])
    this.wrote(k)
  },
  '-l'(c) { this.need(1, c); const k = this.argSym(0); const l = this.list(k); this.wrote(k); this.mset(S_DOLLAR, l.length ? l.pop() : '404') },
  '[l]'(c) {
    this.need(3, c)
    const k = this.argSym(0); let i = Math.trunc(this.x(1)); const d = this.argSym(2)
    this.read(k)
    const l = this.list(k)
    if (i < 0) i += l.length
    if (i >= 0 && i < l.length) this.mset(d, l[i])
  },
  'l='(c) {
    this.need(3, c)
    const v = this.args[0], k = this.argSym(1); let i = Math.trunc(this.x(2))
    const l = this.list(k)
    this.wrote(k)
    if (i < 0) i += l.length
    if (i >= 0 && i < l.length) l[i] = v
  },
  'l-'(c) { this.need(1, c); const k = this.argSym(0); this.lists[k] = undefined; this.wrote(k) },
  '#l'(c) { this.need(1, c); const k = this.argSym(0); this.read(k); const l = this.lists[k]; this.nset(this.dest(1, S_EQ), l ? l.length : 0) },
  '/l'(c) {
    this.need(2, c)
    const t = this.t(0), k = this.argSym(1), sep = this.optT(2)
    const parts = sep === null ? t.split(/[ \n\t\r\f\v]+/).filter(Boolean) : sep === '' ? Array.from(t) : t.split(sep)
    this.lists[k] = parts.map(valOf)
    this.wrote(k)
  },
  'l/'(c) {
    this.need(2, c)
    const k = this.argSym(0), d = this.argSym(1), sep = this.optT(2) ?? ' '
    this.read(k)
    const l = this.lists[k]
    this.mset(d, valOf(l ? l.map(text).join(sep) : ''))
  },
  '#s'(c) { this.need(1, c); const n = Array.from(text(this.args[0])).length; this.nset(this.dest(1, S_EQ), n) },
  'f'(c) {
    this.need(3, c)
    const p = this.t(0), mode = this.t(2), k = this.argSym(1)
    let data = this.vfs.get(p)
    if (data === undefined) {
      if (mode[0] === 'r') { this.onErr(3, `f: ${p}: No such file or directory (os error 2)`); return false }
      data = ''
    }
    if (mode[0] === 'w') data = ''
    this.vfs.set(p, data)
    this.handles.set(k, { path: p, pos: mode[0] === 'a' ? data.length : 0, eof: false })
  },
  '+f'(c) {
    this.need(3, c)
    const k = this.argSym(0), off = this.x(1), key = this.argSym(2)
    const h = this.handles.get(k)
    if (!h) { this.onErr(3, `+f: ${this.name(k)}: no open file`); return false }
    const data = this.mem[key] !== undefined ? text(this.mem[key]) : ''
    const cur = this.vfs.get(h.path)
    this.vfs.set(h.path, cur.slice(0, off).padEnd(off, '\0') + data + cur.slice(off + data.length))
  },
  '@f'(c) {
    this.need(4, c)
    const k = this.argSym(0), off = this.x(1), len = this.x(2), d = this.argSym(3)
    const h = this.handles.get(k)
    if (!h) this.fail(`@f: ${this.name(k)}: no open file`)
    const s = this.vfs.get(h.path).slice(off, off + len)
    h.eof = s.length < len
    this.mset(d, valOf(s))
  },
  'f-'(c) { this.need(1, c); this.handles.delete(this.argSym(0)) },
  'f_'(c) {
    this.need(2, c)
    const k = this.argSym(0)
    const h = this.handles.get(k)
    if (!h) { this.onErr(2, `f_: ${this.name(k)}: no open file`); return false }
    const cur = this.vfs.get(h.path), line = this.t(1) + '\n'
    this.vfs.set(h.path, cur.slice(0, h.pos) + line + cur.slice(h.pos + line.length))
    h.pos += line.length
  },
  '_f'(c) {
    this.need(2, c)
    const k = this.argSym(0), d = this.argSym(1)
    const h = this.handles.get(k)
    if (!h) this.fail(`_f: ${this.name(k)}: no open file`)
    const s = this.vfs.get(h.path)
    if (h.pos >= s.length) { h.eof = true; this.mset(d, ''); return }
    let e = s.indexOf('\n', h.pos)
    if (e < 0) e = s.length
    let line = s.slice(h.pos, e)
    h.pos = Math.min(s.length, e + 1)
    if (line.endsWith('\r')) line = line.slice(0, -1)
    this.mset(d, valOf(line))
  },
  '?eof'(c) {
    this.need(1, c)
    const h = this.handles.get(this.argSym(0))
    this.mset(S_EOF, !h || h.eof ? 1 : 0)
  },
  '#prng'() { const g = prng(this); this.nset(this.dest(2, S_RN), g.f64()) },
  '`prng'() {
    const AZ = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
    const g = prng(this)
    const d = this.dest(2, S_RN)
    let r = ''
    for (let i = 0; i < 32; i++) r += AZ[Number(g.next() % 52n)]
    this.mset(d, valOf(r))
  },
  '><'(c) {
    this.need(2, c)
    const key = this.t(1)
    const v = this.args[0]
    let u = null
    try { u = this.codeOf(text(v)) } catch {}
    if (canonNum(key) !== null) this.replNum = true
    const gen = this.gen++
    if (!this.repl.has(key)) this.repl.set(key, [])
    const lib = !!(this.siteCode && this.libTexts().has(this.siteCode.src))
    this.repl.get(key).push({ val: v, code: u ? { u } : null, gen, lib })
  },
  '('(c) {
    this.need(1, c)
    const n = this.args.length - 1
    const k = this.count(n, '(')
    this.args.pop()
    const keep = Math.min(k, n)
    this.frames.push(this.args.splice(0, n - keep))
    return false
  },
  ')'() {
    const saved = this.frames.pop()
    if (!saved) this.fail('): no open frame')
    this.args = saved.concat(this.args)
    return false
  },
  '~'() {
    if (!this.args.length) this.fail('~: stack is empty')
    this.runSrc(text(this.args.pop()))
    return false
  },
  '##'() { this.args.push(this.args.length); return false },
  '//'() {},
  '<-'() { return true },
  '<--'() { throw new SExit(this.args.length ? Math.trunc(num(this.args[0])) : 0) },
}

// ---------- words on lanes, and try ----------

function laneWords(rt) {
  const w = (name, f) => rt.native(name, f)
  w('lanes', (r) => { r.args.push(r.joinedLanes('lanes').length) })
  w('lane', (r) => {
    const i = r.count(r.args.length - 1, 'lane')
    r.args.pop()
    r.viewLane(i)
    r.args.push(...r.lanesJoined.lanes[i].args)
  })
  w('endlane', (r) => r.endView())
  w('collect', (r) => { for (const l of r.joinedLanes('collect')) r.args.push(...l.args) })
  w('into', (r) => {
    r.need(1, 'into')
    const k = r.argSym(r.args.length - 1)
    r.args.pop()
    const out = []
    for (const l of r.joinedLanes('into')) out.push(...l.args)
    r.lists[k] = out
    r.wrote(k)
  })
  w('reduce', (r) => {
    const q = text(r.pop('reduce'))
    const lanes = r.joinedLanes('reduce')
    if (!lanes.length) return
    r.args.push(...lanes[0].args)
    for (let i = lanes.length - 1; i >= 1; i--) {
      r.fs.push(r.srcFrame(q))
      r.fs.push(r.hostFrame(() => r.args.push(...lanes[i].args)))
    }
  })
  w('lanewise', (r) => {
    const q = text(r.pop('lanewise'))
    const rec = r.lanesJoined
    const lanes = r.joinedLanes('lanewise')
    r.endView()
    const base = r.args, home = r.state(), ow0 = r.ow
    r.fs.push(r.hostFrame(() => { r.useState(home); r.args = base; r.laneAt = rec.laneAt; r.ow = ow0 }))
    for (let i = lanes.length - 1; i >= 0; i--) {
      r.fs.push(r.hostFrame(() => {
        const st = r.state()
        lanes[i] = { args: r.args, st, w: r.laneWrites(st, rec.fork) }
      }))
      r.fs.push(r.srcFrame(q))
      r.fs.push(r.hostFrame(() => { r.useState(lanes[i].st); r.args = lanes[i].args.slice(); r.laneAt = i; r.ow = ow0 }))
    }
    r.args = []
  })
  w('try', (r) => {
    // shake: uses .recover
    const h = text(r.pop('try'))
    const body = text(r.pop('try'))
    r.args = []
    const f = r.hostFrame(() => {})
    f.catch = { h, frames: r.frames.length, lanes: r.laneStack.length, laneAt: r.laneAt }
    r.fs.push(f)
    r.runSrc(body)
  })
}
