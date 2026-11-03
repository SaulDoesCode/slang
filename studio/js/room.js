// The room: one s file as pages of curve text in space, flown through first person.
// The file is the only truth; where the pages stand is kept beside it, written as s.
import * as THREE from '../vendor/three.module.js'
import * as T from './text.js'

const $ = (s) => document.querySelector(s)
async function api(cmd, o = {}) {
  try {
    const r = await fetch('api', { method: 'POST', body: JSON.stringify({ cmd, ...o }) })
    return await r.json()
  } catch (e) {
    return { error: 'the server does not answer: ' + e.message }
  }
}
const store = {
  get(k, d) { try { const v = localStorage.getItem('room.' + k); return v == null ? d : JSON.parse(v) } catch { return d } },
  set(k, v) { try { localStorage.setItem('room.' + k, JSON.stringify(v)) } catch {} },
}

// ---------- sizes and colors ----------

const EM = 0.3 // world height of one em
const LH = T.LINE // line height in em
let ADV = 0.6 // advance of one character in em, from the font
const SUFFIX = '?!~$`#;:.@'
const INK = '#3b2d20', INK2 = '#8a7660', ACCENT = '#7a5532', ERR = '#b0413e', OUT = '#2f6f62'
const KIND = {
  num: '#2f6f62', cmd: '#7a3e20', word: INK, look: '#2d5c8a', call: '#2d5c8a', def: '#8a2d6a', set: '#8a5a00', numset: '#8a5a00',
  pop: '#5a4a8a', esc: '#7d7060', quote: '#8a6a10', lib: '#4b6a2a', geo: INK2, angle: '#6a3e8a',
}
const DEPTH = ['#9a6f45', '#2f6f62', '#8a2d6a', '#2d5c8a', '#8a5a00', '#5a4a8a']
const colorOf = (k) => new THREE.Color(k)

// ---------- the scene ----------

const canvas = $('#view')
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
renderer.setPixelRatio(Math.min(devicePixelRatio || 1, 2))
const scene = new THREE.Scene()
const BG = 0xe9dbc6
scene.background = new THREE.Color(BG)
scene.fog = new THREE.Fog(BG, 70, 300)
const camera = new THREE.PerspectiveCamera(60, 1, 0.05, 2000)
T.shared.uTanHalf.value = Math.tan(THREE.MathUtils.degToRad(30))
const textMat = T.colorMaterial()
const lineMats = new Map()
const lineMat = (c) => { if (!lineMats.has(c)) lineMats.set(c, new THREE.LineBasicMaterial({ color: c })); return lineMats.get(c) }
const selMat = new THREE.MeshBasicMaterial({ color: 0xcfb08a, transparent: true, opacity: 0.45, depthWrite: false })
const caretMat = new THREE.MeshBasicMaterial({ color: 0x7a5532 })
const quad = new THREE.PlaneGeometry(1, 1)

// ---------- state ----------

const R = {
  path: null,
  text: '',
  pages: [], // { text, sep, key, pos: Vector3, placed, group, lines, toks, w, h, tags, err, start }
  dirty: false,
  savedText: '',
  undo: [],
  redo: [],
  edit: null, // the page open in the box
  hover: null, // { page, off, tok }
  carry: null,
  live: store.get('live', false),
  run: null,
  defs: new Map(), // name -> [{ page, tok }]
  docs: new Map(),
  caps: new Map(),
  kinds: new Map(), // word -> class, from the server
  pendingWords: new Set(),
  found: [],
  foundAt: -1,
  back: [],
}
const cam = { pos: new THREE.Vector3(0, 2, 16), q: new THREE.Quaternion(), speed: store.get('speed', 8), keys: new Set(), vel: new THREE.Vector3(), glide: new THREE.Vector3(), anim: null }
let locked = false

function status(t, err = false) {
  const s = $('#stat')
  s.textContent = t
  s.className = err ? 'err' : ''
}
function showFile() {
  $('#file').innerHTML = ''
  const b = document.createElement('b')
  b.textContent = R.path || '(no file)'
  $('#file').append(b, document.createTextNode((R.dirty ? ' ●' : '') + (R.live ? ' · live' : '')))
}

// ---------- the file as pages ----------

function splitPages(text) {
  const parts = text.split(/(\n(?:[ \t]*\n)+)/)
  const pages = []
  for (let i = 0; i < parts.length; i += 2) {
    let t = parts[i], sep = parts[i + 1] || ''
    const m = t.match(/\s*$/)[0]
    t = t.slice(0, t.length - m.length)
    sep = m + sep
    if (!t && pages.length) { pages[pages.length - 1].sep += sep; continue }
    pages.push({ text: t, sep })
  }
  if (!pages.length) pages.push({ text: '', sep: '\n' })
  return pages
}
function joinPages() {
  const ps = R.pages
  let s = ''
  ps.forEach((p, i) => {
    let sep = p.sep
    // pages stay paragraphs: at least one blank line between two of them
    if (i < ps.length - 1 && !/\n[ \t]*\n/.test(sep)) sep = '\n\n'
    if (i === ps.length - 1 && !sep.endsWith('\n') && p.text) sep = '\n'
    p.start = s.length
    s += p.text + sep
  })
  return s
}
function keyOf(text) {
  const first = (text.split('\n').find((l) => l.trim()) || '').trim().slice(0, 60)
  return first.replace(/[{}]/g, (c) => (c === '{' ? '(' : ')'))
}
function assignKeys() {
  const seen = new Map()
  for (const p of R.pages) {
    const k = keyOf(p.text)
    const n = seen.get(k) || 0
    seen.set(k, n + 1)
    p.key = k + '#' + n
  }
}

// ---------- tokens ----------

