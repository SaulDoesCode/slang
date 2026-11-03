// The editor: s text, right-aligned, with its braces drawn as coloured zones and its bead
// lines drawn as lanes. The text is the file; zones and lanes are how it is shown.
//   middle-drag   a pointer from where the drag began; the wheel picks what the range
//                 becomes (try first), release wraps it; a middle click on a zone retypes or
//                 unwraps it
//   Ctrl+\  split the selection into lanes     Alt+Enter  add a lane    Alt+Backspace  remove it
//   Ctrl+]  merge   Ctrl+Shift+]  join   Ctrl+L  label the fork   Ctrl+M  mutualize lines
//   F6  the minted lines of the caret's line; an edit to one goes back into the beads
import { elementSpec, attrWord, eventWord, selWord, isCode } from './dom.js'
import { expandMap, mintEdit, mutualize, lineStats } from './beads.js'

const MARKS = { '⟪': 'fork', '¦': 'lane', '⟫': 'merge', '⨝': 'join' }
const CMDS = new Set(['>', '>_', '<_', 'w', '+w', 'mv', 'r', 'r@', '?)', '&&&', 'n', '+', '-', '*', '/', '^', '%', '!!', '=', '!=', ';', '&', '==', '<<', '>>', '<<=', '>>=', 'l', '-l', '[l]', 'l=', 'l-', '#l', '/l', 'l/', '#s', '#prng', '`prng', '><', '//', '<-', '<--', '(', ')', '~', '##'])
const SUFFIX = '?!~$`#;:.@'
const LOOPS = new Set(['each', '&&&', 'lanewise', 'reduce', 'every', 'after'])
const STYLES = new Set(['css', 'sheet', 'html', 'title'])
const NOCODE = { repl: new Map(), scripts: [], it: { sym: () => 0 } }

export const KINDS = [
  { kind: 'try', label: 'try', wrap: (x) => [`{${x}} {err~ >} try`, null] },
  { kind: 'el', label: 'element', wrap: (x) => [`{${x}} div`, 'div'] },
  { kind: 'ev', label: 'on click', wrap: (x) => [`{${x}} onclick`, null] },
  { kind: 'loop', label: 'each', wrap: (x) => [`items {${x}} each`, 'items'] },
  { kind: 'loop', label: 'every', wrap: (x) => [`{${x}} 1000 every`, '1000'] },
  { kind: 'page', label: 'page', wrap: (x) => [`{${x}} name page`, 'name'] },
  { kind: 'def', label: 'word', wrap: (x) => [`{${x}} name ><`, 'name'] },
  { kind: 'style', label: 'css', wrap: (x) => [`{${x}} .name css`, '.name'] },
  { kind: 'plain', label: 'group', wrap: (x) => [`{${x}}`, null] },
  { kind: 'lanes', label: 'lanes', wrap: (x) => [`⟪${x}¦⟫ ⨝`, null] },
]

export function wordClass(w) {
  if (elementSpec(w) || selWord(w)) return 'el'
  if (attrWord(w)) return 'attr'
  if (eventWord(w)) return 'ev'
  if (/^[A-Z][\w-]*$/.test(w)) return 'comp'
  if (/^-?\d+(\.\d+)?$/.test(w)) return 'num'
  if (w === '//') return 'note'
  if (w === 'try') return 'try'
  if (CMDS.has(w) || CMDS.has(w.replace(/^\\/, ''))) return 'cmd'
  if (w.length > 1 && SUFFIX.includes(w[w.length - 1])) return 'sig'
  return ''
}
// one-letter commands that read as names, and the commands that take a name from the word
// before them: `x r n` sets nothing called r, it runs r
const SHORT = new Set(['n', 'r', 'w', 'l', 'f', 't'])
const NAMED = new Set(['n', 'r', 'r@', 'l', '-l', '[l]', 'l=', 'l-', '/l', 'l/', 'f', '+f', '@f', 'f-', 'f_', '_f', 't', '-t', '<t', '>t'])
export function clash(w, next) { return SHORT.has(w) && NAMED.has(next) }
function kindOf(w) {
  if (!w) return 'plain'
  if (w === '//') return 'note'
  if (w === 'try') return 'try'
  if (elementSpec(w) || selWord(w)) return 'el'
  if (/^[A-Z][\w-]*$/.test(w)) return 'comp'
  if (attrWord(w)) return 'attr'
  if (eventWord(w)) return 'ev'
  if (LOOPS.has(w)) return 'loop'
  if (w === 'page' || w === 'routes') return 'page'
  if (STYLES.has(w)) return 'style'
  if (w === '><' || (w.length > 1 && w.endsWith('$'))) return 'def'
  if (w === '?)') return 'branch'
  return 'plain'
}

// how words and braces read: a host may read them its own way (Editor opts.word, .zone, .prose)
const PAGE = { word: wordClass, zone: kindOf, prose: (t) => !isCode(NOCODE, t) }
let H = PAGE

// ---------- reading the text into zones and forks ----------

export function lex(text) {
  const root = { t: 'root', kids: [] }
  const stack = [root]
  const n = text.length
  const push = (x) => stack[stack.length - 1].kids.push(x)
  let i = 0
  while (i < n) {
    const c = text[i]
    if (c === '{') { const z = { t: 'zone', a: i, b: -1, kids: [] }; push(z); stack.push(z); i++; continue }
    if (c === '}') {
      if (stack.length > 1) stack.pop().b = i
      else push({ t: 'bad', a: i, b: i + 1 })
      i++
      continue
    }
    if (MARKS[c]) { push({ t: 'mark', m: MARKS[c], a: i, b: i + 1 }); i++; continue }
    if (/\s/.test(c)) {
      let j = i
      while (j < n && /\s/.test(text[j])) j++
      push({ t: 'ws', a: i, b: j })
      i = j
      continue
    }
    if (c === "'") {
      let j = text.indexOf("'", i + 1)
      j = j < 0 ? n : j + 1
      push({ t: 'quote', a: i, b: j })
      i = j
      continue
    }
    let j = i
    while (j < n && !/\s/.test(text[j]) && text[j] !== '{' && text[j] !== '}' && text[j] !== "'" && !MARKS[text[j]]) j++
    push({ t: 'word', a: i, b: j, w: text.slice(i, j) })
    i = j
  }
  shape(root, text)
  return root
}

