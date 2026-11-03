// Bead lines (BEADS.md §2): a line with fork, lane, merge and join marks. beadTree reads
// the marks out of a token list, mint makes the lanes of one region, expand mints text.

export const MARKS = { '⟪': 'fork', '¦': 'lane', '⟫': 'merge', '⨝': 'join' }
export const ASCII = { '[[': 'fork', '||': 'lane', ']]': 'merge', '||>': 'join' }
export const MARK_CHAR = { fork: '⟪', lane: '¦', merge: '⟫', join: '⨝' }

export class BeadError extends Error { constructor(msg, at) { super(msg); this.at = at } }

/** toks: { m, label, at, line } for a mark, { v, at, line } for anything else. The result
 *  holds plain tokens and regions; a region holds plain tokens and forks; a fork holds lanes;
 *  a lane holds plain tokens, unjoined forks and joined regions. */
export function beadTree(toks) {
  let p = 0
  const n = toks.length
  const top = []
  while (p < n) {
    const t = toks[p]
    if (!t.m) { top.push(t); p++; continue }
    if (t.m !== 'fork') throw new BeadError(`${MARK_CHAR[t.m]} outside a fork`, t.at)
    top.push(region())
  }
  return top

  function region() {
    const start = toks[p]
    let line = start.line
    const items = []
    let joined = false
    while (p < n) {
      const t = toks[p]
      if (t.m === 'fork') { const f = fork(); items.push(f); line = f.endLine; continue }
      if (t.m === 'join') { p++; joined = true; break }
      if (t.m) throw new BeadError(`${MARK_CHAR[t.m]} outside a fork`, t.at)
      if (t.line !== line) break
      items.push(t); p++
    }
    return { region: items, joined, at: start.at }
  }

  function fork() {
    const t = toks[p++]
    const lanes = [[]]
    let open = -1 // where the lane's first fork not yet joined stands
    for (;;) {
      if (p >= n) throw new BeadError('⟪ without ⟫', t.at)
      const u = toks[p]
      const lane = lanes[lanes.length - 1]
      if (u.m === 'merge') { p++; return { fork: lanes, label: t.label || '', at: t.at, endLine: u.line } }
      if (u.m === 'lane') { p++; lanes.push([]); open = -1; continue }
      if (u.m === 'fork') { if (open < 0) open = lane.length; lane.push(fork()); continue }
      if (u.m === 'join') {
        if (open < 0) throw new BeadError('⨝ with no open fork', u.at)
        const items = lane.splice(open)
        lane.push({ region: items, joined: true, at: items[0].at })
        open = -1; p++
        continue
      }
      lane.push(u); p++
    }
  }
}

/** The lanes of a region: forks in one set zip, sets multiply, in the order they first
 *  appear; a lane's own unjoined forks multiply within the lane. Each lane is a list of
 *  plain tokens and joined regions. */
export function mint(items) {
  const sets = new Map()
  for (const it of items) {
    if (!it.fork) continue
    const k = it.fork.length, s = sets.get(it.label)
    if (s === undefined || s === 1) sets.set(it.label, k)
    else if (k !== 1 && k !== s) throw new BeadError(`this ⟪ has ${k} lanes where its set has ${s}`, it.at)
  }
  const names = [...sets.keys()]
  let tuples = [[]]
  for (const nm of names) {
    const next = []
    for (const t of tuples) for (let i = 0; i < sets.get(nm); i++) next.push([...t, i])
    tuples = next
  }
  const out = []
  for (const tup of tuples) {
    let vs = [[]]
    for (const it of items) {
      if (!it.fork) { for (const v of vs) v.push(it); continue }
      const lane = it.fork.length === 1 ? it.fork[0] : it.fork[tup[names.indexOf(it.label)]]
      const lv = mint(lane)
      const next = []
      for (const v of vs) for (const l of lv) next.push(v.concat(l))
      vs = next
    }
    out.push(...vs)
  }
  return out
}

/** Text tokens of one text: beads and marks with their offsets (UTF-16) and line numbers.
 *  `⟪name:` labels a fork; a join with a space on each side keeps one of them. */
export function textTokens(text) {
  const toks = []
  let line = 0, cur = '', curAt = 0
  const flush = (at) => { if (cur) toks.push({ v: cur, at: curAt, line }); cur = ''; curAt = at }
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    const m = MARKS[c]
    if (!m) {
      if (!cur) curAt = i
      cur += c
      if (c === '\n') { flush(i + 1); line++ }
      continue
    }
    flush(i)
    let label = ''
    if (m === 'fork') {
      const r = /^([A-Za-z_][\w-]*):(?=[^\s⟪¦⟫⨝])/.exec(text.slice(i + 1, i + 40))
      if (r) { label = r[1]; i += r[0].length }
    }
    toks.push({ m, label, at: i, line })
    if (m === 'join' && text[i - 1] === ' ' && text[i + 1] === ' ') i++
    curAt = i + 1
  }
  flush(text.length)
  return toks
}