function tokenize(text) {
  const toks = []
  const n = text.length
  let i = 0, depth = 0
  while (i < n) {
    const c = text[i]
    if (c === ' ' || c === '\n' || c === '\t' || c === '\r') { i++; continue }
    const from = i
    if (c === '{') { toks.push({ from, to: i + 1, w: '{', kind: 'open', depth }); depth++; i++; continue }
    if (c === '}') {
      depth = Math.max(0, depth - 1)
      let j = i + 1
      while (j < n && !/\s/.test(text[j]) && text[j] !== '{' && text[j] !== '}') j++
      toks.push({ from, to: j, w: text.slice(from, j), kind: 'close', depth })
      i = j
      continue
    }
    if (c === "'") {
      let j = i + 1
      while (j < n && text[j] !== "'" && text[j] !== '\n') j++
      j = Math.min(n, j + 1)
      while (j < n && !/\s/.test(text[j]) && text[j] !== '{' && text[j] !== '}') j++
      toks.push({ from, to: j, w: text.slice(from, j), kind: 'quote', depth })
      i = j
      continue
    }
    if (c === '⟨') {
      let j = i + 1
      while (j < n && text[j] !== '⟩') j++
      j = Math.min(n, j + 1)
      while (j < n && !/\s/.test(text[j])) j++
      toks.push({ from, to: j, w: text.slice(from, j), kind: 'angle', depth })
      i = j
      continue
    }
    let j = i
    while (j < n && !/\s/.test(text[j]) && text[j] !== '{' && text[j] !== '}') j++
    const w = text.slice(from, j)
    toks.push({ from, to: j, w, kind: wordKind(w), depth })
    i = j
  }
  return toks
}
function wordKind(w) {
  if (/^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(w)) return 'num'
  const k = R.kinds.get(w)
  if (k) return k
  R.pendingWords.add(w)
  return 'word'
}
// the server classes words exactly as the machine reads them; asked in batches
let kindTimer = 0
function askKinds() {
  if (!R.pendingWords.size || kindTimer) return
  kindTimer = setTimeout(async () => {
    kindTimer = 0
    const ws = [...R.pendingWords].filter((w) => !R.kinds.has(w))
    R.pendingWords.clear()
    if (!ws.length) return
    const r = await api('lex', { text: ws.join(' ') })
    for (const t of r.tokens || []) R.kinds.set(t.w, t.kind === 'brace' ? 'word' : t.kind)
    for (const w of ws) if (!R.kinds.has(w)) R.kinds.set(w, 'word')
    let any = false
    for (const p of R.pages) if (p.toks && p.toks.some((t) => ws.includes(t.w))) { layoutText(p); buildPage(p); any = true }
    if (any) indexDefs()
  }, 30)
}

// a word without its sigils: the name it reads or defines
function bare(w) {
  let s = w
  if (s.startsWith("'") && s.endsWith("'") && s.length > 1) return s.slice(1, -1)
  if (s.startsWith('\\') && s.length > 1) s = s.slice(1)
  while (s.length > 1 && SUFFIX.includes(s[s.length - 1])) s = s.slice(0, -1)
  return s
}

// definitions: {…} key ><, {…} name$, and words the machine reads as definitions
function indexDefs() {
  R.defs = new Map()
  const add = (name, page, tok) => {
    if (!name) return
    if (!R.defs.has(name)) R.defs.set(name, [])
    R.defs.get(name).push({ page, tok })
  }
  for (const p of R.pages) {
    const ts = p.toks || []
    ts.forEach((t, i) => {
      if (t.depth !== 0) return
      if (t.w === '><' && i >= 2 && ts[i - 2].kind === 'close' && ts[i - 2].depth === 0) add(bare(ts[i - 1].w), p, ts[i - 1])
      if (t.kind !== 'close' && t.w.endsWith('$') && !t.w.endsWith('\\$') && t.w.length > 1 && i >= 1 && ts[i - 1].kind === 'close') add(bare(t.w), p, t)
      if (t.kind === 'def') add(bare(t.w), p, t)
    })
  }
}

// ---------- building a page ----------

function layoutText(p) {
  p.lines = p.text.split('\n')
  p.lineStart = []
  let o = 0
  for (const l of p.lines) { p.lineStart.push(o); o += l.length + 1 }
  const wMax = Math.max(6, ...p.lines.map((l) => l.trimEnd().length))
  p.wEm = wMax * ADV
  p.hEm = Math.max(1, p.lines.length) * LH
  p.toks = tokenize(p.text)
  // the depth of braces at each character; braces inside quotes do not count
  p.depthArr = new Uint8Array(p.text.length + 1)
  const quoted = new Uint8Array(p.text.length + 1)
  for (const t of p.toks) if (t.kind === 'quote' || t.kind === 'angle') quoted.fill(1, t.from, t.to)
  let d = 0
  for (let i = 0; i < p.text.length; i++) {
    const c = p.text[i]
    if (!quoted[i] && c === '{') { p.depthArr[i] = d; d = Math.min(250, d + 1) }
    else if (!quoted[i] && c === '}') { d = Math.max(0, d - 1); p.depthArr[i] = d }
    else p.depthArr[i] = d
  }
  askKinds()
}
function lineOf(p, off) {
  let i = 0
  while (i + 1 < p.lineStart.length && p.lineStart[i + 1] <= off) i++
  return i
}
// where a character of the page sits, in em, page-local: x of its left edge, y of the baseline
function charAt(p, off) {
  const li = lineOf(p, off)
  const line = p.lines[li]
  const len = line.trimEnd().length
  const col = off - p.lineStart[li]
  const x0 = p.wEm / 2 - len * ADV
  return { li, x: x0 + col * ADV, y: p.hEm / 2 - (li + 0.5) * LH - T.CENTER, x0 }
}
function offAt(p, xEm, yEm) {
  const li = Math.max(0, Math.min(p.lines.length - 1, Math.floor((p.hEm / 2 - yEm) / LH)))
  const line = p.lines[li]
  const len = line.trimEnd().length
  const x0 = p.wEm / 2 - len * ADV
  const col = Math.max(0, Math.min(line.length, Math.round((xEm - x0) / ADV)))
  return p.lineStart[li] + col
}
function tokAt(p, off) {
  return (p.toks || []).find((t) => off >= t.from && off < t.to) || null
}

function disposeGroup(g) {
  for (const o of [...g.children]) {
    g.remove(o)
    if (o.geometry && o.geometry !== quad) o.geometry.dispose()
  }
}

function buildPage(p) {
  if (!p.group) {
    p.group = new THREE.Group()
    scene.add(p.group)
  }
  const g = p.group
  disposeGroup(g)
  const s = EM
  const runs = []
  const ul = new Map() // color -> points
  const seg = (c, x0, x1, y) => { if (!ul.has(c)) ul.set(c, []); ul.get(c).push(x0 * s, y * s, 0, x1 * s, y * s, 0) }
  const errTok = p.err && p.err.tok
  for (const t of p.toks) {
    const a = charAt(p, t.from)
    let col = t.kind === 'open' || t.kind === 'close' ? DEPTH[t.depth % DEPTH.length] : KIND[t.kind] || INK
    if (t === errTok) col = ERR
    if (R.found.length && R.found.some((f) => f.page === p && f.from === t.from)) col = '#c0392b'
    runs.push({ text: t.w, x: a.x * s, y: a.y * s, size: s, color: colorOf(col) })
  }
  // nesting: one underline per level of braces a character stands in
  p.lines.forEach((line, li) => {
    const base = p.lineStart[li]
    const len = line.trimEnd().length
    const x0 = p.wEm / 2 - len * ADV
    const y = p.hEm / 2 - (li + 0.5) * LH - T.CENTER - 0.22
    const depthAt = []
    for (let c = 0; c < len; c++) depthAt.push(depthOf(p, base + c))
    const maxD = Math.max(0, ...depthAt)
    for (let d = 1; d <= maxD; d++) {
      let c = 0
      while (c < len) {
        if (depthAt[c] >= d) {
          let e = c
          while (e < len && depthAt[e] >= d) e++
          seg(DEPTH[d % DEPTH.length], x0 + c * ADV, x0 + e * ADV, y - (d - 1) * 0.11)
          c = e
        } else c++
      }
    }
  })
  // what the last run printed, beside the line of the word that printed it
  const tagRuns = []
  if (p.tags) {
    for (const [li, t] of p.tags) {
      const y = p.hEm / 2 - (li + 0.5) * LH - T.CENTER
      tagRuns.push({ text: t, x: (p.wEm / 2 + 1.2) * s, y: y * s, size: s * 0.82, color: colorOf(/^fail\b/.test(t) ? ERR : OUT) })
    }
  }
  if (p.err) {
    const li = p.err.li
    const y = p.hEm / 2 - (li + 0.5) * LH - T.CENTER
    tagRuns.push({ text: '× ' + p.err.msg.slice(0, 90), x: (p.wEm / 2 + 1.2) * s, y: (y - (p.tags && p.tags.has(li) ? LH * 0.8 : 0)) * s, size: s * 0.82, color: colorOf(ERR) })
  }
  // its place in the file, small above its right edge
  const n = R.pages.indexOf(p) + 1
  tagRuns.push({ text: '§' + n, x: (p.wEm / 2 - 2.2) * s, y: (p.hEm / 2 + 0.55) * s, size: s * 0.6, color: colorOf(INK2) })
  const text = new THREE.Mesh(T.colorGeometry(runs.concat(tagRuns)), textMat)
  text.frustumCulled = false
  g.add(text)
  for (const [c, pts] of ul) {
    const lg = new THREE.BufferGeometry()
    lg.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3))
    g.add(new THREE.LineSegments(lg, lineMat(c)))
  }
  if (R.edit === p) buildCaret(p)
  p.hoverLine = null
}
function depthOf(p, off) {
  return p.depthArr[off] || 0
}