function shape(node, text) {
  const kids = node.kids
  // a run of braces belongs to the word after it
  for (let k = 0; k < kids.length; k++) {
    if (kids[k].t !== 'zone') continue
    let j = k, run = []
    while (j < kids.length && (kids[j].t === 'zone' || kids[j].t === 'ws')) { if (kids[j].t === 'zone') run.push(kids[j]); j++ }
    let w = j < kids.length && kids[j].t === 'word' ? kids[j] : null
    let kind = H.zone(w && w.w)
    // `{…} name page`, `{…} 1000 every`, `{…} .x css`: one argument stands between
    if (kind === 'plain' && w && kids[j + 1] && kids[j + 1].t === 'ws' && kids[j + 2] && kids[j + 2].t === 'word') {
      const k2 = H.zone(kids[j + 2].w)
      if (['page', 'def', 'style', 'loop'].includes(k2)) { kind = k2; w = kids[j + 2] }
    }
    run.forEach((z, i) => {
      z.kind = kind === 'try' && i > 0 ? 'catch' : kind
      z.by = w
      if (z.b >= 0) z.prose = H.prose(text.slice(z.a + 1, z.b), w && w.w)
    })
    k = j - 1
  }
  for (const k of kids) if (k.t === 'zone') shape(k, text)
  node.kids = forks(kids, text)
}

function forks(kids, text) {
  const out = []
  let i = 0
  while (i < kids.length) {
    if (kids[i].t === 'mark' && kids[i].m === 'fork') { const r = fork(kids, i, text); out.push(r.node); i = r.next } else out.push(kids[i++])
  }
  return out
}
function fork(kids, i, text) {
  const node = { t: 'fork', open: kids[i], lanes: [[]], seps: [], close: null, shared: [], join: null }
  node.a = kids[i].a
  let j = i + 1
  while (j < kids.length) {
    const k = kids[j]
    if (k.t === 'mark' && k.m === 'fork') { const r = fork(kids, j, text); node.lanes[node.lanes.length - 1].push(r.node); j = r.next; continue }
    if (k.t === 'mark' && k.m === 'lane') { node.seps.push(k); node.lanes.push([]); j++; continue }
    if (k.t === 'mark' && k.m === 'merge') { node.close = k; j++; break }
    node.lanes[node.lanes.length - 1].push(k); j++
  }
  if (node.close) {
    while (j < kids.length) {
      const k = kids[j]
      if (k.t === 'mark' && k.m === 'join') { node.join = k; j++; break }
      if (k.t === 'ws' && text.slice(k.a, k.b).includes('\n')) break
      if (k.t === 'mark' && k.m === 'fork') { const r = fork(kids, j, text); node.shared.push(r.node); j = r.next; continue }
      node.shared.push(k); j++
    }
  }
  const first = node.lanes[0][0]
  if (first && first.t === 'word' && first.a === node.open.b && /^[A-Za-z_][\w-]*:./.test(first.w)) {
    const m = /^[A-Za-z_][\w-]*:/.exec(first.w)[0]
    node.label = m.slice(0, -1)
    node.lanes[0].splice(0, 1, { t: 'label', a: first.a, b: first.a + m.length }, { t: 'word', a: first.a + m.length, b: first.b, w: first.w.slice(m.length) })
  }
  return { node, next: j }
}

/** Forks of the text by position: [{ open, seps, close }] from the marks, innermost last. */
function forkMarks(text) {
  const out = [], stack = []
  for (let i = 0; i < text.length; i++) {
    const m = MARKS[text[i]]
    if (m === 'fork') { const f = { open: i, seps: [], close: -1 }; stack.push(f); out.push(f) }
    else if (m === 'lane' && stack.length) stack[stack.length - 1].seps.push(i)
    else if (m === 'merge' && stack.length) stack.pop().close = i
  }
  return out
}

// ---------- the editor ----------

export class Editor {
  constructor(host, opts = {}) {
    this.host = host
    this.text = ''
    this.a = 0 // anchor
    this.b = 0 // caret
    this.goal = null
    this.undos = []
    this.redos = []
    this.lastEdit = 0
    this.err = null
    this.joins = []
    this.forks = []
    this.onChange = opts.onChange || (() => {})
    this.onCaret = opts.onCaret || (() => {})
    this.hooks = { word: opts.word || wordClass, zone: opts.zone || kindOf, prose: opts.prose || PAGE.prose }
    this.onKey = opts.onKey || null       // (e) => true when the host took the key
    this.onHover = opts.onHover || null   // (x, cx, cy) the offset under a resting pointer, or null
    this.onGo = opts.onGo || null         // (x) Ctrl+click at offset x
    this.numbers = !!opts.numbers         // line numbers at the right edge
    this.notes = new Map()                // line index → { text, cls }: written at the left of the line
    this.marks = []                       // { a, b, cls }: ranges drawn under the text
    this.readOnly = false
    this.mint = false
    this.kindIx = 0
    host.classList.add('ed')
    host.innerHTML = `<div class="ed-scroll"><div class="ed-inner"><div class="ed-doc"></div><div class="ed-lines"></div><div class="ed-over"></div><textarea class="ed-input" spellcheck="false" autocomplete="off" aria-label="s text"></textarea></div></div><div class="ed-mint" hidden></div><div class="ed-menu" hidden></div>`
    this.scroll = host.querySelector('.ed-scroll')
    this.inner = host.querySelector('.ed-inner')
    this.doc = host.querySelector('.ed-doc')
    this.over = host.querySelector('.ed-over')
    this.linesEl = host.querySelector('.ed-lines')
    if (this.numbers) host.classList.add('numbered')
    new ResizeObserver(() => this.lines()).observe(this.scroll)
    this.input = host.querySelector('.ed-input')
    this.mintEl = host.querySelector('.ed-mint')
    this.menu = host.querySelector('.ed-menu')
    this.render()
    this.bind()
  }