/** The plain lines a text mints (§2.7), one text line at a time. */
export function expand(text) {
  const out = []
  for (const line of text.split('\n')) out.push(...expandItems(beadTree(textTokens(line))))
  return out
}
function expandItems(items) {
  let vs = ['']
  for (const it of items) {
    const alts = it.region ? mint(it.region).flatMap(expandItems) : [it.v]
    vs = vs.flatMap((a) => alts.map((b) => a + b))
  }
  return vs
}

/** The lines one text line mints, each with where every character came from:
 *  { s, m } where m[k] is the offset in the line of the character s[k]. */
export function expandMap(line) {
  return mapItems(beadTree(textTokens(line)))
}
function mapItems(items) {
  let vs = [{ s: '', m: [] }]
  for (const it of items) {
    const alts = it.region ? mint(it.region).flatMap(mapItems) : [{ s: it.v, m: Array.from(it.v, (_, k) => it.at + k) }]
    vs = vs.flatMap((a) => alts.map((b) => ({ s: a.s + b.s, m: a.m.concat(b.m) })))
  }
  return vs
}

/** An edit to one minted line of a text line (BEADS §4.4). The minted line i had the
 *  characters p..q replaced by ins. With every = true the edit goes to the beads those
 *  characters came from, so every line minted through them changes. Otherwise only line i
 *  changes: the bead it touched becomes a new fork, or the whole line is mutualized again.
 *  Returns { at, to, ins } (a change to the text line) or null when nothing fits. */
export function mintEdit(line, i, p, q, ins, every) {
  const lines = expandMap(line)
  const { s, m } = lines[i]
  const want = lines.map((l, j) => (j === i ? s.slice(0, p) + ins + s.slice(q) : l.s))
  const norm = (x) => x.trim().split(/\s+/).join(' ')
  const fits = (to) => { try { const e = expand(to); return e.length === want.length && e.every((x, j) => norm(x) === norm(want[j])) } catch { return false } }
  if (every) {
    // the source of p..q, when those characters stand together in the line
    const at = p < q ? m[p] : p > 0 ? m[p - 1] + 1 : s.length ? m[0] : 0
    const to = p < q ? m[q - 1] + 1 : at
    if (p < q && to - at !== q - p) return null
    return { at, to, ins }
  }
  // only line i: the words touched become a fork whose lanes zip with the line's set
  if (p < q && m[q - 1] + 1 - m[p] === q - p || p === q) {
    let a = p, b = q
    while (a > 0 && !/\s/.test(s[a - 1]) && m[a - 1] === m[a] - 1) a--
    while (b < s.length && !/\s/.test(s[b]) && (b === 0 || m[b] === m[b - 1] + 1)) b++
    const at = a < s.length ? m[a] : m[a - 1] + 1
    const to = b > a ? m[b - 1] + 1 : at
    const old = line.slice(at, to)
    const neu = s.slice(a, p) + ins + s.slice(q, b)
    const fork = '⟪' + lines.map((_, j) => (j === i ? neu : old)).join('¦') + '⟫'
    const r = { at, to, ins: fork }
    if (fits(line.slice(0, at) + fork + line.slice(to))) return r
  }
  const mu = mutualize(want)
  if (fits(mu)) return { at: 0, to: line.length, ins: mu }
  return null
}

/** Forks, lanes and minted lines of one text line, for a status line. */
export function lineStats(line) {
  const tree = beadTree(textTokens(line))
  let forks = 0, lanes = 0
  const walk = (items) => {
    for (const it of items) {
      if (it.fork) { forks++; lanes += it.fork.length; it.fork.forEach(walk) }
      else if (it.region) walk(it.region)
    }
  }
  walk(tree)
  return { forks, lanes, minted: expandItems(tree).length }
}

/** Mutualize (§4.5): plain lines folded into one bead line. Tokens common to every line,
 *  in order, become shared beads; what differs between them becomes lanes of one fork. */
export function mutualize(lines) {
  const toks = lines.map((l) => l.trim().split(/\s+/).filter(Boolean))
  let common = toks[0]
  for (const t of toks.slice(1)) common = lcs(common, t)
  // split each line at the common tokens
  const gaps = toks.map((t) => {
    const g = [[]]
    let c = 0
    for (const w of t) {
      if (c < common.length && w === common[c]) { g.push([]); c++ } else g[g.length - 1].push(w)
    }
    return g
  })
  const parts = []
  for (let i = 0; i <= common.length; i++) {
    const alts = gaps.map((g) => g[i].join(' '))
    if (alts.every((a) => a === alts[0])) { if (alts[0]) parts.push(alts[0]) }
    else parts.push('⟪' + alts.join('¦') + '⟫')
    if (i < common.length) parts.push(common[i])
  }
  return parts.join(' ')
}
function lcs(a, b) {
  const m = a.length, n = b.length
  const d = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0))
  for (let i = m - 1; i >= 0; i--) for (let j = n - 1; j >= 0; j--) d[i][j] = a[i] === b[j] ? d[i + 1][j + 1] + 1 : Math.max(d[i + 1][j], d[i][j + 1])
  const out = []
  let i = 0, j = 0
  while (i < m && j < n) {
    if (a[i] === b[j]) { out.push(a[i]); i++; j++ } else if (d[i + 1][j] >= d[i][j + 1]) i++; else j++
  }
  return out
}