// the caret and the selection of the box, drawn on the page
let caretGroup = null
function buildCaret(p) {
  if (caretGroup) { caretGroup.parent && caretGroup.parent.remove(caretGroup); disposeGroup(caretGroup) }
  caretGroup = new THREE.Group()
  const box = $('#box')
  const a = box.selectionStart, b = box.selectionEnd
  const s = EM
  if (a !== b) {
    for (let li = lineOf(p, a); li <= lineOf(p, b); li++) {
      const from = Math.max(a, p.lineStart[li]), to = Math.min(b, p.lineStart[li] + p.lines[li].length)
      const ca = charAt(p, from), cb = charAt(p, to)
      const m = new THREE.Mesh(quad, selMat)
      const w = Math.max(0.3, cb.x - ca.x)
      m.scale.set(w * s, LH * s, 1)
      m.position.set((ca.x + w / 2) * s, (p.hEm / 2 - (li + 0.5) * LH) * s, -0.01)
      caretGroup.add(m)
    }
  }
  const c = charAt(p, b)
  const m = new THREE.Mesh(quad, caretMat)
  m.scale.set(0.09 * s, LH * 0.9 * s, 1)
  m.position.set(c.x * s, (p.hEm / 2 - (c.li + 0.5) * LH) * s, 0.002)
  caretGroup.add(m)
  p.group.add(caretGroup)
}

function rebuildAll() {
  for (const p of R.pages) { layoutText(p); buildPage(p) }
  indexDefs()
}

// ---------- layout ----------

// the pages spread out on a wall, in file order, definitions standing a step back
function spread(pages) {
  const rowMax = 34
  let x = 0, y = 0, rowH = 0
  const out = new Map()
  for (const p of pages) {
    const w = p.wEm * EM + 2.2, h = p.hEm * EM + 1.6
    if (x > 0 && x + w > rowMax) { x = 0; y -= rowH; rowH = 0 }
    const isDef = (p.toks || []).some((t) => t.depth === 0 && (t.w === '><' || (t.w.endsWith('$') && t.w.length > 1)))
    out.set(p, new THREE.Vector3(x + w / 2 - rowMax / 2, y - h / 2 + 4, isDef ? -3.5 : 0))
    x += w
    rowH = Math.max(rowH, h)
  }
  return out
}
function placeAll(saved) {
  const loose = R.pages.filter((p) => !(saved && saved.has(p.key)))
  const auto = spread(R.pages)
  for (const p of R.pages) {
    const s = saved && saved.get(p.key)
    p.pos = s ? s.clone() : auto.get(p)
    p.placed = !!s
    p.group.position.copy(p.pos)
  }
  return loose
}
function layoutText2s() {
  const n = (x) => String(Math.round(x * 100) / 100)
  return R.pages.filter((p) => p.placed).map((p) => n(p.pos.x) + ' ' + n(p.pos.y) + ' ' + n(p.pos.z) + ' {' + p.key + '} page.').join('\n') + '\n'
}
function parseLayout(t) {
  const m = new Map()
  for (const l of (t || '').split('\n')) {
    const r = l.match(/^\s*(-?[\d.]+)\s+(-?[\d.]+)\s+(-?[\d.]+)\s+\{(.*)\}\s+page\.\s*$/)
    if (r) m.set(r[4], new THREE.Vector3(+r[1], +r[2], +r[3]))
  }
  return m
}
let layoutTimer = 0
function saveLayout() {
  clearTimeout(layoutTimer)
  layoutTimer = setTimeout(() => { if (R.path) api('roomput', { path: R.path, text: layoutText2s() }) }, 400)
}

// ---------- files ----------

async function openFile(path) {
  if (R.dirty) await save()
  const r = await api('read', { path, create: true })
  if (r.error) { status(r.error, true); return }
  for (const p of R.pages) if (p.group) { disposeGroup(p.group); scene.remove(p.group) }
  endEdit(false)
  R.path = path
  store.set('file', path)
  const recent = store.get('recent', []).filter((x) => x !== path)
  recent.unshift(path)
  store.set('recent', recent.slice(0, 12))
  R.text = r.text
  R.savedText = r.text
  R.dirty = false
  R.undo = []
  R.redo = []
  R.run = null
  R.found = []
  R.pages = splitPages(r.text)
  assignKeys()
  for (const p of R.pages) layoutText(p)
  const lay = await api('roomget', { path })
  for (const p of R.pages) buildPage(p)
  placeAll(parseLayout(lay.text))
  indexDefs()
  joinPages()
  showFile()
  home()
  document.title = path + ' · s room'
  status(r.new ? 'a new file: it is written when it holds something' : R.pages.length + ' pages')
  if (R.live) runFile()
}

function home() {
  if (!R.pages.length) return
  const first = R.pages[0]
  const box = new THREE.Box3()
  for (const p of R.pages.slice(0, 12)) box.expandByPoint(p.pos)
  const c = new THREE.Vector3()
  box.getCenter(c)
  const size = Math.max(8, box.max.x - box.min.x + 6, (box.max.y - box.min.y + 4) * 1.4)
  cam.pos.set(c.x, c.y, Math.max(box.max.z, 0) + size * 0.62 + 2)
  cam.q.identity()
  cam.anim = null
}