  get value() { return this.text }
  set value(t) { this.text = t; this.a = this.b = Math.min(this.b, t.length); this.undos = []; this.redos = []; this.render() }
  focus() { this.input.focus({ preventScroll: true }) }
  goTo(i) { this.a = this.b = Math.max(0, Math.min(i, this.text.length)); this.focus(); this.paint(true) }
  setError(i) { this.err = i; this.paint() }
  setJoins(js) { this.joins = js; this.render() }
  /** How each fork ran, from a run of the native machine: [{ at, how }]. */
  setForks(fs) { this.forks = fs; this.render() }
  showBraces(on) { this.host.classList.toggle('show-braces', on); this.paint(); this.lines() }
  /** Text beside lines, after a run: Map of line index → { text, cls }. */
  setNotes(m) { this.notes = m || new Map(); this.lines() }
  setMarks(ms) { this.marks = ms || []; this.paint() }
  setReadOnly(on) { this.readOnly = on; this.host.classList.toggle('readonly', on) }
  setZoom(px) { this.host.style.setProperty('--fs', px + 'px'); this.host.style.setProperty('--lh', Math.round(px * 1.57) + 'px'); this.paint(); this.lines() }
  /** The line index of offset x. */
  lineOf(x) { let n = 0; for (let i = 0; i < x && i < this.text.length; i++) if (this.text[i] === '\n') n++; return n }
  /** The offset where line i starts. */
  lineAt(i) { let x = 0; while (i > 0 && x <= this.text.length) { const e = this.text.indexOf('\n', x); if (e < 0) return this.text.length; x = e + 1; i-- } return x }
  /** Line numbers and the notes beside lines, placed where each line starts. */
  lines() {
    const el = this.linesEl
    if (!el) return
    el.replaceChildren()
    if (!this.numbers && !this.notes.size) return
    const inner = this.inner.getBoundingClientRect()
    const t = this.text
    let i = 0, x = 0
    const frag = document.createDocumentFragment()
    for (;;) {
      const end = t.indexOf('\n', x)
      const c = this.caretRect(x)
      if (this.numbers) {
        const d = document.createElement('div')
        d.className = 'ln'
        d.textContent = i + 1
        d.style.top = c.top + 'px'
        frag.append(d)
      }
      const note = this.notes.get(i)
      if (note) {
        const d = document.createElement('div')
        d.className = 'note' + (note.cls ? ' ' + note.cls : '')
        d.textContent = note.text
        d.title = note.text
        d.style.top = c.top + 'px'
        // up to the line's first character
        const first = this.caretRect(x + (/^\s*/.exec(t.slice(x, end < 0 ? t.length : end))[0].length))
        d.style.maxWidth = Math.max(40, first.left - 24) + 'px'
        frag.append(d)
      }
      if (end < 0) break
      x = end + 1
      i++
    }
    el.append(frag)
    void inner
  }

  // ---------- drawing ----------

  render() {
    H = this.hooks
    const root = lex(this.text)
    H = PAGE
    this.index = []
    this.doc.replaceChildren()
    this.seq(root.kids, this.doc, false)
    const end = document.createElement('span')
    end.className = 'ed-end'
    end.textContent = '​'
    this.doc.append(end)
    this.endEl = end
    this.paint()
    this.lines()
    if (this.mint && !this.mintEl.contains(document.activeElement)) this.showMint()
  }
  span(cls, a, b, clone) {
    const s = document.createElement('span')
    if (cls) s.className = cls
    s.dataset.a = a
    s.textContent = this.text.slice(a, b)
    if (!clone) this.index.push({ a, b, node: s.firstChild, el: s })
    return s
  }
  edge(cls, a, clone) {
    const s = document.createElement('span')
    s.className = 'edge ' + cls
    s.dataset.a = a
    s.textContent = this.text[a]
    if (!clone) this.index.push({ a, b: a + 1, node: null, el: s })
    return s
  }
  seq(kids, parent, clone, prose = false) {
    for (let ix = 0; ix < kids.length; ix++) {
      const k = kids[ix]
      switch (k.t) {
        case 'ws': parent.append(this.span('', k.a, k.b, clone)); break
        case 'word': {
          const wc = prose ? '' : k.w === '//' ? 'note' : this.hooks.word(k.w)
          const s = this.span(wc ? 'k-' + wc : '', k.a, k.b, clone)
          const next = !prose && kids[ix + 1] && kids[ix + 1].t === 'ws' && !this.text.slice(kids[ix + 1].a, kids[ix + 1].b).includes('\n') && kids[ix + 2] && kids[ix + 2].t === 'word' ? kids[ix + 2].w : null
          if (next && clash(k.w, next)) {
            s.classList.add('k-clash')
            s.title = `${k.w} runs the ${k.w} command here, before ${next} takes its name; {${k.w}} is the name ${k.w} (\\${k.w} runs the command too)`
          }
          parent.append(s)
          break
        }
        case 'quote': parent.append(this.span('k-quote', k.a, k.b, clone)); break
        case 'label': parent.append(this.span('k-label', k.a, k.b, clone)); break
        case 'bad': parent.append(this.edge('brace bad', k.a, clone)); break
        case 'mark': parent.append(this.edge('mark stray', k.a, clone)); break
        case 'zone': {
          const z = document.createElement('span')
          z.className = `zone z-${k.kind}${k.prose ? ' prose' : ''}${k.b < 0 ? ' open' : ''}`
          z.title = k.kind === 'catch' ? 'try: what runs on an error' : k.by ? k.by.w : 'group'
          z.append(this.edge('brace', k.a, clone))
          this.seq(k.kids, z, clone, k.prose || k.kind === 'note')
          if (k.b >= 0) z.append(this.edge('brace', k.b, clone))
          parent.append(z)
          break
        }
        case 'fork': this.forkEl(k, parent, clone); break
      }
    }
  }
  forkEl(f, parent, clone) {
    const box = document.createElement('span')
    box.className = 'fork' + (f.close ? '' : ' open')
    if (f.label) box.dataset.label = f.label
    const how = this.forks.find((x) => x.at === f.open.a)
    if (how && !clone) { box.dataset.how = how.how; box.title = how.how }
    const k = f.lanes.length
    f.lanes.forEach((lane, i) => {
      const row = document.createElement('span')
      row.className = 'lane'
      if (i === 0) row.append(this.edge('mark m-fork', f.open.a, clone))
      this.seq(lane, row, clone)
      if (i < k - 1) {
        row.append(this.edge('mark m-sep', f.seps[i].a, clone))
        if (f.shared.length) {
          const c = document.createElement('span')
          c.className = 'clone'
          this.seq(f.shared, c, true)
          row.append(c)
        }
      } else {
        if (f.close) row.append(this.edge('mark m-merge', f.close.a, clone))
        this.seq(f.shared, row, clone)
      }
      box.append(row)
    })
    parent.append(box)
    if (f.join) parent.append(this.edge('mark m-join', f.join.a, clone))
    const res = this.joins.find((j) => j.at === f.open.a)
    if (res && !clone && res.lines && (res.n != null || res.lines.some((l) => l.length))) {
      const r = document.createElement('span')
      r.className = 'joinres'
      r.contentEditable = 'false'
      const n = res.n ?? res.lines.length
      r.textContent = (res.n != null ? `${n} lane${n === 1 ? '' : 's'} · ` : '') + res.lines.map((l) => l.join(' ') || '·').join('  ¦  ') + (n > res.lines.length ? `  ¦  … ${n - res.lines.length} more` : '')
      r.title = `the lines of the lanes at the join, lane 0 first${n > res.lines.length ? `; the first ${res.lines.length} of ${n}` : ''}`
      parent.append(r)
    }
  }