// the text changed: the file follows, undo keeps the old one
function changed(push = true) {
  const old = R.text
  R.text = joinPages()
  if (R.text === old) return
  if (push) { R.undo.push(old); if (R.undo.length > 200) R.undo.shift(); R.redo = [] }
  R.dirty = R.text !== R.savedText
  showFile()
  scheduleSave()
  if (R.live) scheduleRun()
}
let saveTimer = 0
function scheduleSave() {
  clearTimeout(saveTimer)
  saveTimer = setTimeout(save, 1200)
}
async function save() {
  clearTimeout(saveTimer)
  if (!R.path || !R.dirty) return
  if (!R.text.trim() && !R.savedText) return
  const text = R.text
  const r = await api('write', { path: R.path, text })
  if (r.error) { status(r.error, true); return }
  R.savedText = text
  R.dirty = R.text !== R.savedText
  showFile()
  saveLayout()
}

// whole-file undo: the pages are rebuilt, each keeping its place by its key
function restore(text) {
  const where = new Map(R.pages.map((p) => [p.key, p.placed ? p.pos.clone() : null]))
  for (const p of R.pages) if (p.group) { disposeGroup(p.group); scene.remove(p.group) }
  R.pages = splitPages(text)
  assignKeys()
  for (const p of R.pages) { layoutText(p); buildPage(p) }
  const saved = new Map([...where].filter((e) => e[1]))
  placeAll(saved)
  indexDefs()
  R.text = joinPages()
  R.dirty = R.text !== R.savedText
  showFile()
  scheduleSave()
}
function undo() { if (!R.undo.length) return; R.redo.push(R.text); restore(R.undo.pop()); status('undone') }
function redo() { if (!R.redo.length) return; R.undo.push(R.text); restore(R.redo.pop()); status('redone') }

// ---------- editing in the box ----------

const box = $('#box')
function startEdit(p, off = null) {
  if (R.edit && R.edit !== p) endEdit(true)
  R.edit = p
  R.editWas = p.text
  box.classList.add('editing')
  box.value = p.text
  box.placeholder = ''
  fitBox()
  box.focus({ preventScroll: true })
  const o = off == null ? p.text.length : off
  box.setSelectionRange(o, o)
  buildPage(p)
  status('editing §' + (R.pages.indexOf(p) + 1) + ' · Esc ends · Tab completes')
}
function endEdit(commit = true) {
  const p = R.edit
  if (!p) return
  R.edit = null
  box.classList.remove('editing')
  box.value = ''
  box.placeholder = "s here runs with the file's definitions · /text searches · Enter on a page edits it"
  fitBox()
  hideComp()
  if (caretGroup && caretGroup.parent) caretGroup.parent.remove(caretGroup)
  if (!commit) return
  const i = R.pages.indexOf(p)
  if (i < 0) return
  // an emptied page goes; a blank line splits it
  if (!p.text.trim()) {
    disposeGroup(p.group)
    scene.remove(p.group)
    R.pages.splice(i, 1)
    if (!R.pages.length) { R.pages.push({ text: '', sep: '\n' }); layoutText(R.pages[0]); buildPage(R.pages[0]); R.pages[0].pos = new THREE.Vector3(); R.pages[0].group.position.set(0, 0, 0) }
  } else if (/\n[ \t]*\n/.test(p.text)) {
    const parts = splitPages(p.text)
    parts[parts.length - 1].sep = p.sep
    const news = parts.map((q, k) => {
      const np = { text: q.text, sep: q.sep }
      layoutText(np)
      buildPage(np)
      np.pos = p.pos.clone().add(new THREE.Vector3(0, -k * (np.hEm * EM + 1.4), 0))
      np.placed = p.placed
      np.group.position.copy(np.pos)
      return np
    })
    disposeGroup(p.group)
    scene.remove(p.group)
    R.pages.splice(i, 1, ...news)
  }
  const oldKey = p.key
  assignKeys()
  if (p.key !== oldKey && p.placed) saveLayout()
  R.pages.forEach((q) => buildPage(q))
  indexDefs()
  changed()
  canvas.focus()
}
function fitBox() {
  box.style.height = 'auto'
  box.style.height = Math.min(box.scrollHeight + 2, innerHeight * 0.42) + 'px'
}
box.addEventListener('input', () => {
  fitBox()
  if (!R.edit) { showCompFor(); return }
  const p = R.edit
  p.text = box.value
  layoutText(p)
  buildPage(p)
  p.tags = null
  p.err = null
  R.text = joinPages()
  R.dirty = R.text !== R.savedText
  showFile()
  scheduleSave()
  if (R.live) scheduleRun()
  showCompFor()
})
for (const ev of ['keyup', 'mouseup', 'select']) box.addEventListener(ev, () => { if (R.edit) buildCaret(R.edit) })
document.addEventListener('selectionchange', () => { if (R.edit && document.activeElement === box) buildCaret(R.edit) })

box.addEventListener('keydown', (e) => {
  if (e.key === 'F5' || (e.key === 'Enter' && (e.ctrlKey || e.metaKey))) { e.preventDefault(); runFile(); return }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') { e.preventDefault(); if (R.edit) { R.text = joinPages(); R.dirty = true } save(); return }
  if (comp.items.length && !comp.hidden) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); comp.i = (comp.i + (e.key === 'ArrowDown' ? 1 : -1) + comp.items.length) % comp.items.length; renderComp(); return }
    if (e.key === 'Tab') { e.preventDefault(); takeComp(); return }
    if (e.key === 'Escape') { e.preventDefault(); hideComp(); return }
  }
  if (e.key === 'Tab') { e.preventDefault(); showCompFor(true); if (comp.items.length === 1) takeComp(); return }
  if (e.key === 'Escape') {
    e.preventDefault()
    if (R.edit) endEdit(true)
    else { box.value = ''; fitBox(); canvas.focus() }
    return
  }
  if (!R.edit && e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    command(box.value)
    return
  }
  if (!R.edit && e.key === 'F3') { e.preventDefault(); nextFound() }
})

// ---------- completion ----------

const comp = { items: [], i: 0, from: 0, hidden: true }
const compEl = $('#comp')
function wordBefore() {
  const v = box.value, e = box.selectionStart
  let s = e
  while (s > 0 && !/[\s{}]/.test(v[s - 1])) s--
  return { s, e, w: v.slice(s, e) }
}
function showCompFor(force = false) {
  const { s, w } = wordBefore()
  if (!force && w.length < 2) { hideComp(); return }
  if (w.startsWith('/') && !R.edit) { hideComp(); return }
  const seen = new Set()
  const items = []
  const add = (word, note) => { if (word.startsWith(w) && word !== w && !seen.has(word)) { seen.add(word); items.push({ word, note }) } }
  for (const name of R.defs.keys()) add(name, 'defined here')
  for (const [word, d] of R.docs) add(word, d.sig || d.doc || '')
  for (const p of R.pages) for (const t of p.toks || []) if (t.kind !== 'num' && t.w.length > 2 && t.kind !== 'open' && t.kind !== 'close') add(t.w, '')
  comp.items = items.slice(0, 40)
  comp.i = 0
  comp.from = s
  if (!comp.items.length) { hideComp(); return }
  comp.hidden = false
  renderComp()
}
function renderComp() {
  compEl.hidden = false
  compEl.innerHTML = ''
  comp.items.forEach((it, k) => {
    const d = document.createElement('div')
    d.className = k === comp.i ? 'on' : ''
    d.append(document.createTextNode(it.word))
    const sp = document.createElement('span')
    sp.textContent = it.note.slice(0, 60)
    d.append(sp)
    d.onmousedown = (e) => { e.preventDefault(); comp.i = k; takeComp() }
    compEl.append(d)
  })
}
function takeComp() {
  const it = comp.items[comp.i]
  if (!it) return
  const { s, e } = wordBefore()
  box.setRangeText(it.word, s, e, 'end')
  box.dispatchEvent(new Event('input'))
  hideComp()
}
function hideComp() { comp.items = []; comp.hidden = true; compEl.hidden = true }

// ---------- the command line: s with the file's definitions, or a search ----------

function prelude() {
  // the pages that define something, in file order
  return R.pages.filter((p) => (p.toks || []).some((t) => t.depth === 0 && (t.w === '><' || (t.w.endsWith('$') && t.w.length > 1 && !t.w.endsWith('\\$'))))).map((p) => p.text).join('\n\n')
}
const outEl = $('#out')
function outAdd(head, text, err) {
  outEl.hidden = false
  const h = document.createElement('span')
  h.className = 'head'
  h.textContent = head
  outEl.append(h)
  if (text) outEl.append(document.createTextNode(text.replace(/^\n/, '') + '\n'))
  if (err) { const e = document.createElement('span'); e.className = 'err'; e.textContent = err + '\n'; outEl.append(e) }
  while (outEl.childNodes.length > 400) outEl.removeChild(outEl.firstChild)
  outEl.scrollTop = 1e9
}
async function command(line) {
  const t = line.trim()
  if (!t) return
  if (t.startsWith('/')) { search(t.slice(1)); return }
  const r = await api('srun', { text: t, path: R.path, prelude: prelude(), trace: false, ms: 5000 })
  outAdd('› ' + t, r.out, r.error && r.error.msg)
  box.select()
}

function search(q) {
  R.found = []
  if (q) {
    for (const p of R.pages) for (const t of p.toks || []) if (t.w.includes(q)) R.found.push({ page: p, from: t.from })
  }
  R.pages.forEach(buildPage)
  R.foundAt = -1
  status(q ? R.found.length + ' found · Enter or F3 for the next' : '')
  if (R.found.length) nextFound()
}
function nextFound() {
  if (!R.found.length) return
  R.foundAt = (R.foundAt + 1) % R.found.length
  const f = R.found[R.foundAt]
  flyTo(f.page, f.from)
  status((R.foundAt + 1) + ' of ' + R.found.length)
}

// ---------- running ----------

let runTimer = 0
function scheduleRun() { clearTimeout(runTimer); runTimer = setTimeout(() => runFile(true), 450) }
async function runFile(quiet = false) {
  const text = R.edit ? joinPages() : R.text
  const r = await api('srun', { text, path: R.path, trace: true, ms: quiet ? 1500 : 15000 })
  if (r.error && !('out' in r)) { status(r.error, true); return }
  // char offsets from the machine are code points; the page offsets are JS string offsets
  const cps = Array.from(text)
  const jsAt = []
  let o = 0
  for (const c of cps) { jsAt.push(o); o += c.length }
  jsAt.push(o)
  const pageAt = (off) => {
    let best = R.pages[0]
    for (const p of R.pages) if (p.start <= off) best = p
    return best
  }
  for (const p of R.pages) { p.tags = null; p.err = null }
  for (const t of r.tags || []) {
    const off = jsAt[t.at] ?? 0
    const p = pageAt(off)
    const li = lineOf(p, off - p.start)
    const s = t.text.replace(/\s+/g, ' ').trim()
    if (!s) continue
    if (!p.tags) p.tags = new Map()
    const was = p.tags.get(li)
    p.tags.set(li, (was ? was + ' · ' : '') + s)
  }
  for (const p of R.pages) if (p.tags) for (const [li, s] of p.tags) if (s.length > 80) p.tags.set(li, s.slice(0, 78) + '…')
  if (r.error) {
    const off = r.error.at != null ? jsAt[r.error.at] ?? 0 : null
    const p = off != null ? pageAt(off) : R.pages[0]
    const local = off != null ? off - p.start : 0
    p.err = { msg: r.error.msg, li: lineOf(p, local), tok: off != null ? tokAt(p, local) : null }
  }
  R.pages.forEach(buildPage)
  R.run = r
  outEl.innerHTML = ''
  outAdd('▸ ' + R.path + ' · ' + (r.ms || 0).toFixed(1) + ' ms · ' + (r.ops || 0) + ' ops' + (r.exit != null ? ' · exit ' + r.exit : ''), r.out, r.error && r.error.msg)
  if (quiet) outEl.hidden = outEl.dataset.open !== '1'
  else outEl.dataset.open = '1'
  status(r.error ? 'error: ' + r.error.msg : 'ran in ' + (r.ms || 0).toFixed(1) + ' ms', !!r.error)
}

async function buildRust(bin) {
  if (!R.path) return
  await save()
  status(bin ? 'building with rustc…' : 'writing the Rust…')
  const r = await api('rust', { path: R.path, text: R.text, build: bin })
  if (r.error) { outAdd('✗ build', '', r.error); status('the build failed', true); return }
  outAdd('▸ ' + (bin ? 'built ' + r.bin : 'wrote ' + r.rs), '', '')
  status(bin ? 'built ' + r.bin : 'wrote ' + r.rs)
}

// ---------- looking and aiming ----------

const ray = new THREE.Raycaster()
const ndc = new THREE.Vector2()
const mouse = { x: 0, y: 0 }
function camAxes() {
  return {
    right: new THREE.Vector3(1, 0, 0).applyQuaternion(cam.q),
    up: new THREE.Vector3(0, 1, 0).applyQuaternion(cam.q),
    fwd: new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q),
  }
}
function aimRay() {
  if (locked) ndc.set(0, 0)
  else {
    const r = canvas.getBoundingClientRect()
    ndc.set(((mouse.x - r.left) / r.width) * 2 - 1, -((mouse.y - r.top) / r.height) * 2 + 1)
  }
  ray.setFromCamera(ndc, camera)
  return ray.ray
}
// the page and character under the aim: pages face the view, so each is a plane across it
function pick() {
  const rr = aimRay()
  const { right, up, fwd } = camAxes()
  let best = null
  for (const p of R.pages) {
    if (!p.group || p === (R.carry && R.carry.page)) continue
    const denom = rr.direction.dot(fwd)
    if (denom <= 1e-6) continue
    const t = p.pos.clone().sub(rr.origin).dot(fwd) / denom
    if (t <= 0.1) continue
    const hit = rr.origin.clone().addScaledVector(rr.direction, t)
    const v = hit.clone().sub(p.pos)
    const x = v.dot(right) / EM, y = v.dot(up) / EM
    const tagW = p.tags || p.err ? 30 : 0
    if (x < -p.wEm / 2 - 0.6 || x > p.wEm / 2 + 0.6 + tagW || y < -p.hEm / 2 - 0.4 || y > p.hEm / 2 + 0.8) continue
    if (!best || t < best.t) best = { page: p, t, x, y, hit }
  }
  if (!best) return null
  const p = best.page
  const off = offAt(p, Math.min(best.x, p.wEm / 2), best.y)
  const tok = tokAt(p, off) || tokAt(p, Math.max(0, off - 1))
  return { ...best, off, tok }
}
function tokWorld(p, t) {
  const a = charAt(p, t.from)
  const { right, up } = camAxes()
  return p.pos.clone().addScaledVector(right, (a.x + (t.to - t.from) * ADV / 2) * EM).addScaledVector(up, (a.y + 0.3) * EM)
}