  // ---------- positions ----------

  entryAt(x) {
    const ix = this.index
    let lo = 0, hi = ix.length - 1
    while (lo <= hi) {
      const m = (lo + hi) >> 1
      if (ix[m].b <= x) lo = m + 1
      else if (ix[m].a > x) hi = m - 1
      else return ix[m]
    }
    return null
  }
  /** The client rectangle of the character at x (or the end marker). */
  charRect(x) {
    const e = this.entryAt(x)
    if (!e) return this.endEl.getBoundingClientRect()
    if (!e.node) return e.el.getBoundingClientRect()
    const r = document.createRange()
    r.setStart(e.node, x - e.a)
    r.setEnd(e.node, x - e.a + 1)
    const rs = r.getClientRects()
    return rs[rs.length - 1] || e.el.getBoundingClientRect()
  }
  caretRect(x) {
    const t = this.text
    const inner = this.inner.getBoundingClientRect()
    let left, top, height
    if (x > 0 && (x === t.length || t[x] === '\n') && t[x - 1] !== '\n') {
      const r = this.charRect(x - 1)
      left = r.right; top = r.top; height = r.height
    } else if (x < t.length && t[x] !== '\n') {
      const r = this.charRect(x)
      left = r.left; top = r.top; height = r.height
    } else {
      // an empty line: at its right edge
      const r = x < t.length ? this.charRect(x) : this.endEl.getBoundingClientRect()
      const d = this.doc.getBoundingClientRect()
      const pad = parseFloat(getComputedStyle(this.doc).paddingRight) || 0
      left = d.right - pad; top = r.top; height = r.height || 21
    }
    return { left: left - inner.left, top: top - inner.top, height: height || 21, cx: left, cy: top + (height || 21) / 2 }
  }
  offsetAt(cx, cy) {
    const hit = document.elementFromPoint(cx, cy)
    const edge = hit && hit.closest && hit.closest('.edge')
    if (edge && this.doc.contains(edge)) {
      const r = edge.getBoundingClientRect()
      return +edge.dataset.a + (cx > r.left + r.width / 2 ? 1 : 0)
    }
    let node = null, off = 0
    if (document.caretPositionFromPoint) { const p = document.caretPositionFromPoint(cx, cy); if (p) { node = p.offsetNode; off = p.offset } }
    else if (document.caretRangeFromPoint) { const r = document.caretRangeFromPoint(cx, cy); if (r) { node = r.startContainer; off = r.startOffset } }
    if (node && this.doc.contains(node)) {
      if (node.nodeType === 3) {
        const s = node.parentElement
        if (s.classList.contains('ed-end')) return this.text.length
        if (s.dataset.a !== undefined && !s.classList.contains('edge')) return Math.min(+s.dataset.a + off, this.text.length)
      }
    }
    // nearest character on the line under the point
    let best = this.text.length, bd = Infinity
    for (const e of this.index) {
      const r = e.el.getBoundingClientRect()
      if (cy < r.top - 2 || cy > r.bottom + 2) continue
      for (const x of [e.a, e.b]) {
        const c = this.caretRect(x)
        const d = Math.abs(c.cx - cx) + Math.abs(c.cy - cy) * 4
        if (d < bd) { bd = d; best = x }
      }
    }
    return best
  }
  rangeRects(a, b) {
    if (a === b) return []
    const ea = this.entryAt(a), eb = this.entryAt(b - 1)
    if (!ea || !eb) return []
    const r = document.createRange()
    if (ea.node) r.setStart(ea.node, a - ea.a); else r.setStartBefore(ea.el)
    if (eb.node) r.setEnd(eb.node, b - eb.a); else r.setEndAfter(eb.el)
    const inner = this.inner.getBoundingClientRect()
    return [...r.getClientRects()].filter((x) => x.width > 0.5).map((x) => ({ left: x.left - inner.left, top: x.top - inner.top, width: x.width, height: x.height }))
  }