// threads: from the word aimed at to where it is defined, or from a definition to its uses
const threads = new THREE.Group()
scene.add(threads)
let threadKey = ''
function drawThreads(h) {
  const key = h && h.tok ? R.pages.indexOf(h.page) + ':' + h.tok.from : ''
  if (key === threadKey && !h) return
  threadKey = key
  disposeGroup(threads)
  if (!h || !h.tok) return
  const name = bare(h.tok.w)
  const defs = R.defs.get(name)
  if (!defs) return
  const from = tokWorld(h.page, h.tok)
  let targets = []
  const isDef = defs.some((d) => d.page === h.page && d.tok.from === h.tok.from)
  if (isDef) {
    for (const p of R.pages) for (const t of p.toks || []) if (bare(t.w) === name && !(p === h.page && t.from === h.tok.from)) targets.push(tokWorld(p, t))
  } else targets = defs.map((d) => tokWorld(d.page, d.tok))
  for (const to of targets.slice(0, 40)) {
    const mid = from.clone().add(to).multiplyScalar(0.5).add(new THREE.Vector3(0, from.distanceTo(to) * 0.18 + 0.4, 0))
    const c = new THREE.QuadraticBezierCurve3(from, mid, to)
    const g = new THREE.BufferGeometry().setFromPoints(c.getPoints(24))
    threads.add(new THREE.Line(g, lineMat(isDef ? '#2f6f62' : '#8a2d6a')))
  }
}

function describe(h) {
  if (!h) return ''
  const t = h.tok
  if (!t) return '§' + (R.pages.indexOf(h.page) + 1)
  const name = bare(t.w)
  const parts = [t.w]
  const doc = R.docs.get(t.w)
  if (doc) parts.push((doc.sig ? doc.sig + '  ' : '') + doc.doc)
  else {
    const last = t.w[t.w.length - 1]
    if (t.w.length > 1 && SUFFIX.includes(last) && R.caps.has(last)) { const c = R.caps.get(last); parts.push(name + ' · ' + c.name + ': ' + c.doc) }
  }
  if (/\.s\.$/.test(t.w)) parts.push('loads ' + t.w.slice(0, -1) + ' · G opens it, B comes back')
  const defs = R.defs.get(name)
  if (defs) {
    const d = defs[0]
    parts.push(d.page === h.page && d.tok.from === t.from ? 'defined here' : 'defined on §' + (R.pages.indexOf(d.page) + 1) + ' · G goes there')
  } else if (t.kind === 'open' || t.kind === 'close') parts.push('a box, level ' + (t.depth + 1))
  return parts.join('  ·  ')
}

// ---------- moving ----------

function flyTo(p, off = 0) {
  const { fwd } = camAxes()
  const dist = Math.max(4, p.hEm * EM * 1.3, p.wEm * EM * 0.9)
  const t = off ? tokAt(p, off) : null
  const target = t ? tokWorld(p, t) : p.pos.clone()
  R.back.push({ pos: cam.pos.clone(), q: cam.q.clone() })
  if (R.back.length > 50) R.back.shift()
  cam.anim = { from: cam.pos.clone(), to: target.clone().addScaledVector(fwd, -dist), q0: cam.q.clone(), q1: cam.q.clone(), t: 0 }
}
function goDef(h) {
  if (!h || !h.tok) return
  // a file the program loads opens in the room
  const m = h.tok.w.match(/^(.+\.s)\.$/)
  if (m) {
    const dir = R.path.includes('/') ? R.path.slice(0, R.path.lastIndexOf('/') + 1) : ''
    const parts = (dir + m[1]).split('/')
    const norm = []
    for (const x of parts) { if (x === '..') norm.pop(); else if (x && x !== '.') norm.push(x) }
    R.back.push({ path: R.path, pos: cam.pos.clone(), q: cam.q.clone() })
    openFile(norm.join('/'))
    return
  }
  const defs = R.defs.get(bare(h.tok.w))
  if (!defs) { status(h.tok.w + ' is not defined in this file'); return }
  const d = defs.find((x) => !(x.page === h.page && x.tok.from === h.tok.from)) || defs[0]
  flyTo(d.page, d.tok.from)
}
async function goBack() {
  const b = R.back.pop()
  if (!b) return
  if (b.path && b.path !== R.path) {
    const keep = R.back
    await openFile(b.path)
    R.back = keep
    cam.pos.copy(b.pos)
    cam.q.copy(b.q)
    return
  }
  cam.anim = { from: cam.pos.clone(), to: b.pos, q0: cam.q.clone(), q1: b.q, t: 0 }
}

function moveCamera(dt) {
  if (cam.anim) {
    const a = cam.anim
    a.t = Math.min(1, a.t + dt / 0.35)
    const k = a.t * a.t * (3 - 2 * a.t)
    cam.pos.lerpVectors(a.from, a.to, k)
    cam.q.slerpQuaternions(a.q0, a.q1, k)
    if (a.t >= 1) cam.anim = null
    return
  }
  // what the wheel set going eases out
  cam.pos.addScaledVector(cam.glide, Math.min(1, dt * 9))
  cam.glide.multiplyScalar(Math.max(0, 1 - dt * 9))
  const K = cam.keys
  const typing = document.activeElement === box || document.activeElement === $('#f-filter')
  if (typing) { cam.vel.multiplyScalar(0.8); cam.pos.add(cam.vel.clone().multiplyScalar(dt)); return }
  const want = new THREE.Vector3((K.has('KeyD') ? 1 : 0) - (K.has('KeyA') ? 1 : 0), (K.has('Space') ? 1 : 0) - (K.has('KeyC') ? 1 : 0), (K.has('KeyS') ? 1 : 0) - (K.has('KeyW') ? 1 : 0))
  const fast = K.has('ShiftLeft') || K.has('ShiftRight') ? 3.5 : 1
  want.multiplyScalar(cam.speed * fast).applyQuaternion(cam.q)
  cam.vel.lerp(want, Math.min(1, dt * 10))
  cam.pos.addScaledVector(cam.vel, dt)
  const roll = (K.has('KeyQ') ? 1 : 0) - (K.has('KeyE') ? 1 : 0)
  if (roll) cam.q.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), roll * dt * 1.6))
}
function look(dx, dy) {
  const s = 0.0024
  cam.q.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), -dx * s))
  cam.q.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), -dy * s))
  cam.q.normalize()
}