  paint(reveal) {
    this.over.replaceChildren()
    for (const m of this.marks) for (const r of this.rangeRects(m.a, m.b)) this.box('mark ' + (m.cls || ''), r)
    const lo = Math.min(this.a, this.b), hi = Math.max(this.a, this.b)
    for (const r of this.rangeRects(lo, hi)) this.box('sel', r)
    if (this.err !== null && this.err < this.text.length) {
      let e = this.err
      while (e < this.text.length && !/\s/.test(this.text[e])) e++
      for (const r of this.rangeRects(this.err, Math.max(e, this.err + 1))) this.box('errbox', r)
    }
    if (this.drag) {
      const [x, y] = this.drag.range
      const k = KINDS[this.kindIx]
      for (const r of this.rangeRects(x, y)) this.box('pre z-' + k.kind, r)
    }
    const c = this.caretRect(this.b)
    this.box('caret', { left: c.left - 1, top: c.top, width: 2, height: c.height })
    this.input.style.left = c.left + 'px'
    this.input.style.top = c.top + 'px'
    if (reveal) {
      const s = this.scroll
      if (c.top < s.scrollTop + 8) s.scrollTop = c.top - 8
      else if (c.top + c.height > s.scrollTop + s.clientHeight - 8) s.scrollTop = c.top + c.height - s.clientHeight + 8
    }
    this.onCaret(this.b)
  }
  box(cls, r) {
    const d = document.createElement('div')
    d.className = cls
    Object.assign(d.style, { left: r.left + 'px', top: r.top + 'px', width: r.width + 'px', height: r.height + 'px' })
    this.over.append(d)
    return d
  }

  // ---------- editing ----------

  sel() { return [Math.min(this.a, this.b), Math.max(this.a, this.b)] }
  selected() { const [x, y] = this.sel(); return this.text.slice(x, y) }
  edit(x, y, ins, caret = x + ins.length, group = '') {
    if (this.readOnly) return
    const now = Date.now()
    if (!(group && group === this.lastGroup && now - this.lastEdit < 1000)) this.undos.push({ text: this.text, a: this.a, b: this.b })
    if (this.undos.length > 500) this.undos.shift()
    this.redos = []
    this.lastEdit = now
    this.lastGroup = group
    this.text = this.text.slice(0, x) + ins + this.text.slice(y)
    this.a = this.b = caret
    this.goal = null
    this.err = null
    this.render()
    this.paint(true)
    this.onChange(this.text)
  }
  replaceSel(ins, group) { const [x, y] = this.sel(); this.edit(x, y, ins, x + ins.length, group) }
  select(a, b) { this.a = a; this.b = b; this.goal = null; this.paint(true) }
  undo() { const u = this.undos.pop(); if (!u) return; this.redos.push({ text: this.text, a: this.a, b: this.b }); this.restore(u) }
  redo() { const u = this.redos.pop(); if (!u) return; this.undos.push({ text: this.text, a: this.a, b: this.b }); this.restore(u) }
  restore(u) { this.text = u.text; this.a = u.a; this.b = u.b; this.lastGroup = ''; this.render(); this.paint(true); this.onChange(this.text) }

  lineStart(x) { return this.text.lastIndexOf('\n', x - 1) + 1 }
  lineEnd(x) { const e = this.text.indexOf('\n', x); return e < 0 ? this.text.length : e }
  step(x, d) {
    const t = this.text
    let y = x + d
    if (y > 0 && y < t.length && /[\uDC00-\uDFFF]/.test(t[y])) y += d
    return Math.max(0, Math.min(t.length, y))
  }
  wordAt(x) {
    const t = this.text
    const isW = (c) => c && !/\s/.test(c) && !'{}⟪¦⟫⨝\''.includes(c)
    let a = x, b = x
    while (a > 0 && isW(t[a - 1])) a--
    while (b < t.length && isW(t[b])) b++
    return [a, b]
  }
  vertical(dir) {
    const c = this.caretRect(this.b)
    const inner = this.inner.getBoundingClientRect()
    if (this.goal === null) this.goal = c.cx
    const y = dir < 0 ? c.cy - c.height * 0.9 : c.cy + c.height * 0.9
    if (y < inner.top || y > inner.bottom) return dir < 0 ? 0 : this.text.length
    const s = this.scroll
    if (y < s.getBoundingClientRect().top || y > s.getBoundingClientRect().bottom) s.scrollTop += dir * c.height
    const g = this.goal
    const yy = dir < 0 ? c.cy - c.height * 0.9 : c.cy + c.height * 0.9
    const x = this.offsetAt(g, yy - (dir < 0 ? 0 : 0))
    return x
  }
  move(to, extend) {
    const goal = this.goal
    this.b = to
    if (!extend) this.a = to
    this.paint(true)
    this.goal = goal
  }

  /** The enclosing fork of x, from the marks. */
  forkAt(x) {
    const fs = forkMarks(this.text).filter((f) => f.open < x && (f.close < 0 || x <= f.close))
    return fs[fs.length - 1] || null
  }

  key(e) {
    const t = this.text
    const ctrl = e.ctrlKey || e.metaKey
    const ext = e.shiftKey
    const [x, y] = this.sel()
    const k = e.key
    const done = () => { e.preventDefault(); e.stopPropagation() }
    if (this.onKey && this.onKey(e)) return done()
    if (this.menu.hidden === false && k === 'Escape') { this.closeMenu(); return done() }
    if (ctrl && e.code === 'Backslash') { const s = this.selected(); this.edit(x, y, `⟪${s}¦⟫`, x + s.length + 2); return done() }
    if (ctrl && e.code === 'BracketRight') { this.replaceSel(ext ? '⨝' : '⟫'); return done() }
    if (e.altKey && k === 'Enter') {
      const f = this.forkAt(this.b)
      if (!f) return done()
      const end = [...f.seps, f.close < 0 ? this.lineEnd(this.b) : f.close].find((p) => p >= this.b)
      this.edit(end, end, '¦', end + 1)
      return done()
    }
    if (e.altKey && k === 'Backspace') {
      const f = this.forkAt(this.b)
      if (!f) return done()
      const bounds = [f.open, ...f.seps, f.close < 0 ? this.lineEnd(this.b) : f.close]
      let i = 0
      while (i < bounds.length - 1 && bounds[i + 1] < this.b) i++
      if (f.seps.length === 0) return done()
      // the lane and the separator before it (or after it, for the first lane)
      const from = i === 0 ? bounds[0] + 1 : bounds[i], to = i === 0 ? bounds[1] + 1 : bounds[i + 1]
      this.edit(from, to, '', from)
      return done()
    }
    if (ctrl && k.toLowerCase() === 'l' && !e.altKey) {
      const f = this.forkAt(this.b)
      if (!f) return done()
      if (/^[A-Za-z_][\w-]*:/.test(t.slice(f.open + 1, f.open + 40))) return done()
      this.edit(f.open + 1, f.open + 1, 'name:', f.open + 1)
      this.select(f.open + 1, f.open + 5)
      return done()
    }
    if (ctrl && k.toLowerCase() === 'm') {
      const a = this.lineStart(x), b = this.lineEnd(Math.max(x, y - (y > x && t[y - 1] === '\n' ? 1 : 0)))
      const lines = t.slice(a, b).split('\n').filter((l) => l.trim())
      if (lines.length < 2) return done()
      const m = mutualize(lines)
      this.edit(a, b, m, a + m.length)
      return done()
    }
    if (k === 'F6') { this.toggleMint(); return done() }
    if (ctrl && k.toLowerCase() === 'z') { ext ? this.redo() : this.undo(); return done() }
    if (ctrl && k.toLowerCase() === 'y') { this.redo(); return done() }
    if (ctrl && k.toLowerCase() === 'a') { this.select(0, t.length); return done() }
    switch (k) {
      case 'ArrowLeft': {
        let to = !ext && x !== y ? x : ctrl ? this.wordAt(this.step(this.b, -1))[0] : this.step(this.b, -1)
        this.move(to, ext); this.goal = null; return done()
      }
      case 'ArrowRight': {
        let to = !ext && x !== y ? y : ctrl ? this.wordAt(this.b)[1] || this.step(this.b, 1) : this.step(this.b, 1)
        if (ctrl && to === this.b) to = this.step(this.b, 1)
        this.move(to, ext); this.goal = null; return done()
      }
      case 'ArrowUp': this.move(this.vertical(-1), ext); return done()
      case 'ArrowDown': this.move(this.vertical(1), ext); return done()
      case 'Home': this.move(ctrl ? 0 : this.lineStart(this.b), ext); this.goal = null; return done()
      case 'End': this.move(ctrl ? t.length : this.lineEnd(this.b), ext); this.goal = null; return done()
      case 'PageUp': case 'PageDown': {
        const c = this.caretRect(this.b), h = this.scroll.clientHeight * 0.9 * (k === 'PageUp' ? -1 : 1)
        this.scroll.scrollTop += h
        this.move(this.offsetAt(c.cx, c.cy + h - (this.scroll.scrollTop - this.scroll.scrollTop)), ext)
        return done()
      }
      case 'Backspace': {
        if (x !== y) { this.edit(x, y, ''); return done() }
        if (x === 0) return done()
        const p = ctrl ? this.wordAt(this.step(x, -1))[0] : this.step(x, -1)
        // an empty zone goes as a whole
        if (t[p] === '{' && t[x] === '}') { this.edit(p, x + 1, '', p, 'del'); return done() }
        this.edit(p, x, '', p, 'del')
        return done()
      }
      case 'Delete': {
        if (x !== y) { this.edit(x, y, ''); return done() }
        if (x >= t.length) return done()
        const q = ctrl ? this.wordAt(x)[1] || this.step(x, 1) : this.step(x, 1)
        if (t[x] === '{' && t[x + 1] === '}') { this.edit(x, x + 2, '', x, 'del'); return done() }
        this.edit(x, q, '', x, 'del')
        return done()
      }
      case 'Enter': {
        if (ctrl) return
        const ls = this.lineStart(x)
        const ind = /^[ \t]*/.exec(t.slice(ls))[0]
        this.replaceSel('\n' + ind)
        return done()
      }
      case 'Tab': this.replaceSel('  ', 'type'); return done()
      case 'Escape': if (this.mint) this.toggleMint(); else if (x !== y) this.select(this.b, this.b); return
    }
  }
  type(s) {
    const [x, y] = this.sel()
    const t = this.text
    if (s === '{') {
      if (x !== y) { this.edit(x, y, '{' + t.slice(x, y) + '}', y + 2); return }
      this.edit(x, y, '{}', x + 1, 'type')
      return
    }
    if (s === '}' && x === y && t[x] === '}') { this.select(x + 1, x + 1); return }
    this.replaceSel(s, /\s/.test(s) ? 'space' : 'type')
  }

  // ---------- zones made with the middle button ----------