// ---------- carrying pages ----------

function startCarry(h) {
  const { right, up } = camAxes()
  const v = h.page.pos.clone().sub(h.hit)
  R.carry = { page: h.page, dist: h.t, ox: v.dot(right), oy: v.dot(up) }
  status('carrying §' + (R.pages.indexOf(h.page) + 1) + ' · the wheel pushes it · let go to set it down')
}
function carryFrame() {
  const c = R.carry
  if (!c) return
  const rr = aimRay()
  const { right, up } = camAxes()
  const p = c.page
  p.pos.copy(rr.origin).addScaledVector(rr.direction, c.dist).addScaledVector(right, c.ox).addScaledVector(up, c.oy)
  p.group.position.copy(p.pos)
}
function endCarry() {
  if (!R.carry) return
  R.carry.page.placed = true
  R.carry = null
  saveLayout()
  status('set down')
}

// ---------- input ----------

function typingNow() {
  const a = document.activeElement
  return a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA')
}
document.addEventListener('pointerlockchange', () => {
  locked = document.pointerLockElement === canvas
  $('#cross').hidden = !locked
  $('#hint').textContent = locked ? '' : 'click empty space or press Tab to look around · F1 for the keys'
})
function lock() { try { const r = canvas.requestPointerLock(); if (r && r.catch) r.catch(() => {}) } catch {} }
function unlock() { if (document.pointerLockElement) document.exitPointerLock() }

let down = null
let lastClick = { t: 0, page: null }
canvas.addEventListener('contextmenu', (e) => e.preventDefault())
canvas.addEventListener('mousedown', (e) => {
  mouse.x = e.clientX; mouse.y = e.clientY
  if (e.button === 2) { down = { look: true, x: e.clientX, y: e.clientY }; return }
  if (e.button === 1) { e.preventDefault(); goDef(pick()); return }
  if (e.button !== 0) return
  const h = pick()
  if (!h) {
    if (!locked) lock()
    else if (R.edit) endEdit(true)
    return
  }
  down = { h, x: e.clientX, y: e.clientY, t: performance.now(), moved: 0 }
})
addEventListener('mousemove', (e) => {
  mouse.x = e.clientX; mouse.y = e.clientY
  if (locked) {
    look(e.movementX, e.movementY)
    if (down && down.h) down.moved += Math.abs(e.movementX) + Math.abs(e.movementY)
  } else if (down && down.look) look(e.clientX - down.x, e.clientY - down.y), (down.x = e.clientX), (down.y = e.clientY)
  else if (down && down.h) down.moved = Math.abs(e.clientX - down.x) + Math.abs(e.clientY - down.y)
  if (down && down.h && !R.carry && down.moved > 6 && (!locked || performance.now() - down.t > 200)) startCarry(down.h)
})
addEventListener('mouseup', (e) => {
  if (!down) return
  const d = down
  down = null
  if (d.look) return
  if (R.carry) { endCarry(); return }
  if (!d.h) return
  const h = d.h
  const now = performance.now()
  if (R.edit === h.page) {
    box.focus({ preventScroll: true })
    box.setSelectionRange(h.off, h.off)
    buildCaret(h.page)
  } else if (lastClick.page === h.page && now - lastClick.t < 380) {
    startEdit(h.page, h.off)
  } else status(describe(h) || '')
  lastClick = { t: now, page: h.page }
})
canvas.addEventListener('wheel', (e) => {
  e.preventDefault()
  const dir = e.deltaY > 0 ? -1 : 1
  if (R.carry) { R.carry.dist = Math.max(1, R.carry.dist * (dir > 0 ? 0.88 : 1.14)); return }
  if (e.altKey) {
    const h = pick()
    if (h) {
      const rr = aimRay()
      h.page.pos.addScaledVector(rr.direction, dir > 0 ? -h.t * 0.12 : h.t * 0.14)
      h.page.group.position.copy(h.page.pos)
      h.page.placed = true
      saveLayout()
      return
    }
  }
  // the wheel moves through the room: in and out along the view, which is the zoom,
  // sideways with a sideways scroll, up and down with Shift
  const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? 400 : 1
  const dx = e.deltaX * unit, dy = e.deltaY * unit
  const { right, up, fwd } = camAxes()
  const k = cam.speed * 0.0025
  if (e.shiftKey) cam.glide.addScaledVector(up, -(dy || dx) * k)
  else {
    cam.glide.addScaledVector(fwd, -dy * k * (e.ctrlKey ? 2.5 : 1))
    cam.glide.addScaledVector(right, dx * k)
  }
}, { passive: false })

addEventListener('keydown', (e) => {
  const k = e.key
  if (k === 'F1' || (k === '?' && !typingNow())) { e.preventDefault(); $('#help').hidden = !$('#help').hidden; return }
  // the box handles its own runs and saves
  if (e.target !== box) {
    if (k === 'F5' || (k === 'Enter' && (e.ctrlKey || e.metaKey) && !typingNow())) { e.preventDefault(); runFile(); return }
    if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 's') { e.preventDefault(); save(); return }
  }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'o') { e.preventDefault(); showFiles(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'b') { e.preventDefault(); buildRust(!e.shiftKey); return }
  if (typingNow()) return
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'z') { e.preventDefault(); e.shiftKey ? redo() : undo(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'y') { e.preventDefault(); redo(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'l') {
    e.preventDefault()
    for (const p of R.pages) p.placed = false
    placeAll(null)
    saveLayout()
    status('spread out again')
    return
  }
  if (e.ctrlKey || e.metaKey || e.altKey) return
  if (k === 'Escape') { if (!$('#help').hidden) { $('#help').hidden = true; return } if (R.found.length) { search(''); return } return }
  if (k === 'Tab') { e.preventDefault(); locked ? unlock() : lock(); return }
  if (k === 'Enter') {
    e.preventDefault()
    const h = pick()
    if (h) startEdit(h.page, h.off)
    else box.focus()
    return
  }
  if (k === '/') { e.preventDefault(); box.value = '/'; box.focus(); return }
  if (k === '`') { e.preventDefault(); outEl.hidden = !outEl.hidden; outEl.dataset.open = outEl.hidden ? '0' : '1'; return }
  if (k === 'F3') { e.preventDefault(); nextFound(); return }
  if (k === 'Home') { home(); return }
  if (k === '=' || k === '+' || k === '-') {
    cam.speed = Math.max(1, Math.min(120, cam.speed * (k === '-' ? 0.8 : 1.25)))
    store.set('speed', cam.speed)
    status('speed ' + cam.speed.toFixed(1))
    return
  }
  if (k === 'g' || k === 'G') { goDef(pick()); return }
  if (k === 'b' || k === 'B') { goBack(); return }
  if (k === 'l' || k === 'L') { R.live = !R.live; store.set('live', R.live); showFile(); status(R.live ? 'live: runs after every edit' : 'live off'); if (R.live) runFile(true); return }
  if (k === 'n' || k === 'N') { newPage(pick()); return }
  if (k === '[' || k === ']') { movePage(pick(), k === '[' ? -1 : 1); return }
  cam.keys.add(e.code)
})
addEventListener('keyup', (e) => cam.keys.delete(e.code))
addEventListener('blur', () => cam.keys.clear())