  /** The range grown to whole words and whole zones and forks. */
  snap(x, y) {
    let a = Math.min(x, y), b = Math.max(x, y)
    const t = this.text
    const isW = (c) => c && !/\s/.test(c) && !'{}⟪¦⟫⨝'.includes(c)
    while (a > 0 && isW(t[a - 1]) && isW(t[a])) a--
    while (b < t.length && isW(t[b - 1]) && isW(t[b])) b++
    for (let pass = 0; pass < 20; pass++) {
      let grew = false
      const st = []
      const pairs = []
      for (let i = 0; i < t.length; i++) {
        if (t[i] === '{') st.push(i)
        else if (t[i] === '}' && st.length) pairs.push([st.pop(), i])
      }
      for (const f of forkMarks(t)) pairs.push([f.open, f.close < 0 ? f.open : f.close])
      for (const [p, q] of pairs) {
        const inA = p >= a && p < b, inB = q >= a && q < b
        if (inA !== inB) { a = Math.min(a, p); b = Math.max(b, q + 1); grew = true }
      }
      if (!grew) break
    }
    while (a < b && /\s/.test(t[a])) a++
    while (b > a && /\s/.test(t[b - 1])) b--
    return [a, b]
  }
  wrap(x, y, k) {
    const [s, place] = k.wrap(this.text.slice(x, y))
    this.edit(x, y, s, x + s.length)
    if (place) {
      const i = s.lastIndexOf(place)
      this.select(x + i, x + i + place.length)
    }
  }
  pointer() {
    const d = this.drag
    this.over.querySelectorAll('.ptr').forEach((n) => n.remove())
    if (!d) return
    const inner = this.inner.getBoundingClientRect()
    const s = this.caretRect(d.range[0])
    const ex = d.cx - inner.left, ey = d.cy - inner.top
    const sx = s.left, sy = s.top + s.height / 2
    const len = Math.hypot(ex - sx, ey - sy), ang = Math.atan2(ey - sy, ex - sx)
    const k = KINDS[this.kindIx]
    const line = this.box('ptr line z-' + k.kind, { left: sx, top: sy - 1, width: len, height: 2 })
    line.style.transform = `rotate(${ang}rad)`
    line.style.transformOrigin = '0 50%'
    const lab = this.box('ptr tag z-' + k.kind, { left: ex + 10, top: ey + 8, width: 'auto', height: 'auto' })
    lab.style.width = lab.style.height = ''
    lab.textContent = `${k.label} · ${d.range[1] - d.range[0]} chars · wheel for another`
  }
  openMenu(cx, cy, items) {
    const m = this.menu
    m.replaceChildren()
    for (const it of items) {
      const b = document.createElement('button')
      b.className = 'z-' + (it.kind || 'plain')
      b.textContent = it.label
      b.onmousedown = (e) => e.preventDefault()
      b.onclick = () => { this.closeMenu(); it.run() }
      m.append(b)
    }
    m.hidden = false
    const h = this.host.getBoundingClientRect()
    m.style.left = Math.min(cx - h.left, h.width - 180) + 'px'
    m.style.top = Math.min(cy - h.top + 6, h.height - m.offsetHeight - 6) + 'px'
  }
  closeMenu() { this.menu.hidden = true }
  /** The innermost zone around x, as offsets of its braces and its word. */
  zoneAt(x) {
    let best = null
    const walk = (kids) => {
      for (const k of kids) {
        if (k.t === 'zone') { if (k.a <= x && (k.b < 0 || x <= k.b)) { best = k; walk(k.kids) } }
        else if (k.t === 'fork') { k.lanes.forEach(walk); walk(k.shared) }
      }
    }
    H = this.hooks
    const root = lex(this.text)
    H = PAGE
    walk(root.kids)
    return best
  }