function newPage(h) {
  const i = h ? R.pages.indexOf(h.page) + 1 : R.pages.length
  const p = { text: '', sep: '\n\n' }
  layoutText(p)
  buildPage(p)
  const { fwd } = camAxes()
  p.pos = cam.pos.clone().addScaledVector(fwd, 5)
  p.placed = true
  p.group.position.copy(p.pos)
  R.pages.splice(i, 0, p)
  R.pages.forEach(buildPage)
  startEdit(p, 0)
}
function movePage(h, d) {
  if (!h) return
  const i = R.pages.indexOf(h.page), j = i + d
  if (j < 0 || j >= R.pages.length) return
  const [p] = R.pages.splice(i, 1)
  R.pages.splice(j, 0, p)
  R.pages.forEach(buildPage)
  changed()
  status('§' + (i + 1) + ' is now §' + (j + 1))
}

// ---------- files sheet ----------

let fileList = []
let fileSel = 0
async function showFiles() {
  const r = await api('ls')
  if (r.error) { status(r.error, true); return }
  $('#root').textContent = r.root
  const recent = store.get('recent', [])
  fileList = r.files.map((f) => f.path).sort((a, b) => {
    const ra = recent.indexOf(a), rb = recent.indexOf(b)
    return (ra < 0 ? 999 : ra) - (rb < 0 ? 999 : rb) || a.localeCompare(b)
  })
  unlock()
  $('#files').hidden = false
  $('#f-filter').value = ''
  $('#f-filter').focus()
  renderFiles()
}
function filesShown() {
  const q = $('#f-filter').value.trim().toLowerCase()
  return fileList.filter((f) => f.toLowerCase().includes(q))
}
function renderFiles() {
  const el = $('#f-list')
  el.innerHTML = ''
  const fs = filesShown()
  fileSel = Math.min(fileSel, Math.max(0, fs.length - 1))
  fs.slice(0, 300).forEach((f, i) => {
    const d = document.createElement('div')
    d.className = i === fileSel ? 'on' : ''
    d.textContent = f
    if (f === R.path) { const s = document.createElement('span'); s.textContent = 'open'; d.append(s) }
    d.onclick = () => { $('#files').hidden = true; openFile(f) }
    el.append(d)
  })
  const q = $('#f-filter').value.trim()
  if (q.endsWith('.s') && !fileList.includes(q)) {
    const d = document.createElement('div')
    d.textContent = 'make ' + q
    d.className = fs.length ? '' : 'on'
    d.onclick = () => { $('#files').hidden = true; openFile(q) }
    el.append(d)
  }
}
$('#f-filter').addEventListener('input', () => { fileSel = 0; renderFiles() })
$('#f-filter').addEventListener('keydown', (e) => {
  const fs = filesShown()
  if (e.key === 'ArrowDown') { e.preventDefault(); fileSel = Math.min(fs.length - 1, fileSel + 1); renderFiles() }
  else if (e.key === 'ArrowUp') { e.preventDefault(); fileSel = Math.max(0, fileSel - 1); renderFiles() }
  else if (e.key === 'Escape') { $('#files').hidden = true; canvas.focus() }
  else if (e.key === 'Enter') {
    e.preventDefault()
    const q = $('#f-filter').value.trim()
    const f = fs[fileSel] || (q.endsWith('.s') ? q : null)
    if (f) { $('#files').hidden = true; openFile(f) }
  }
})

// ---------- the frame ----------

function resize() {
  const w = innerWidth, h = innerHeight
  renderer.setSize(w, h, false)
  camera.aspect = w / h
  camera.updateProjectionMatrix()
  T.shared.uViewH.value = h * renderer.getPixelRatio()
}
addEventListener('resize', resize)

let last = performance.now()
let hoverKey = ''
function frame(now) {
  const dt = Math.min(0.05, (now - last) / 1000)
  last = now
  moveCamera(dt)
  camera.position.copy(cam.pos)
  camera.quaternion.copy(cam.q)
  camera.updateMatrixWorld()
  // pages face the view and stay level to the screen
  for (const p of R.pages) if (p.group) p.group.quaternion.copy(cam.q)
  carryFrame()
  const h = R.carry ? null : pick()
  R.hover = h
  const key = h && h.tok ? R.pages.indexOf(h.page) + ':' + h.tok.from : ''
  if (key !== hoverKey) {
    hoverKey = key
    $('#aim').textContent = describe(h)
  }
  drawThreads(h)
  renderer.render(scene, camera)
  requestAnimationFrame(frame)
}

async function boot() {
  resize()
  await T.loadFonts(['fonts/ibm-plex-mono-latin-400-normal.woff', 'fonts/ibm-plex-mono-latin-ext-400-normal.woff'])
  ADV = T.glyph('0').adv
  const pal = await api('palette')
  for (const g of pal.groups || []) {
    if (g.name === 'graph') continue
    for (const it of g.items) if (!R.docs.has(it.w)) R.docs.set(it.w, { sig: it.sig || '', doc: it.doc || '' })
  }
  for (const c of pal.caps || []) R.caps.set(c.sigil, c)
  $('#hint').textContent = 'click empty space or press Tab to look around · F1 for the keys'
  showFile()
  const want = new URLSearchParams(location.search).get('file') || store.get('file', null)
  const ls = await api('ls')
  const files = (ls.files || []).map((f) => f.path)
  const path = want && (files.includes(want) || want.endsWith('.s')) ? want : files[0] || 'main.s'
  await openFile(path)
  requestAnimationFrame(frame)
}
function screenOf(v) {
  const p = v.clone().project(camera)
  const r = canvas.getBoundingClientRect()
  return { x: r.left + (p.x + 1) / 2 * r.width, y: r.top + (1 - p.y) / 2 * r.height }
}
// a file changed by another editor comes back in when the room gets the focus again
addEventListener('focus', async () => {
  if (!R.path || R.dirty || R.edit) return
  const r = await api('read', { path: R.path })
  if (r.error || r.text === R.savedText) return
  R.savedText = r.text
  R.undo.push(R.text)
  restore(r.text)
  status('read again: the file changed outside the room')
})
window.__room = { R, cam, openFile, runFile, startEdit, endEdit, pick, flyTo, home, look, search, tokWorld, screenOf }
boot()