  toggleMint() {
    this.mint = !this.mint
    this.mintEl.hidden = !this.mint
    if (this.mint) this.showMint()
  }
  /** The minted lines of the caret's line, each one editable (BEADS §4.4). An edit goes to
   *  the beads it touched, so every line through them changes; with "this line only" the
   *  touched words become a new fork and the other lines keep them. */
  showMint(focusRow = -1, caret = 0) {
    const a = this.lineStart(this.b), b = this.lineEnd(this.b)
    const line = this.text.slice(a, b)
    let lines
    try { lines = expandMap(line).map((l) => l.s) } catch (e) { lines = null; this.mintEl.replaceChildren(Object.assign(document.createElement('div'), { className: 'head', textContent: '× ' + e.message })); return }
    if (!this.mintHead) {
      const h = document.createElement('div')
      h.className = 'head'
      h.innerHTML = '<span class="n"></span><label><input type="checkbox" class="only"> this line only</label><span class="note"></span>'
      this.mintHead = h
      this.mintOnly = h.querySelector('.only')
    }
    this.mintHead.querySelector('.n').textContent = `the lines this line mints · ${lines.length} · F6 or Escape closes · `
    this.mintHead.querySelector('.note').textContent = ''
    this.mintEl.replaceChildren(this.mintHead)
    lines.forEach((l, i) => {
      const r = document.createElement('input')
      r.className = 'row'
      r.spellcheck = false
      r.value = l
      r.dataset.was = l
      r.addEventListener('input', () => this.mintInput(r, i, a))
      r.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' || e.key === 'F6') { e.preventDefault(); this.toggleMint(); this.focus() }
        else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault()
          const rows = [...this.mintEl.querySelectorAll('.row')], o = rows[i + (e.key === 'ArrowDown' ? 1 : -1)]
          if (o) { const c = r.selectionStart; o.focus(); o.setSelectionRange(c, c) }
        }
      })
      this.mintEl.append(r)
    })
    if (focusRow >= 0) {
      const r = this.mintEl.querySelectorAll('.row')[Math.min(focusRow, lines.length - 1)]
      if (r) { r.focus(); r.setSelectionRange(caret, caret) }
    }
  }
  mintInput(r, i, lineAt) {
    const was = r.dataset.was, now = r.value
    let p = 0
    while (p < was.length && p < now.length && was[p] === now[p]) p++
    let e = 0
    while (e < was.length - p && e < now.length - p && was[was.length - 1 - e] === now[now.length - 1 - e]) e++
    const q = was.length - e, ins = now.slice(p, now.length - e)
    const line = this.text.slice(lineAt, this.lineEnd(lineAt))
    const only = this.mintOnly.checked
    let c = null
    try { c = mintEdit(line, i, p, q, ins, !only) } catch { c = null }
    if (!c && !only) try { c = mintEdit(line, i, p, q, ins, false) } catch { c = null }
    if (!c) { r.value = was; this.mintHead.querySelector('.note').textContent = ' · that edit has no place in the beads'; return }
    const caret = r.selectionStart
    this.edit(lineAt + c.at, lineAt + c.to, c.ins, lineAt + c.at + c.ins.length, 'mint')
    this.showMint(i, caret)
  }
  stats() {
    const a = this.lineStart(this.b), b = this.lineEnd(this.b)
    try { return lineStats(this.text.slice(a, b)) } catch { return null }
  }

  // ---------- events ----------

  bind() {
    const inp = this.input
    inp.addEventListener('keydown', (e) => this.key(e))
    inp.addEventListener('input', (e) => {
      if (e.isComposing) return
      const v = inp.value
      inp.value = ''
      if (v) this.type(v)
    })
    inp.addEventListener('compositionend', () => { const v = inp.value; inp.value = ''; if (v) this.type(v) })
    inp.addEventListener('copy', (e) => { e.preventDefault(); e.clipboardData.setData('text/plain', this.selected()) })
    inp.addEventListener('cut', (e) => { e.preventDefault(); e.clipboardData.setData('text/plain', this.selected()); this.replaceSel('') })
    inp.addEventListener('paste', (e) => {
      e.preventDefault()
      // the middle button pastes on some systems; here it makes zones
      if (Date.now() - (this.middleAt || 0) < 150) return
      this.replaceSel(e.clipboardData.getData('text/plain').replace(/\r\n?/g, '\n'))
    })
    inp.addEventListener('focus', () => this.host.classList.add('focused'))
    inp.addEventListener('blur', () => this.host.classList.remove('focused'))

    const sc = this.scroll
    sc.addEventListener('mousedown', (e) => { if (e.button === 1) e.preventDefault() })
    sc.addEventListener('auxclick', (e) => { if (e.button === 1) e.preventDefault() })
    sc.addEventListener('pointerdown', (e) => {
      if (e.target.closest('.joinres')) return
      this.closeMenu()
      if (e.button === 1) {
        e.preventDefault()
        const x = this.offsetAt(e.clientX, e.clientY)
        this.drag = { from: x, range: [x, x], cx: e.clientX, cy: e.clientY, moved: false }
        sc.setPointerCapture(e.pointerId)
        this.paint(); this.pointer()
        return
      }
      if (e.button !== 0) return
      e.preventDefault()
      this.focus()
      const x = this.offsetAt(e.clientX, e.clientY)
      if ((e.ctrlKey || e.metaKey) && this.onGo) { this.select(x, x); this.onGo(x); return }
      if (e.detail === 2) { const [a, b] = this.wordAt(x); this.select(a, b); return }
      if (e.detail >= 3) { this.select(this.lineStart(x), Math.min(this.text.length, this.lineEnd(x) + 1)); return }
      if (e.shiftKey) this.select(this.a, x)
      else this.select(x, x)
      this.selecting = true
      sc.setPointerCapture(e.pointerId)
    })
    sc.addEventListener('pointermove', (e) => {
      if (this.drag) {
        const d = this.drag
        const x = this.offsetAt(e.clientX, e.clientY)
        d.cx = e.clientX; d.cy = e.clientY
        if (Math.abs(x - d.from) > 0) d.moved = true
        d.range = this.snap(d.from, x)
        this.paint(); this.pointer()
        return
      }
      if (this.selecting) { this.b = this.offsetAt(e.clientX, e.clientY); this.paint(); return }
      if (this.onHover && e.buttons === 0) {
        clearTimeout(this.hoverT)
        const cx = e.clientX, cy = e.clientY
        this.hoverT = setTimeout(() => {
          const hit = document.elementFromPoint(cx, cy)
          this.onHover(hit && this.doc.contains(hit) && hit !== this.doc ? this.offsetAt(cx, cy) : null, cx, cy)
        }, 350)
      }
    })
    sc.addEventListener('pointerleave', () => { clearTimeout(this.hoverT); if (this.onHover) this.onHover(null) })
    sc.addEventListener('wheel', (e) => {
      if (!this.drag) return
      e.preventDefault()
      this.kindIx = (this.kindIx + (e.deltaY > 0 ? 1 : -1) + KINDS.length) % KINDS.length
      this.paint(); this.pointer()
    }, { passive: false })
    sc.addEventListener('pointerup', (e) => {
      if (e.button === 1) this.middleAt = Date.now()
      if (this.drag) {
        const d = this.drag
        this.drag = null
        this.paint(); this.pointer()
        if (d.moved && d.range[1] > d.range[0]) { this.wrap(d.range[0], d.range[1], KINDS[this.kindIx]); this.focus(); return }
        // a click: the zone under the pointer can change what it is, or go
        const z = this.zoneAt(d.from)
        if (!z || z.b < 0) return
        const items = [{ label: 'unwrap', kind: 'plain', run: () => this.unwrap(z) }]
        for (const k of KINDS.filter((k) => k.kind !== 'lanes' && k.kind !== 'plain')) items.push({ label: 'make it ' + k.label, kind: k.kind, run: () => this.retype(z, k) })
        this.openMenu(e.clientX, e.clientY, items)
        return
      }
      this.selecting = false
    })
    addEventListener('keydown', (e) => { if (this.drag && e.key === 'Escape') { this.drag = null; this.paint(); this.pointer() } })
    new ResizeObserver(() => this.paint()).observe(this.host)
  }
  unwrap(z) {
    const t = this.text
    let end = z.b + 1
    // the word that took the brace goes too
    if (z.by && t.slice(z.b + 1, z.by.a).trim() === '') end = z.by.b
    const inner = t.slice(z.a + 1, z.b)
    this.edit(z.a, end, inner, z.a + inner.length)
  }
  retype(z, k) {
    const inner = this.text.slice(z.a + 1, z.b)
    let end = z.b + 1
    if (z.by && this.text.slice(z.b + 1, z.by.a).trim() === '') end = z.by.b
    const [s, place] = k.wrap(inner)
    this.edit(z.a, end, s, z.a + s.length)
    if (place) { const i = s.lastIndexOf(place); this.select(z.a + i, z.a + i + place.length) }
  }
}
